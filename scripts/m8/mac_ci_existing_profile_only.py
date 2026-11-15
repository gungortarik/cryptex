"""Fast native acceptance using exact pinned signed images, never a new build."""
import os
from pathlib import Path
import platform
import sys
import tempfile

from mac_ci_credential_keychain import require_hosted_mac
from mac_ci_existing_profile import exercise
from mac_identity_preflight import PreflightError, run_bounded, checked
from mac_keychain_item_ci import extract
from mac_development_startup_check import require_same_designated_identity

ARCHIVE_HASH = '11637fe468663bf6b1c731279e20077aaee08a9e2ad277cf079fd4293880aad7'


def main():
    require_hosted_mac()
    os.umask(0o077)
    if len(sys.argv) != 2:
        raise PreflightError('pinned_ci_archive_argument')
    with tempfile.TemporaryDirectory(prefix='cx-pinned-credential-') as directory:
        root = Path(directory).resolve(strict=True)
        package, meta = extract(Path(sys.argv[1]), root, ARCHIVE_HASH)
        if (meta['source'] != '21f6bf757ed6c9d6d2fdaf399e75fdc2ae98bd94' or meta['architecture'] != platform.machine()
                or int(platform.mac_ver()[0].split('.')[0]) < 26):
            raise PreflightError('pinned_ci_source_or_platform')
        desktop = package / 'development.app/Contents/MacOS/cryptex-desktop'
        creator = package / 'profile-creator'
        require_same_designated_identity(creator, desktop.with_name('cryptexd'),
            lambda args, label, timeout=30: checked(run_bounded, [str(v) for v in args], label, timeout))
        exercise(root, desktop, creator, package / 'development.app/Contents/Resources/m8-startup.json', run_bounded)
    print('pinned_signed_existing_profile_acceptance=passed', flush=True)


if __name__ == '__main__':
    try:
        main()
    except PreflightError as error:
        print(str(error), file=sys.stderr)
        sys.exit(2)
    except BaseException:
        print('pinned_ci_internal_failure', file=sys.stderr)
        sys.exit(2)
