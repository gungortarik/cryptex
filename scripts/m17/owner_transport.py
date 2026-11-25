"""Prepare or verify the official transport payload of an owner release."""
import argparse
import hashlib
import io
from pathlib import Path
import os
import re
import stat
import subprocess
import tempfile
import urllib.request
import zipfile

VERSION = "2.1.7"
NOTICES = Path(__file__).resolve().parents[2] / "third_party/syncthing" / VERSION
# Archive, executable, then linked-dependency distribution notice.
TARGETS = {
    "x86_64-pc-windows-msvc": (
        "windows-amd64", "syncthing.exe",
        "3ce7da770f546a8d02b65b2e7ee31f2a3b8b7c3f990ede045590cca4172ecfe1",
        "cd9aea20afda2b9bdba5aff2e6ac964ed5c2e741ed4ceb48ad9e1be096ca7891",
        "8603658416888cc528e67681f60aa570cf8d85a5edc81b74be683c2bfd61569e",
    ),
    "aarch64-apple-darwin": (
        "macos-arm64", "syncthing",
        "2df075b463231a307c91da657157beaed06383a941ba7ea04d33d536e3c4af38",
        "38aab77073e47a717ec59eab082670b4fccf28c256fa0f7935aebb3f4665bb01",
        "e69e8838b92f33b3e6b6e140104bd84e410b72a93e773246be5eedc36345bc4b",
    ),
}
UPSTREAM_TEXT = {
    "LICENSE.txt": "9221c2f936159b8446d329249fb4c0f25be510f447383a0f13336ac7985668a3",
    "AUTHORS.txt": "2981b9b7604222e3042f748652654ece919ff059f5dba430520ea729c67adeb2",
}


def ordinary(path, directory=False):
    info = path.lstat()
    expected = stat.S_ISDIR if directory else stat.S_ISREG
    if (not expected(info.st_mode) or (not directory and info.st_nlink != 1)
            or getattr(info, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT):
        raise ValueError(f"Indirect transport package member: {path.name}")


def verify(directory, triple):
    _, name, _, binary, notice = TARGETS[triple]
    expected = {name: binary, **UPSTREAM_TEXT, "NOTICES.txt": notice}
    ordinary(directory, directory=True)
    if {p.name for p in directory.iterdir()} != set(expected):
        raise ValueError("Unexpected transport package membership")
    for member, pin in expected.items():
        file = directory / member
        ordinary(file)
        digest = hashlib.sha256()
        with file.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
        if digest.hexdigest() != pin:
            raise ValueError(f"Transport digest mismatch: {member}")


def prepare(directory, triple):
    if os.path.lexists(directory):
        verify(directory, triple)
        return
    target, name, pin, _, _ = TARGETS[triple]
    stem = f"syncthing-{target}-v{VERSION}"
    url = f"https://github.com/syncthing/syncthing/releases/download/v{VERSION}/{stem}.zip"
    with urllib.request.urlopen(url, timeout=90) as response:
        raw = response.read(64 * 1024 * 1024 + 1)
    if hashlib.sha256(raw).hexdigest() != pin:
        raise ValueError("Official Syncthing archive digest mismatch")
    with tempfile.TemporaryDirectory(dir=directory.parent) as temporary:
        package = Path(temporary) / "transport"
        package.mkdir(mode=0o700)
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            for member in (name, *UPSTREAM_TEXT):
                (package / member).write_bytes(archive.read(f"{stem}/{member}"))
        (package / name).chmod(0o755)
        (package / "NOTICES.txt").write_bytes((NOTICES / f"Syncthing-{target}-NOTICES.txt").read_bytes())
        verify(package, triple)
        try:
            package.rename(directory)
        except OSError:
            if not directory.exists():
                raise
            verify(directory, triple)


def installed(directory, triple):
    verify(directory, triple)
    target, name, *_ = TARGETS[triple]
    engine = directory / name
    if target == "macos-arm64":
        description = subprocess.check_output(
            ["/usr/bin/codesign", "-d", "--verbose=4", engine], stderr=subprocess.STDOUT, text=True)
        if re.findall(r"^CDHash=(\w+)$", description, re.M) != ["47b6ac8b92f51c8fd2d85ec684e05829ef0101b4"]:
            raise ValueError("Installed transport CDHash mismatch")
    version = subprocess.check_output([engine, "--version"], timeout=15, text=True)
    if not version.startswith(f"syncthing v{VERSION} "):
        raise ValueError("Installed transport version mismatch")
    print(f"Verified installed Syncthing {VERSION}, official executable and notices for {target}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("prepare", "verify", "installed"))
    parser.add_argument("directory", type=Path)
    parser.add_argument("target", choices=TARGETS)
    args = parser.parse_args()
    {"prepare": prepare, "verify": verify, "installed": installed}[args.operation](args.directory, args.target)
