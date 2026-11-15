#!/usr/bin/python3
"""Diagnose cold startup separately from an explicit OS credential-access step."""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import stat
import sys
import tempfile
import uuid
import zipfile

from mac_identity_preflight import PreflightError, run_bounded
from mac_development_startup_check import FencedRun, PROFILE_STAGES, NATIVE_ERRORS, canonical_hashes
from mac_transport_check import _metadata


def progress(output):
    result = []
    for line in output.splitlines():
        if line in NATIVE_ERRORS:
            result.append({'error': line})
        elif line.startswith('{') and len(line) <= 512:
            try:
                value = json.loads(line)
            except (ValueError, RecursionError):
                continue
            name, phase = value.get('m8ProfileStage'), value.get('phase')
            if isinstance(name, str) and name in PROFILE_STAGES and phase in ('begin', 'complete', 'failed'):
                result.append({'stage': name, 'phase': phase})
    return result


def successful(outcome):
    return outcome.code == 0 and not (outcome.timed_out or outcome.limited or outcome.pipe_failed)


def verify_package(package, meta):
    if (meta.get('schema') != 1 or meta.get('diagnostic_only') is not True
            or meta.get('ci_startup_passed') is not False
            or not re.fullmatch(r'[0-9a-f]{40}', meta.get('source', ''))):
        raise PreflightError('package_metadata_invalid')
    files = meta['files']
    if not isinstance(files, dict) or not 6 <= len(files) <= 4096:
        raise PreflightError('package_inventory_invalid')
    required = {'profile-creator', 'development.app/Contents/MacOS/cryptexd',
                'development.app/Contents/MacOS/cryptex-desktop'}
    if not required.issubset(files):
        raise PreflightError('package_inventory_incomplete')
    for relative, digest in files.items():
        path = PurePosixPath(relative)
        if path.is_absolute() or '..' in path.parts or '\\' in relative:
            raise PreflightError('package_path_invalid')
        file = package / relative
        info = file.lstat()
        if (file.resolve() != package.resolve() / relative or not stat.S_ISREG(info.st_mode)
                or info.st_nlink != 1 or info.st_size > 512 * 1024 * 1024
                or hashlib.sha256(file.read_bytes()).hexdigest() != digest):
            raise PreflightError('package_changed')


