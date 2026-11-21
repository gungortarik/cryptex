"""Stage the offline owner utility separately from the ordinary Desktop bundle."""
from pathlib import Path
import json
import os
import shutil
import subprocess
import sys

from packaged_codec import ROOT, SPIKE, stage


def scripts(destination):
    destination.mkdir(parents=True, exist_ok=True)
    for source, name in (
        (SPIKE / "offline-kit/authorize.cjs", "authorize.cjs"),
        (SPIKE / "offline-kit/open-kit.cjs", "open-kit.cjs"),
        (SPIKE / "offline-kit/export-recipient.cjs", "export-recipient.cjs"),
        (SPIKE / "offline-kit/recovery-backup.cjs", "recovery-backup.cjs"),
        (SPIKE / "offline-kit/verified-recipient.cjs", "verified-recipient.cjs"),
        (ROOT / "services/cryptexd/backup-codec/restore.cjs", "restore.cjs"),
        (SPIKE / "node_modules/fflate/umd/index.js", "fflate.cjs"),
        (SPIKE / "node_modules/openpgp/dist/node/openpgp.min.cjs", "openpgp.min.cjs"),
    ):
        shutil.copy2(source, destination / name)


if __name__ == "__main__":
    import tempfile
    import zipfile

    archive = Path(sys.argv[1])
    with tempfile.TemporaryDirectory() as temporary:
        base = Path(temporary)
        codec = base / "codec"
        pins = stage(codec)
        owner = base / "Cryptex-Offline-Kit"
        scripts(owner)
        for filename in (Path(shutil.which("node")).name, "fflate-LICENSE", "OpenPGP-LICENSE"):
            shutil.copy2(codec / filename, owner / filename)
        env = dict(os.environ, CRYPTEX_M11_PACKAGE_CODEC_DIGESTS=json.dumps(pins))
        features = "m11-offline-verifier"
        if sys.platform == "darwin":
            features += ",m8-local-startup-check"
            env["CRYPTEX_M8_LOCAL_STARTUP_CERT_DER"] = str(ROOT / "services/cryptexd/src/m8_owner_channel/release_inventory/fixtures/signer.der")
        result = subprocess.run(["cargo", "build", "--locked", "-p", "cryptexd", "--bin",
                                 "cryptex-kit-check", "--features", features,
                                 "--message-format=json"], cwd=ROOT, env=env, check=True,
                                stdout=subprocess.PIPE, text=True)
        verifier, = [event["executable"] for line in result.stdout.splitlines()
                     if (event := json.loads(line)).get("executable") and event["target"]["name"] == "cryptex-kit-check"]
        shutil.copy2(verifier, owner)
        for title, entry in (("Authorize", "authorize.cjs"), ("Export public recipient", "export-recipient.cjs"),
                             ("Keep or advance recovery backup", "recovery-backup.cjs")):
            if sys.platform == "win32":
                (owner / f"{title}.cmd").write_bytes(
                    ('@echo off\r\nsetlocal\r\nset "NODE_OPTIONS="\r\nset "NODE_PATH="\r\n'
                     f'"%~dp0node.exe" --no-addons "%~dp0{entry}"\r\n'
                     'set "kit_result=%errorlevel%"\r\npause\r\nexit /b %kit_result%\r\n').encode("ascii"))
            else:
                launcher = owner / f"{title}.command"
                launcher.write_text(f'#!/bin/sh\nunset NODE_OPTIONS NODE_PATH\ncd "$(dirname "$0")" || exit 1\nexec ./node --no-addons ./{entry}\n',
                                    encoding="ascii", newline="\n")
                launcher.chmod(0o755)
        with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED) as output:
            for item in sorted(owner.iterdir()):
                output.write(item, item.relative_to(base))
