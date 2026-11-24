"""Bind the exact NSIS Desktop bytes to the reviewed unbundled build hash."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def desktop_hash(payload, approved):
    installed = b"__TAURI_BUNDLE_TYPE_VAR_NSS"
    unbundled = b"__TAURI_BUNDLE_TYPE_VAR_UNK"
    if payload.count(installed) != 1:
        raise ValueError("Unexpected NSIS bundle-type marker")
    original = payload.replace(installed, unbundled, 1)
    if hashlib.sha256(original).hexdigest().upper() != approved:
        raise ValueError("NSIS Desktop differs from the approved build")
    return hashlib.sha256(payload).hexdigest().upper()


if __name__ == "__main__":
    folder, extractor, output = map(Path, sys.argv[1:])
    record = json.loads((folder / "build.json").read_text(encoding="utf-8-sig"))
    data = subprocess.check_output([extractor, "e", "-r", "-so", folder / "setup.exe", "cryptex-desktop.exe"],
                                   timeout=90)
    record["installed_desktop"] = desktop_hash(data, record["desktop"])
    output.write_text(json.dumps(record), encoding="utf-8")
