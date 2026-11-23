"""Sign an approved local update using the owner's OS-managed signing identity."""
import argparse
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile


def sign(updater, package, digest, signer, output, mac_helper=None):
    if not all(re.fullmatch("[0-9a-f]{64}", value) for value in (digest, signer)):
        raise ValueError("Package and certificate SHA-256 values must be 64 lowercase hex digits")
    if os.path.lexists(output):
        raise FileExistsError(output)
    if sys.platform == "darwin" and mac_helper is None:
        raise ValueError("Provide the reviewed Keychain CMS helper with --mac-helper")
    if sys.platform not in ("win32", "darwin"):
        raise ValueError("Owner signing requires Windows or macOS")
    # The operator supplies a trusted updater and an independently approved package digest.
    message = subprocess.check_output([updater, "statement", package, digest], timeout=180)
    with tempfile.TemporaryDirectory(dir=output.parent, prefix=".cryptex-receipt-") as temp:
        statement = Path(temp) / "statement"
        receipt = Path(temp) / "release.cms"
        statement.write_bytes(message)
        if sys.platform == "darwin":
            args = [mac_helper, signer, statement, receipt]
        else:
            args = ["pwsh", "-NoProfile", "-File", Path(__file__).with_name("sign_owner_update.ps1"),
                    signer, statement, receipt]
        subprocess.run(args, check=True, timeout=120)
        if not 0 < receipt.stat().st_size <= 65536:
            raise ValueError("Signed release receipt outside updater limits")
        # Publish the complete file without overwriting a concurrent operator's receipt.
        os.link(receipt, output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("updater", type=Path)
    parser.add_argument("package", type=Path)
    parser.add_argument("approved_digest")
    parser.add_argument("certificate_sha256")
    parser.add_argument("output", type=Path)
    parser.add_argument("--mac-helper", type=Path)
    args = parser.parse_args()
    sign(args.updater.resolve(strict=True), args.package, args.approved_digest,
         args.certificate_sha256, args.output, args.mac_helper)
