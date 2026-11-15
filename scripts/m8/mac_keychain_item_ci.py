"""Pinned-image CI probe of a generated credential's metadata, never its value."""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import uuid
import zipfile

from mac_identity_preflight import PreflightError, run_bounded
from mac_development_diagnostic_owner import successful, verify_package
from mac_development_startup_check import FencedRun, canonical_hashes, require_same_designated_identity
from mac_transport_check import _metadata
from mac_window_process import window_progress

ARCHIVE_HASH = '25f50a847c9bbce23891ffd1bbd9e53c42f85b49d86a51918350a9bd04c6e3cf'
BOOL_FIELDS = {'keychain_unlocked', 'item_in_default_keychain', 'partition_format_recognized',
               'creator_cdhash_allowed', 'product_cdhash_allowed', 'different_image_cdhash'}
INT_FIELDS = {'interaction_disabled_status', 'lookup_status', 'match_count',
              'containing_keychain_status', 'keychain_state_status', 'default_keychain_status',
              'copy_access_status', 'partition_acl_count', 'partition_contents_status', 'partition_count'}


def sanitize_probe(raw):
    value = json.loads(raw)
    if not isinstance(value, dict) or set(value) - BOOL_FIELDS - INT_FIELDS:
        raise PreflightError('probe_output_shape')
    if any(type(v) is not (bool if k in BOOL_FIELDS else int) for k, v in value.items()):
        raise PreflightError('probe_output_type')
    if any(not -(2**31) <= value[k] < 2**31 for k in value.keys() & INT_FIELDS):
        raise PreflightError('probe_output_range')
    return value


def extract(archive, root, expected_hash=ARCHIVE_HASH):
    if hashlib.sha256(archive.read_bytes()).hexdigest() != expected_hash:
        raise PreflightError('archive_changed')
    with zipfile.ZipFile(archive) as source:
        infos = source.infolist()
        if len(infos) != 18 or len({i.filename for i in infos}) != 18:
            raise PreflightError('archive_inventory')
        for info in infos:
            path = PurePosixPath(info.filename)
            if (path.is_absolute() or '..' in path.parts or '\\' in info.filename
                    or path.parts[0] != 'Cryptex-Mac-Baslangic-Tani'
                    or not stat.S_ISREG(info.external_attr >> 16)
                    or info.file_size > 128 * 1024 * 1024):
                raise PreflightError('archive_member')
        for info in infos:
            target = root / info.filename
            target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            target.write_bytes(source.read(info))
            target.chmod((info.external_attr >> 16) & 0o700)
    package = root / 'Cryptex-Mac-Baslangic-Tani'
    meta = json.loads((package / 'diagnostic.json').read_text(encoding='utf-8'))
    verify_package(package, meta)
    if datetime.now(timezone.utc) >= datetime.fromisoformat(meta['expires_utc']):
        raise PreflightError('artifact_expired')
    return package, meta


def exercise(package, root, report, runner=run_bounded):
    fence = FencedRun(runner)
    profile = root / 'probe-runtime'
    profile.mkdir(mode=0o700)
    (profile / 'pilot-runtime.marker').write_bytes(b'm0-native-desktop-runtime-v1\n')
    creator = package / 'profile-creator'
    daemon = package / 'development.app/Contents/MacOS/cryptexd'
    desktop = daemon.with_name('cryptex-desktop')
    device = None
    report['commands'] = []

    def run(args, label, timeout=30):
        try:
            result = fence.outcome(args, timeout)
        finally:
            report['captured_groups_reaped'] = fence.stopped
        report['commands'].append({'stage': label, 'exit_code': result.code,
            'timed_out': result.timed_out, 'output_limited': result.limited,
            'pipe_failed': result.pipe_failed})
        if not successful(result):
            raise PreflightError(label + '_failed')
        return result.output

    try:
        probe = root / 'item-probe'
        run(['/usr/bin/xcrun', 'clang', '-fobjc-arc', '-Wno-deprecated-declarations',
             '-framework', 'Foundation', '-framework', 'Security',
             Path(__file__).with_name('mac_keychain_item_probe.m'), '-o', probe], 'compile_probe', 90)
        require_same_designated_identity(creator, daemon, run)
        try:
            run([creator, 'pilot-fixture', 'signed-transfer-v1', '--runtime-root', profile,
                 '--confirm-isolated-pilot'], 'create_test_profile', 45)
        finally:
            metadata = profile / 'identity/local_device.json'
            if fence.stopped and metadata.exists():
                device = str(uuid.UUID(_metadata(metadata)['device_id']))
        baseline = canonical_hashes(profile)
        if not baseline:
            raise PreflightError('canonical_baseline_missing')
        report['item_metadata'] = sanitize_probe(run([probe, 'device-event-signing-seed:' + device,
                                                     creator, daemon], 'read_metadata'))
        result = fence.outcome([desktop, '--cryptex-development-check', profile], 60)
        report['startup'] = {'exit_code': result.code, 'timed_out': result.timed_out,
            'output_limited': result.limited, 'pipe_failed': result.pipe_failed,
            'progress': window_progress(result.output),
            'passed': successful(result) and 'development_existing_profile_startup_and_cleanup=passed' in result.output.splitlines()}
        report['canonical_unchanged'] = canonical_hashes(profile) == baseline
        report['outcome'] = 'metadata_collected'
    finally:
        report['captured_groups_reaped'] = fence.stopped
        if fence.stopped and device is not None:
            if str(uuid.UUID(_metadata(profile / 'identity/local_device.json')['device_id'])) != device:
                raise PreflightError('cleanup_identity_changed')
            run(['/usr/bin/security', 'delete-generic-password', '-s', 'Cryptex-Dev-DeviceCredentials',
                 '-a', 'device-event-signing-seed:' + device], 'cleanup_exact_generated_key')
            report['credential_cleanup_verified'] = True


def main():
    if (sys.platform != 'darwin' or os.environ.get('GITHUB_ACTIONS') != 'true'
            or os.environ.get('RUNNER_ENVIRONMENT') != 'github-hosted'):
        raise PreflightError('hosted_mac_ci_only')
    archive, root = map(Path, sys.argv[1:])
    root.mkdir(mode=0o700)
    report = {'scope': 'read-only synthetic item metadata and bounded headless startup; not product acceptance',
              'archive_sha256': ARCHIVE_HASH, 'outcome': 'failed'}
    code = 0
    try:
        package, meta = extract(archive, root)
        report['product_source'] = meta['source']
        exercise(package, root, report)
    except PreflightError as error:
        report['error'] = str(error)
        code = 1
    except Exception:
        report['error'] = 'unexpected_probe_failure'
        code = 1
    finally:
        (root / 'keychain-item.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
        print(json.dumps(report), flush=True)
    return code


if __name__ == '__main__':
    sys.exit(main())
