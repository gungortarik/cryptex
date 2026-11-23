"""Prepare the detached startup inventory for an independently approved personal Mac release."""
import argparse
import ctypes
import json
import os
from pathlib import Path
import platform
import re
import stat
import subprocess
import sys
import tempfile

from verify_first_install_mac import verify

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "m8"))
from mac_lan_setup import inventory


def inspect_image(args, _stage, timeout):
    return subprocess.run(args, check=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                          text=True, timeout=timeout).stdout


def write_inventory(bundle, revision, pin, helper, directory):
    contents = inventory(bundle, platform.machine(), "cryptex-m17-" + revision, inspect_image, sealed=True)
    if "Contents/Resources/frontend/index.html" not in {entry["path"] for entry in contents["files"]}:
        raise ValueError("Missing protected Desktop frontend")
    manifest = directory / "inventory.json"
    manifest.write_bytes(json.dumps(contents, separators=(",", ":")).encode())
    subprocess.run([helper, pin, manifest, directory / "inventory.cms"], check=True, timeout=60)


def publish(stage, destination):
    library = ctypes.CDLL(None, use_errno=True)
    rename = library.renamex_np
    rename.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    if rename(os.fsencode(stage), os.fsencode(destination), 0x00000004):  # Darwin RENAME_EXCL
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code), destination)


def approved_snapshot(bundle, scratch, pin, approved_image):
    snapshot = scratch / "approved.app"
    subprocess.run(["/usr/bin/ditto", bundle, snapshot], check=True, timeout=120)
    verify(snapshot, pin, approved_image)
    return snapshot


def prepare(bundle, revision, pin, approved_image, helper, output):
    if not re.fullmatch("[0-9a-f]{40}", revision):
        raise ValueError("Expected the full reviewed source revision")
    verify(bundle, pin, approved_image)
    destination = output / ("cryptex-release-" + approved_image.lower())
    if os.path.lexists(destination):
        raise FileExistsError(destination)
    with tempfile.TemporaryDirectory(dir=output) as scratch:
        stage = Path(scratch) / "release"
        stage.mkdir(mode=0o700)
        snapshot = approved_snapshot(bundle, Path(scratch), pin, approved_image)
        write_inventory(snapshot, revision, pin.lower(), helper, stage)
        verify(bundle, pin, approved_image)
        publish(stage, destination)
    return destination


def read_pair(directory):
    pair = {}
    folder = os.open(directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for name, limit in (("inventory.json", 1024**2), ("inventory.cms", 2 * 1024**2)):
            fd = os.open(name, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW, dir_fd=folder)
            with os.fdopen(fd, "rb") as source:
                info = os.fstat(fd)
                if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or not 0 < info.st_size <= limit:
                    raise ValueError("Unexpected inventory file")
                data = source.read(limit + 1)
                if len(data) != info.st_size:
                    raise ValueError("Unstable inventory input")
                pair[name] = data
    finally:
        os.close(folder)
    return pair


def stage_delivery(bundle, source, pin, approved_image, output):
    # Run the read-only verifier only in a private copy of the approved sealed
    # app. The verification path never starts the service or opens a profile.
    verify(bundle, pin, approved_image)
    name = "cryptex-release-" + approved_image.lower()
    destination = output / name
    with tempfile.TemporaryDirectory(dir=output) as scratch:
        snapshot = approved_snapshot(bundle, Path(scratch), pin, approved_image)
        stage = Path(scratch) / name
        stage.mkdir(mode=0o700)
        pair = read_pair(source)
        for filename, data in pair.items():
            (stage / filename).write_bytes(data)
        result = subprocess.run([snapshot / "Contents/MacOS/cryptex-desktop", "--cryptex-verify-installed-inventory"],
                                check=True, capture_output=True, timeout=25)
        if result.stdout.splitlines() != [b"sealed_inventory_verified"]:
            raise ValueError("Package did not verify its inventory")
        verify(bundle, pin, approved_image)
        try:
            publish(stage, destination)
        except FileExistsError:
            if read_pair(destination) != pair:
                raise ValueError("Different inventory already occupies this release slot") from None
    return destination


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("prepare", help="Sign the inventory on the owner's signing machine")
    stage = commands.add_parser("stage", help="Verify and place a delivered inventory on the receiving Mac")
    for action in (build, stage):
        action.add_argument("bundle", type=Path, help="Signed Cryptex .app from the release")
        action.add_argument("certificate_sha256", help="Independently recorded DER certificate SHA-256")
        action.add_argument("image_sha256", help="Independently approved signed Desktop image SHA-256")
        action.add_argument("output", type=Path, help="Existing parent directory for the release slot")
    build.add_argument("revision", help="Full reviewed source revision")
    build.add_argument("--mac-helper", required=True, type=Path, help="Trusted local Keychain CMS helper")
    stage.add_argument("source", type=Path, help="Directory containing inventory.json and inventory.cms")
    args = parser.parse_args()
    bundle, output = args.bundle.absolute(), args.output.resolve(strict=True)
    if args.command == "prepare":
        print(prepare(bundle, args.revision, args.certificate_sha256, args.image_sha256,
                      args.mac_helper.resolve(strict=True), output))
    else:
        print(stage_delivery(bundle, args.source.absolute(), args.certificate_sha256, args.image_sha256, output))
