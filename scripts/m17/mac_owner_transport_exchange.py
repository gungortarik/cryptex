"""Exercise the release engine through Cryptex's signed macOS transport helper."""
from pathlib import Path
import sys

from owner_transport import prepare

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "m8"))
from mac_identity_preflight import PreflightError, checked, run_bounded
from mac_local_startup_ci import run_real_startup_checks
from test_mac_local_signing_ci import disposable_signing_identity


def main():
    with disposable_signing_identity() as (root, keychain, password, fingerprint, cargo, openssl):
        package = root / "owner-transport"
        prepare(package, "aarch64-apple-darwin")
        run_real_startup_checks(root, checked, run_bounded, cargo, openssl, keychain,
                                password, fingerprint, owner_package=package)


if __name__ == "__main__":
    try:
        main()
    except PreflightError as error:
        sys.exit(str(error))
    except Exception:
        sys.exit("mac_owner_transport_exchange_failed")
