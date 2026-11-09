"""Package committed read-only Mac identity prerequisites with closed members."""

import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import sys
import zipfile

sys.dont_write_bytecode = True
from mac_identity_preflight import FILES


ROOT = Path(__file__).resolve().parents[2]


def package_bytes(commit, files):
    if not re.fullmatch("[0-9a-f]{40}", commit) or set(files) != set(FILES):
        raise ValueError("invalid source binding")
    for name, data in files.items():
        if not data or len(data) > 65536 or b"\r" in data:
            raise ValueError("invalid source member")
        data.decode("utf-8")
    manifest = {"schema": 1, "scope": "mac_identity_preflight_only", "source_commit": commit,
                "files": {name: hashlib.sha256(files[name]).hexdigest() for name in FILES}}
    members = dict(files)
    members["MANIFEST.json"] = (json.dumps(manifest, sort_keys=True, indent=2) + "\n").encode()
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w") as archive:
        for name, data in members.items():
            item = zipfile.ZipInfo(name)
            item.create_system = 3
            item.external_attr = (0o100755 if name.endswith(".command") else 0o100644) << 16
            item.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(item, data)
    payload = output.getvalue()
    with zipfile.ZipFile(io.BytesIO(payload)) as archive:
        if archive.namelist() != list(members) or archive.testzip() is not None:
            raise ValueError("package member verification failed")
        for name, data in members.items():
            expected = 0o100755 if name.endswith(".command") else 0o100644
            if archive.read(name) != data or archive.getinfo(name).external_attr >> 16 != expected:
                raise ValueError("package byte/mode verification failed")
    return payload


def build(revision, output=None):
    commit = subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "--verify", revision + "^{commit}"], text=True).strip()
    files = {name: subprocess.check_output(["git", "-C", str(ROOT), "show", f"{commit}:scripts/m8/{name}"]) for name in FILES}
    payload = package_bytes(commit, files)
    output = output or Path.home() / "Downloads" / f"cryptex-mac-kimlik-{commit[:8]}.zip"
    output = output.resolve()
    if ROOT == output or ROOT in output.parents:
        raise ValueError("deliverable must stay outside the source repository")
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("xb") as target:
        target.write(payload)
    print("source_commit=" + commit)
    print("package_sha256=" + hashlib.sha256(payload).hexdigest())
    print("package=" + str(output))
    return output


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", default="HEAD")
    parser.add_argument("--output", type=Path)
    options = parser.parse_args()
    build(options.revision, options.output)