def exercise(package, root, report, runner=run_bounded):
    profile = root / 'runtime'
    profile.mkdir(mode=0o700)
    (profile / 'pilot-runtime.marker').write_bytes(b'm0-native-desktop-runtime-v1\n')
    desktop = package / 'development.app/Contents/MacOS/cryptex-desktop'
    daemon = package / 'development.app/Contents/MacOS/cryptexd'
    fence = FencedRun(runner)
    report['commands'] = []

    def run(args, stage, timeout):
        try:
            outcome = fence.outcome(args, timeout)
        finally:
            report['captured_group_reaped'] = fence.stopped
        report['commands'].append({'stage': stage, 'exit_code': outcome.code,
            'timed_out': outcome.timed_out, 'output_limited': outcome.limited,
            'pipe_failed': outcome.pipe_failed, 'progress': progress(outcome.output)})
        return outcome

    try:
        created = run([package / 'profile-creator', 'pilot-fixture', 'signed-transfer-v1',
                       '--runtime-root', profile, '--confirm-isolated-pilot'], 'create_isolated_profile', 45)
        if not successful(created):
            raise PreflightError('profile_creation_failed')
        baseline = canonical_hashes(profile)
        if not baseline:
            raise PreflightError('canonical_baseline_missing')
        cold = run([desktop, '--cryptex-development-check', profile], 'cold_start', 60)
        marker = 'development_existing_profile_startup_and_cleanup=passed'
        report['cold_start_passed'] = successful(cold) and marker in cold.output.splitlines()
        if not report['cold_start_passed']:
            labels = progress(cold.output)
            if ({'stage': 'key_store', 'phase': 'begin'} not in labels
                    or {'stage': 'key_store', 'phase': 'complete'} in labels
                    or {'error': 'local_signed_startup_Deadline'} not in labels):
                raise PreflightError('cold_failure_not_keychain_wait')
            print('Keychain okuması bekledi. Şimdi yalnızca bu geçici profil için mevcut\n'
                  'daemon kimliğinin erişimini kontrol edeceğiz. macOS izin penceresi\n'
                  'gösterirse uygulama adını kontrol et; onay senin seçimin.\n'
                  'İzinleri otomatik değiştirmiyoruz. Bu adım için iki dakika var.', flush=True)
            access = run([daemon, 'identity', 'enroll-local', '--runtime-root', profile,
                          '--environment', 'dev'], 'explicit_existing_identity_access', 120)
            if not successful(access):
                raise PreflightError('explicit_identity_access_failed')
        if canonical_hashes(profile) != baseline:
            raise PreflightError('canonical_changed')
        for attempt in (1, 2):
            outcome = run([desktop, '--cryptex-development-check', profile], 'restart_' + str(attempt), 60)
            if not successful(outcome) or marker not in outcome.output.splitlines():
                raise PreflightError('restart_failed')
            if canonical_hashes(profile) != baseline:
                raise PreflightError('canonical_changed')
        report['canonical_unchanged'] = True
        report['outcome'] = ('cold_start_and_restart_passed' if report['cold_start_passed']
                             else 'restart_passed_after_explicit_identity_access')
    finally:
        report['captured_group_reaped'] = fence.stopped
        if not fence.stopped:
            raise PreflightError('writer_unresolved')
        metadata = profile / 'identity/local_device.json'
        if metadata.exists():
            device = str(uuid.UUID(_metadata(metadata)['device_id']))
            cleanup = run(['/usr/bin/security', 'delete-generic-password', '-s',
                           'Cryptex-Dev-DeviceCredentials', '-a', 'device-event-signing-seed:' + device],
                          'cleanup_exact_test_key', 30)
            report['credential_cleanup_verified'] = successful(cleanup)
            if not report['credential_cleanup_verified']:
                raise PreflightError('exact_key_cleanup_failed')
        else:
            report['credential_cleanup_verified'] = False
            raise PreflightError('cleanup_identity_metadata_missing')
        report['captured_group_reaped'] = fence.stopped


def main():
    os.umask(0o077)
    package = Path(__file__).resolve().parent
    report = {'scope': 'interactive startup diagnosis; not product or M8 acceptance', 'outcome': 'incomplete'}
    temporary = None
    print('Cryptex başlangıç tanısı. Mevcut Cryptex profilin kullanılmayacak.\n'
          'Pencereyi açık bırak. Sistem izin penceresi çıkıp çıkmadığını not et.', flush=True)
    try:
        meta = json.loads((package / 'diagnostic.json').read_text(encoding='utf-8'))
        verify_package(package, meta)
        report['source'] = meta['source']
        if sys.platform != 'darwin' or platform.machine() != meta['architecture']:
            raise PreflightError('platform_mismatch')
        if datetime.now(timezone.utc) >= datetime.fromisoformat(meta['expires_utc']):
            raise PreflightError('diagnostic_expired')
        temporary = Path(tempfile.mkdtemp(prefix='cryptex-development-diagnostic-')).resolve()
        exercise(package, temporary, report)
    except PreflightError as error:
        report.update(outcome='failed', diagnostic=str(error))
    except (OSError, ValueError, TypeError, KeyError):
        report.update(outcome='collector_incomplete')
    finally:
        if temporary is not None:
            if report.get('credential_cleanup_verified') and report.get('captured_group_reaped'):
                try:
                    shutil.rmtree(temporary)
                    report['runtime_cleanup_verified'] = True
                except OSError:
                    report['outcome'] = 'runtime_cleanup_incomplete'
            else:
                print(f'Test profili inceleme için korundu: {temporary}', flush=True)
    output = package / ('MAC-BASLANGIC-TANI-SONUC-' + uuid.uuid4().hex[:8] + '.zip')
    with zipfile.ZipFile(output, 'x') as archive:
        archive.writestr('result.json', json.dumps(report, indent=2) + '\n')
    print(f'Sonuç: {report["outcome"]}\nGönderilecek dosya: {output}', flush=True)
    passed = (report['outcome'] in ('cold_start_and_restart_passed',
                                   'restart_passed_after_explicit_identity_access')
              and report.get('credential_cleanup_verified') is True
              and report.get('captured_group_reaped') is True
              and report.get('runtime_cleanup_verified') is True)
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
