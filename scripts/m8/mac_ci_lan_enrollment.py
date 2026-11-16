"""Actual product CLI enrollment inside the existing disposable CI Keychain.
No owner-machine ACL changes, fixture-created device key or peer grant.
"""
from pathlib import Path
from mac_ci_credential_keychain import CredentialKeychain
from mac_identity_preflight import PreflightError, checked
from mac_product_key_startup import profile_snapshot

def exercise(root, desktop, creator, runner):
    profile = root.resolve(strict=True)/'ci-product-lan-enrollment'
    profile.mkdir(mode=0o700)
    (profile/'pilot-runtime.marker').write_bytes(b'm0-native-desktop-runtime-v1\n')
    daemon = Path(desktop).with_name('cryptexd')
    with CredentialKeychain(root, runner) as scope:
        scope.run([creator, 'pilot-fixture', 'lan-profile-v1', '--runtime-root', profile,
                   '--brain-id', '49b66fe0-df17-46c3-93d5-e88a8330d02c', '--confirm-isolated-pilot'],
                  'ci_public_brain_create', 45)
        scope.run([daemon, 'identity', 'enroll-local', '--runtime-root', profile, '--environment', 'dev'],
                  'ci_actual_product_key_create', 60)
        outcome = scope.fence.outcome([daemon, 'identity', 'first-note-local-dev', '--runtime-root', profile,
                                     '--confirm-public-first-note-stdin'], 60,
                                     input_data=b'CI owner-confirmed first public note.')
        output = checked(lambda *_: outcome, [], 'ci_actual_product_first_note', 60)
        if 'first_note=committed_public' not in output.splitlines():
            raise PreflightError('ci_first_note_marker_missing')
        before = profile_snapshot(profile)
        if not before['events']:
            raise PreflightError('ci_first_note_history_missing')
        output = scope.run([daemon, 'transport', 'enroll-local-dev', '--runtime-root', profile,
                            '--confirm-local-only'], 'ci_actual_product_transport_enroll', 180)
        if 'transport_enrollment=local_only' not in output.splitlines():
            raise PreflightError('ci_transport_enrollment_marker_missing')
        refused = scope.fence.outcome([daemon, 'transport', 'enroll-local-dev', '--runtime-root', profile,
                                       '--confirm-local-only'], 30)
        if (refused.code != 2 or refused.timed_out or refused.limited or refused.pipe_failed
                or 'transport_enrollment_AlreadyExists' not in refused.output):
            raise PreflightError('ci_repeat_transport_enrollment_not_refused')
        for _ in range(2):
            output = scope.run([desktop, '--cryptex-development-check', profile],
                               'ci_actual_enrolled_startup_restart', 180)
            if 'development_existing_profile_startup_and_cleanup=passed' not in output.splitlines():
                raise PreflightError('ci_enrolled_startup_marker_missing')
            if profile_snapshot(profile) != before:
                raise PreflightError('ci_enrolled_startup_changed_canonical')
        if (profile/'managed-transport/transport.active').exists():
            raise PreflightError('ci_enrolled_helper_not_closed')
    print('product_owned_key_first_capture_transport_enrollment_restart=passed', flush=True)
