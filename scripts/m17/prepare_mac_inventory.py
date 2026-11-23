"""Prepare the detached startup inventory for an independently approved personal Mac release."""
import argparse
import ctypes
import json
import os
from pathlib import Path
import platform
import re
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


def prepare(bundle, revision, pin, approved_image, helper, output):
    if not re.fullmatch("[0-9a-f]{40}", revision):
        raise ValueError("Expected the full reviewed source revision")
    verify(bundle, pin, approved_image)
    destination = output / ("cryptex-release-" + approved_image.lower())
    if os.path.lexists(destination):
        raise FileExistsError(destination)
    # Publish the signed pair together, on the same volume, without replacing a slot.
    with tempfile.TemporaryDirectory(dir=output) as scratch:
        stage = Path(scratch) / "release"
        stage.mkdir(mode=0o700)
        # Read and sign a private, independently verified snapshot of the approved bundle.
        snapshot = Path(scratch) / "approved.app"
        subprocess.run(["/usr/bin/ditto", bundle, snapshot], check=True, timeout=120)
        verify(snapshot, pin, approved_image)
        write_inventory(snapshot, revision, pin.lower(), helper, stage)
        verify(bundle, pin, approved_image)
        library = ctypes.CDLL(None, use_errno=True)
        rename = library.renamex_np
        rename.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint]
        rename.restype = ctypes.c_int
        if rename(os.fsencode(stage), os.fsencode(destination), 0x00000004):  # Darwin RENAME_EXCL
            code = ctypes.get_errno()
            raise OSError(code, os.strerror(code), destination)
    return destination


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    parser.add_argument("revision", help="Full reviewed release commit used to build this application")
    parser.add_argument("certificate_sha256", help="Independently recorded DER certificate SHA-256")
    parser.add_argument("image_sha256", help="Independently approved signed Desktop image SHA-256")
    parser.add_argument("output", type=Path, help="Existing directory for the new release inventory")
    parser.add_argument("--mac-helper", required=True, type=Path, help="Trusted local Keychain CMS helper")
    args = parser.parse_args()
    print(prepare(args.bundle.absolute(), args.revision, args.certificate_sha256, args.image_sha256,
                  args.mac_helper.resolve(strict=True), args.output.resolve(strict=True)))
