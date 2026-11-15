#!/usr/bin/python3
"""Normal no-argument opening; never adopt or replace an existing default profile."""
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import platform
import sys
import uuid
import zipfile

from mac_identity_preflight import PreflightError, run_bounded, unique_object
from mac_development_diagnostic_owner import verify_package, successful
from mac_development_startup_check import FencedRun, canonical_hashes
from mac_transport_check import _metadata
from mac_window_process import WindowProcess, natural_quit_ok
from mac_window_owner import answer
from mac_normal_profile import DefaultProfile


def exercise(package, lease, report, ask=answer, runner=run_bounded, process_type=WindowProcess):
    fence = FencedRun(runner)
    processes = []
    device = None
    baseline = None
    completed = False
    report.update(launches={}, commands=[], owner_observations={})

    def command(args, stage, timeout):
        result = fence.outcome(args, timeout)
        report['commands'].append({'stage': stage, 'exit_code': result.code,
            'timed_out': result.timed_out, 'output_limited': result.limited, 'pipe_failed': result.pipe_failed})
        if not successful(result):
            raise PreflightError(stage + '_failed')

    try:
        for name in ('cryptex-desktop', 'cryptexd'):
            result = fence.outcome(['/usr/bin/pgrep', '-x', name], 5)
            if result.code != 1 or result.timed_out or result.limited or result.pipe_failed:
                raise PreflightError('close_existing_cryptex_first')
        profile = lease.acquire()
        report['exclusive_absent_default_root_created'] = True
        (profile / 'pilot-runtime.marker').write_bytes(b'm0-native-desktop-runtime-v1\n')
        print('Varsayılan geliştirme konumunda yeni, yalnızca bu teste ait profil hazırlanıyor.\n'
              'macOS izin sorarsa şifreni yalnızca macOS penceresine yaz.', flush=True)
        command([package / 'profile-creator', 'pilot-fixture', 'signed-transfer-v1',
                 '--runtime-root', profile, '--confirm-isolated-pilot'], 'create_default_test_profile', 120)
        device = str(uuid.UUID(_metadata(profile / 'identity/local_device.json')['device_id']))
        baseline = canonical_hashes(profile)
        if not baseline:
            raise PreflightError('normal_canonical_baseline_missing')
        for label in ('normal_open', 'normal_restart'):
            lease.check()
            print('\nCryptex normal şekilde açılıyor. Open local memory seç.\n'
                  'Not yazma veya kaydetme; cihaz güvenliği işlemi yapma.', flush=True)
            process = process_type(package / 'development.app/Contents/MacOS/cryptex-desktop', None)
            processes.append((label, process))
            process.start()
            opened = ask('Cryptex ana penceresi açıldı mı?')
            report['owner_observations'][label + '_opened'] = opened
            if not opened:
                raise PreflightError('normal_open_not_confirmed')
            process.daemon_pin()
            quit_seen = ask('Şimdi Command-Q ile çık. Uygulama kapandı mı?')
            report['owner_observations'][label + '_quit'] = quit_seen
            result = process.finish(wait=45)
            report['launches'][label] = result
            if not quit_seen or not natural_quit_ok(result):
                raise PreflightError('normal_quit_not_verified')
            lease.check()
            if (canonical_hashes(profile) != baseline
                    or str(uuid.UUID(_metadata(profile / 'identity/local_device.json')['device_id'])) != device):
                raise PreflightError('normal_profile_data_changed')
        completed = True
    finally:
        safe = fence.stopped
        for label, process in processes:
            try:
                result = process.finish()
                report['launches'][label] = result
                safe &= natural_quit_ok(result)
            except Exception:
                safe = False
        report['service_shutdown_via_product_exit_verified'] = safe
        # Only the fully successful, unchanged synthetic fixture can be removed.
        # Uncertain outcomes preserve both key and profile for inspection.
        if completed and safe and device and baseline:
            lease.check()
            if (canonical_hashes(lease.path) != baseline or
                    str(uuid.UUID(_metadata(lease.path / 'identity/local_device.json')['device_id'])) != device):
                raise PreflightError('normal_cleanup_state_changed')
            command(['/usr/bin/security', 'delete-generic-password', '-s', 'Cryptex-Dev-DeviceCredentials',
                     '-a', 'device-event-signing-seed:' + device], 'cleanup_exact_test_key', 30)
            report['credential_cleanup_verified'] = True
            lease.remove()
            report['runtime_cleanup_verified'] = True
        elif lease.created:
            report['test_profile_retained'] = True
    report.update(outcome='normal_default_locator_open_restart_passed', canonical_unchanged=True,
                  device_identity_unchanged=True, no_arguments=True, real_home_retained=True)


def main():
    os.umask(0o077)
    package = Path(__file__).resolve().parent
    report = {'schema': 1, 'outcome': 'incomplete', 'ci_startup_passed': False,
              'scope': 'normal no-argument development opening/restart using synthetic fixture at absent default root',
              'not_tested': ['personal-profile migration', 'independent out-of-group helper enumeration',
                             'cross-device sync', 'public distribution', 'full M8 acceptance']}
    lease = None
    print('Cryptex normal açılış testi. Diğer Cryptex uygulamalarını Quit ile kapat.\n'
          'Mevcut varsayılan profil varsa test hiçbir şeyi değiştirmeden durur.\n'
          'İki açılış/kapanış kontrolü var. Terminali açık tut; yalnızca E/H yanıtı ver.', flush=True)
    try:
        meta = json.loads((package / 'diagnostic.json').read_text(encoding='utf-8'), object_pairs_hook=unique_object)
        verify_package(package, meta)
        if (meta.get('normal_test') != 'absent_default_root_v1' or sys.platform != 'darwin'
                or platform.machine() != meta['architecture'] or int(platform.mac_ver()[0].split('.')[0]) < 26):
            raise PreflightError('normal_platform_or_package_invalid')
        if datetime.now(timezone.utc) >= datetime.fromisoformat(meta['expires_utc']):
            raise PreflightError('normal_package_expired')
        report.update(source=meta['source'], collector_source=meta['collector_source'])
        if not os.environ.get('HOME'):
            raise PreflightError('normal_home_missing')
        lease = DefaultProfile(os.environ['HOME'])
        exercise(package, lease, report)
    except PreflightError as error:
        report.update(outcome='failed', diagnostic=str(error))
    except (Exception, KeyboardInterrupt):
        report.update(outcome='collector_incomplete')
    finally:
        if lease is not None:
            if lease.created:
                report['test_profile_retained'] = True
                print('Test profili inceleme için korundu: ' + str(lease.path), flush=True)
            lease.close()
    output = package / ('MAC-NORMAL-SONUC-' + uuid.uuid4().hex[:8] + '.zip')
    with zipfile.ZipFile(output, 'x', compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json', json.dumps(report, indent=2) + '\n')
    print('\nSonuç: ' + report['outcome'] + '\nGönderilecek dosya: ' + str(output), flush=True)
    return 0 if report.get('runtime_cleanup_verified') else 1


if __name__ == '__main__':
    sys.exit(main())
