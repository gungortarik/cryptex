"""Nonempty ExistingProfile acceptance with explicitly provisioned CI key access."""
from pathlib import Path
import shutil
import uuid

from mac_ci_credential_keychain import CredentialKeychain, SECURITY, SERVICE
from mac_development_startup_check import require_refusal
from mac_identity_preflight import PreflightError
from mac_product_key_startup import profile_snapshot
from mac_transport_check import _metadata


def exercise(root, desktop, creator, manifest, runner):
    root = root.resolve(strict=True)
    profile = root / 'ci-authorized-existing-profile'
    profile.mkdir(mode=0o700)
    (profile / 'pilot-runtime.marker').write_bytes(b'm0-native-desktop-runtime-v1\n')
    daemon = Path(desktop).with_name('cryptexd')
    complete = False
    with CredentialKeychain(root, runner) as scope:
        device = None
        try:
            probe = scope.root / 'item-probe'
            scope.run(['/usr/bin/xcrun', 'clang', '-fobjc-arc', '-Wno-deprecated-declarations',
                       '-framework', 'Foundation', '-framework', 'Security',
                       Path(__file__).with_name('mac_keychain_item_probe.m'), '-o', probe], 'compile_item_probe', 90)
            try:
                scope.fence.checked([creator, 'pilot-fixture', 'signed-transfer-v1', '--runtime-root', profile,
                                     '--confirm-isolated-pilot'], 'prepare_nonempty_ci_profile', 45)
            finally:
                metadata = profile / 'identity/local_device.json'
                if scope.fence.stopped and metadata.exists():
                    device = str(uuid.UUID(_metadata(metadata)['device_id']))
            before = profile_snapshot(profile)
            if not before['events']:
                raise PreflightError('ci_existing_history_missing')
            account = 'device-event-signing-seed:' + device
            scope.authorize(probe, account, creator, daemon)
            print('ci_exact_synthetic_key_access_provisioned=passed', flush=True)
            for _ in range(2):
                output = scope.fence.checked([desktop, '--cryptex-development-check', profile],
                                             'ci_existing_profile_startup_restart', 60)
                if 'development_existing_profile_startup_and_cleanup=passed' not in output.splitlines():
                    raise PreflightError('ci_existing_profile_startup_marker_missing')
                if profile_snapshot(profile) != before:
                    raise PreflightError('ci_existing_profile_changed')
            original = manifest.read_bytes()
            try:
                manifest.write_bytes(original + b' ')
                require_refusal(scope.fence.outcome([desktop, '--cryptex-development-check', profile], 30),
                                'local_signed_startup_Inventory')
            finally:
                manifest.write_bytes(original)
            if profile_snapshot(profile) != before:
                raise PreflightError('ci_existing_profile_refusal_changed_state')
            complete = True
        finally:
            if scope.fence.stopped and device is not None:
                if str(uuid.UUID(_metadata(profile / 'identity/local_device.json')['device_id'])) != device:
                    raise PreflightError('ci_existing_profile_cleanup_identity_changed')
                scope.run([SECURITY, 'delete-generic-password', '-s', SERVICE,
                           '-a', 'device-event-signing-seed:' + device, scope.path], 'cleanup_exact_ci_device_key')
    if complete:
        shutil.rmtree(profile)
    print('ci_authorized_nonempty_profile_startup_restart_refusals=passed', flush=True)
    print('ci_credential_preferences_restored_and_keychain_removed=passed', flush=True)
