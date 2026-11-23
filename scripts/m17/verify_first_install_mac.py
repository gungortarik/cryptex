"""Check a Cryptex app against the owner's independently recorded signing and release hashes."""
import argparse
import hashlib
import plistlib
import re
import subprocess
import sys
import tempfile
from pathlib import Path


def check(command):
    return subprocess.run(command, capture_output=True, timeout=90, check=True).stdout


def verify_image(image, executable, role, signer_sha256):
    with tempfile.TemporaryDirectory() as temp:
        check(["/usr/bin/codesign", "--verify", "--strict", "--all-architectures",
               "-R", f'=identifier "{role}"', image])
        architectures = check(["/usr/bin/lipo", "-archs", executable]).decode("ascii").split()
        if not architectures or not all(re.fullmatch(r"[A-Za-z0-9_]+", arch) for arch in architectures):
            raise ValueError("Unrecognized Mach-O architecture")
        for arch in architectures:
            prefix = Path(temp) / f"{role}.{arch}."
            check(["/usr/bin/codesign", "--display", "--architecture", arch,
                   f"--extract-certificates={prefix}", image])
            leaf = Path(str(prefix) + "0").read_bytes()
            if hashlib.sha256(leaf).hexdigest() != signer_sha256.lower():
                raise ValueError("Unexpected Cryptex signer")


def verify(bundle, signer_sha256, release_sha256):
    if sys.platform != "darwin":
        raise ValueError("macOS required")
    if not all(re.fullmatch(r"[0-9a-fA-F]{64}", value) for value in (signer_sha256, release_sha256)):
        raise ValueError("Expected two SHA-256 fingerprints")
    bundle = Path(bundle).absolute()
    if bundle.is_symlink() or not bundle.is_dir() or bundle.suffix != ".app":
        raise ValueError("Expected a local application bundle")
    contents = bundle / "Contents"
    macos = contents / "MacOS"
    desktop = macos / "cryptex-desktop"
    daemon = macos / "cryptexd"
    info = contents / "Info.plist"
    if any(p.is_symlink() for p in (contents, macos, desktop, daemon, info)):
        raise ValueError("Indirect Cryptex image path")
    # The main executable's signature seals Info.plist and the bundle resource envelope.
    digest = hashlib.sha256()
    with desktop.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    if digest.hexdigest() != release_sha256.lower():
        raise ValueError("Different Cryptex release")
    verify_image(bundle, desktop, "com.cryptex.desktop", signer_sha256)
    verify_image(daemon, daemon, "com.cryptex.daemon", signer_sha256)
    metadata = plistlib.loads(info.read_bytes())
    if metadata.get("CFBundleIdentifier") != "com.cryptex.desktop" or metadata.get("CFBundleExecutable") != desktop.name:
        raise ValueError("Unexpected application identity")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    parser.add_argument("signer_sha256")
    parser.add_argument("release_sha256", help="SHA-256 of the signed cryptex-desktop from the signing machine")
    args = parser.parse_args()
    try:
        verify(args.bundle, args.signer_sha256, args.release_sha256)
    except (OSError, ValueError, plistlib.InvalidFileException, subprocess.SubprocessError) as exc:
        parser.exit(1, f"Cryptex verification failed: {exc}\n")
    print("Cryptex application, daemon and owner release verified.")
