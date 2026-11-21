"""Stage the offline owner utility separately from the ordinary Desktop bundle."""
from pathlib import Path
import shutil
import sys

from packaged_codec import ROOT, SPIKE, stage


def scripts(destination):
    destination.mkdir(parents=True, exist_ok=True)
    for source, name in (
        (SPIKE / "offline-kit/authorize.cjs", "authorize.cjs"),
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
        stage(codec)
        owner = base / "Cryptex-Offline-Kit"
        scripts(owner)
        for filename in (Path(shutil.which("node")).name, "fflate-LICENSE", "OpenPGP-LICENSE"):
            shutil.copy2(codec / filename, owner / filename)
        if sys.platform == "win32":
            (owner / "Authorize.cmd").write_bytes(
                b'@echo off\r\n"%~dp0node.exe" "%~dp0authorize.cjs"\r\npause\r\n')
        else:
            launcher = owner / "Authorize.command"
            launcher.write_text('#!/bin/sh\ncd "$(dirname "$0")" || exit 1\nexec ./node ./authorize.cjs\n',
                                encoding="ascii", newline="\n")
            launcher.chmod(0o755)
        with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED) as output:
            for item in sorted(owner.iterdir()):
                output.write(item, item.relative_to(base))
