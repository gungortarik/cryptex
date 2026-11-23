"""Rehearse local signing of the actual app and verify an independently pinned leaf."""
import hashlib
from pathlib import Path
import plistlib
import subprocess
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts/m8"))
from test_mac_local_signing_ci import disposable_signing_identity


def command(*args, check=True):
    outcome = subprocess.run(args, timeout=90, capture_output=True, text=True)
    if check and outcome.returncode:
        raise RuntimeError(f"{Path(args[0]).name} failed: {outcome.stderr.strip()[:2048]}")
    return outcome


def verify(image, identifier, signer, public_der, directory):
    requirement = f'=identifier "{identifier}" and certificate leaf = H"{signer}"'
    command("/usr/bin/codesign", "--verify", "--strict", "--all-architectures",
            "-R", requirement, image)
    prefix = directory / identifier
    command("/usr/bin/codesign", "--display", f"--extract-certificates={prefix}", image)
    if Path(str(prefix) + "0").read_bytes() != public_der:
        raise RuntimeError("Installed signer differs from the provisioned certificate")


def main():
    bundle_id = "com.cryptex.desktop"
    daemon_id = "com.cryptex.daemon"
    with disposable_signing_identity() as (root, keychain, _, signer, _cargo, _openssl):
        bundle = root / "Cryptex.app"
        command("/usr/bin/ditto", ROOT / "target/release/bundle/macos/Cryptex.app", bundle)
        daemon_path = Path("Contents/MacOS/cryptexd")
        for image, identifier in [(bundle / daemon_path, daemon_id), (bundle, bundle_id)]:
            command("/usr/bin/codesign", "--force", "--options", "runtime", "--timestamp=none",
                    "--keychain", keychain, "--sign", signer, "--identifier", identifier, image)
        owner_pin = hashlib.sha256((root / "cert.der").read_bytes()).hexdigest()
        with (bundle / "Contents/MacOS/cryptex-desktop").open("rb") as signed:
            release_hash = hashlib.file_digest(signed, "sha256").hexdigest()
        installed = root / "Applications/Cryptex.app"
        installed.parent.mkdir()
        command("/usr/bin/ditto", bundle, installed)
        verifier = [sys.executable, "-B", ROOT / "scripts/m17/verify_first_install_mac.py", installed]
        command(*verifier, owner_pin, release_hash)
        for pin, release, reason in [("0" * 64, release_hash, "Unexpected Cryptex signer"),
                                     (owner_pin, "0" * 64, "Different Cryptex release")]:
            result = command(*verifier, pin, release, check=False)
            if result.returncode == 0 or reason not in result.stderr:
                raise RuntimeError("First-install verification accepted unexpected owner or release")
        native_notices = installed / "Contents/Resources/notices/rust.json"
        if native_notices.read_bytes() != (ROOT / "apps/desktop/src-tauri/binaries/rust-notices.json").read_bytes():
            raise RuntimeError("Installed Rust inventory differs from the input")
        info = plistlib.loads((installed / "Contents/Info.plist").read_bytes())
        if info["CFBundleIdentifier"] != bundle_id:
            raise RuntimeError("Unexpected installed bundle identifier")
        leaf = (root / "cert.der").read_bytes()
        for image, identifier in [(installed, bundle_id), (installed / daemon_path, daemon_id)]:
            verify(image, identifier, signer, leaf, root)
            wrong = f'=identifier "{identifier}" and certificate leaf = H"{"0" * 40}"'
            if command("/usr/bin/codesign", "--verify", "--strict", "-R", wrong,
                       image, check=False).returncode == 0:
                raise RuntimeError("Unrelated signer accepted")
        notice = installed / "Contents/Resources/notices/frontend.json"
        with notice.open("ab") as stream:
            stream.write(b" ")
        if command(*verifier, owner_pin, release_hash, check=False).returncode == 0:
            raise RuntimeError("First-install verification accepted changed resources")
        if command("/usr/bin/codesign", "--verify", "--strict", installed,
                   check=False).returncode == 0:
            raise RuntimeError("Changed sealed resource accepted")
        print("Installed desktop and daemon matched the disposable DER pin;",
              "wrong signer and changed resource refused;",
              "test signer sha256 =", hashlib.sha256(leaf).hexdigest())


if __name__ == "__main__":
    main()
