"""Positive startup fixture: the actual product daemon enrolls its own OS key.

This checks an empty, initialized Brain. Existing signed-event retention and the
different-image Keychain access boundary remain separate acceptance cases.
"""
import hashlib
from pathlib import Path
import shutil
import stat
import uuid

from mac_identity_preflight import PreflightError
from mac_development_startup_check import FencedRun, canonical_hashes, require_refusal
from mac_transport_check import _metadata


def profile_snapshot(profile):
    result = {}
    device = str(uuid.UUID(_metadata(profile / 'identity/local_device.json')['device_id']))
    public_key = 'devices/' + device.encode('utf-8').hex() + '/event_signing_key.json'
    for name in ('brain_identity.json', 'local_device.json', public_key):
        path = profile / 'identity' / name
        info = path.lstat()
        if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_size > 65536:
            raise PreflightError('product_key_identity_unsafe')
        result[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    events = profile / 'canonical/events'
    result['events'] = canonical_hashes(profile) if events.exists() else {}
    return result


def exercise(root, desktop, creator, manifest, runner, run):
    profile = root / 'product-owned-key-runtime'
    profile.mkdir(mode=0o700)
    (profile / 'pilot-runtime.marker').write_bytes(b'm0-native-desktop-runtime-v1\n')
    fence = FencedRun(runner)
    device = None
    complete = False
    try:
        fence.checked([creator, 'pilot-fixture', 'brain-only-v1', '--runtime-root', profile,
                       '--confirm-isolated-pilot'], 'prepare_public_brain_only', 45)
        if (profile / 'identity/local_device.json').exists() or (profile / 'canonical/events').exists():
            raise PreflightError('brain_only_fixture_created_device_or_events')
        daemon = Path(desktop).with_name('cryptexd')
        try:
            fence.checked([daemon, 'identity', 'enroll-local', '--runtime-root', profile,
                           '--environment', 'dev'], 'product_daemon_enroll_own_key', 45)
        finally:
            metadata = profile / 'identity/local_device.json'
            if fence.stopped and metadata.exists():
                device = str(uuid.UUID(_metadata(metadata)['device_id']))
        before = profile_snapshot(profile)
        if before['events']:
            raise PreflightError('product_key_fixture_not_empty')
        for _ in range(2):
            output = fence.checked([desktop, '--cryptex-development-check', profile],
                                   'product_owned_key_startup_restart', 60)
            if 'development_existing_profile_startup_and_cleanup=passed' not in output.splitlines():
                raise PreflightError('product_key_startup_marker_missing')
            if profile_snapshot(profile) != before:
                raise PreflightError('product_key_startup_changed_identity_or_events')
        original = manifest.read_bytes()
        try:
            manifest.write_bytes(original + b' ')
            require_refusal(fence.outcome([desktop, '--cryptex-development-check', profile], 30),
                            'local_signed_startup_Inventory')
        finally:
            manifest.write_bytes(original)
        if profile_snapshot(profile) != before:
            raise PreflightError('product_key_refusal_changed_identity_or_events')
        complete = True
    finally:
        if not fence.stopped:
            raise PreflightError('product_key_cleanup_writer_unresolved')
        if device is not None:
            if str(uuid.UUID(_metadata(profile / 'identity/local_device.json')['device_id'])) != device:
                raise PreflightError('product_key_cleanup_identity_changed')
            run(['/usr/bin/security', 'delete-generic-password', '-s', 'Cryptex-Dev-DeviceCredentials',
                 '-a', 'device-event-signing-seed:' + device], 'cleanup_product_owned_key', 30)
        if complete:
            shutil.rmtree(profile)
    print('product_owned_key_empty_brain_startup_restart_refusals=passed', flush=True)
