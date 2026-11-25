"""Package a reviewed standalone updater with the local owner's OS signing identity."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile

from verify_first_install_mac import verify_image

ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def package(image, approved_image, public_der, pin, output, keychain=None, *, expected_profile=None):
    if sys.platform not in ("darwin", "win32"):
        raise ValueError("Windows or macOS required")
    if not all(re.fullmatch(r"[0-9a-f]{64}", value) for value in (approved_image, pin)):
        raise ValueError("Expected lowercase SHA-256 fingerprints")
    if not public_der.is_file() or not 0 < public_der.stat().st_size <= 65536 or digest(public_der) != pin:
        raise ValueError("Different owner certificate")
    if os.path.lexists(output):
        raise FileExistsError(output)
    if image.is_symlink() or not image.is_file():
        raise ValueError("Expected a local updater executable")
    windows = sys.platform == "win32"
    with tempfile.TemporaryDirectory() as temp:
        stage = Path(temp)
        signed = stage / ("cryptex-updater.exe" if windows else "cryptex-updater")
        shutil.copy2(image, signed)
        if digest(signed) != approved_image:
            raise ValueError("Different unsigned updater")
        if subprocess.check_output([signed, "signer-fingerprint"], text=True, timeout=15).strip() != pin:
            raise ValueError("Updater trusts a different package signer")
        if windows:
            subprocess.run(["pwsh", "-NoProfile", "-File", ROOT / "scripts/m17/sign_owner_windows_updater.ps1",
                            signed, approved_image, pin], check=True, timeout=90)
        else:
            # codesign selects a Keychain identity by SHA-1. Verify the full DER by SHA-256 after signing.
            selector = hashlib.sha1(public_der.read_bytes()).hexdigest().upper()
            args = ["/usr/bin/codesign", "--force", "--timestamp=none", "--options", "runtime",
                    "--identifier", "com.cryptex.updater", "--sign", selector]
            if keychain:
                args.extend(["--keychain", keychain])
            subprocess.run([*args, signed], check=True, timeout=90)
            verify_image(signed, signed, "com.cryptex.updater", pin)
        notices = stage / "rust.json"
        notice_profile = subprocess.check_output([signed, "notice-profile"], text=True, timeout=15).strip()
        if notice_profile not in ("default", "owner-transport-update"):
            raise ValueError("Unrecognized updater build")
        if expected_profile is not None and notice_profile != expected_profile:
            raise ValueError("Updater differs from the required release build profile")
        subprocess.run(["node", ROOT / "scripts/m17/updater_notices.mjs", notices, notice_profile], cwd=ROOT,
                       check=True, timeout=120)
        # Stage in the destination directory so the public archive inherits its permissions.
        with tempfile.NamedTemporaryFile(dir=output.parent, delete=False) as handle:
            pending = Path(handle.name)
        try:
            with tarfile.open(pending, "w:gz") as archive:
                for file, name, mode in ((signed, signed.name, 0o755), (notices, "notices/rust.json", 0o644)):
                    entry = tarfile.TarInfo(name)
                    entry.size, entry.mode = file.stat().st_size, mode
                    with file.open("rb") as stream:
                        archive.addfile(entry, stream)
            archive_hash = digest(pending)
            if windows:
                os.rename(pending, output)
            else:
                os.link(pending, output)
        finally:
            pending.unlink(missing_ok=True)
    return archive_hash


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("updater", type=Path)
    parser.add_argument("approved_unsigned_sha256")
    parser.add_argument("public_der", type=Path)
    parser.add_argument("certificate_sha256")
    parser.add_argument("new_archive", type=Path)
    parser.add_argument("--keychain", type=Path)
    parser.add_argument("--expect-profile", choices=("default", "owner-transport-update"))
    args = parser.parse_args()
    print(package(args.updater, args.approved_unsigned_sha256, args.public_der,
                  args.certificate_sha256, args.new_archive, args.keychain, expected_profile=args.expect_profile))
