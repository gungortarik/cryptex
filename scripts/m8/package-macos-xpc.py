"""Package exact committed synthetic XPC source outside the repository."""
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
from mac_xpc_probe import FILES, SCOPE

ROOT = Path(__file__).resolve().parents[2]


def package_bytes(commit, files):
    if not re.fullmatch(r"[0-9a-f]{40}", commit) or set(files) != set(FILES):
        raise ValueError("source binding invalid")
    for data in files.values():
        if not data or len(data) > 65536 or b"\r" in data:
            raise ValueError("member invalid")
        data.decode("utf-8")
    manifest = {"schema": 1, "scope": SCOPE, "source_commit": commit,
                "files": {name: hashlib.sha256(files[name]).hexdigest() for name in FILES}}
    members = {name: files[name] for name in FILES}
    members["MANIFEST.json"] = (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as archive:
        for name, data in members.items():
            entry = zipfile.ZipInfo(name)
            entry.create_system = 3
            entry.external_attr = (0o100755 if name.endswith(".command") else 0o100644) << 16
            entry.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(entry, data)
    payload = buffer.getvalue()
    with zipfile.ZipFile(io.BytesIO(payload)) as archive:
        if archive.namelist() != list(members) or archive.testzip() is not None:
            raise ValueError("archive invalid")
        for name, data in members.items():
            mode = 0o100755 if name.endswith(".command") else 0o100644
            if archive.read(name) != data or archive.getinfo(name).external_attr >> 16 != mode:
                raise ValueError("archive bytes or mode invalid")
    return payload


def build(revision, output):
    commit = subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "--verify", revision + "^{commit}"], text=True).strip()
    files = {name: subprocess.check_output(["git", "-C", str(ROOT), "show", f"{commit}:scripts/m8/{name}"]) for name in FILES}
    payload = package_bytes(commit, files)
    output = (output or Path.home() / "Downloads" / f"cryptex-mac-imza-{commit[:8]}.zip").resolve()
    if output == ROOT or ROOT in output.parents:
        raise ValueError("output inside repository")
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("xb") as target:
        target.write(payload)
    print("source_commit=" + commit)
    print("package_sha256=" + hashlib.sha256(payload).hexdigest())
    print("package=" + str(output))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", default="HEAD")
    parser.add_argument("--output", type=Path)
    options = parser.parse_args()
    build(options.revision, options.output)
