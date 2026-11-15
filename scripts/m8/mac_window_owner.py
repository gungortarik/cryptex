#!/usr/bin/python3
"""Interactive normal-entry exercise in a private temporary HOME, not an install."""
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import platform
import select
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import uuid
import zipfile

from mac_identity_preflight import PreflightError, run_bounded, unique_object
from mac_development_diagnostic_owner import verify_package, successful, progress
from mac_development_startup_check import FencedRun, canonical_hashes
from mac_transport_check import _metadata
from mac_window_process import WindowProcess, natural_quit_ok


def answer(question, timeout=300):
    print('\n' + question + '\nE = evet / H = hayır veya emin değilim: ', end='', flush=True)
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if not select.select([sys.stdin], [], [], max(0, deadline - time.monotonic()))[0]:
            break
        line = sys.stdin.readline(64)
        if not line:
            raise PreflightError('owner_input_closed')
        value = line.strip().lower()
        if value in ('e', 'h'):
            return value == 'e'
        print('Yalnızca E veya H yaz. Şifre ya da başka içerik yazma: ', end='', flush=True)
    raise PreflightError('owner_step_deadline')


def validate_platform(meta):
    if meta.get('window_test') != 'normal_entry_isolated_home_v1':
        raise PreflightError('window_package_invalid')
    if sys.platform != 'darwin' or platform.machine() != meta['architecture']:
        raise PreflightError('platform_mismatch')
    if int(platform.mac_ver()[0].split('.')[0]) < 26:
        raise PreflightError('macos_26_required')
    if datetime.now(timezone.utc) >= datetime.fromisoformat(meta['expires_utc']):
        raise PreflightError('window_package_expired')


def exercise(package, home, report, ask=answer, runner=run_bounded, process_type=WindowProcess):
    profile = home / 'Library/Application Support/Cryptex-Dev/runtime/p0-2'
    binary = package / 'development.app/Contents/MacOS/cryptex-desktop'
    metadata = profile / 'identity/local_device.json'
    fence = FencedRun(runner)
    report.update(owner_observations={}, launches={}, commands=[], captured_groups_reaped=True)
    owned = []
    device = None
    creation_attempted = False

    def observe(key, question):
        value = ask(question)
        report['owner_observations'][key] = value
        if not value:
            raise PreflightError('owner_observation_not_confirmed')

    def launch(label):
        process = process_type(binary, home)
        owned.append((label, process))
        process.start()
        return process

    def finish(label, process):
        result = process.finish(wait=45)
        report['launches'][label] = result
        if not natural_quit_ok(result):
            raise PreflightError('natural_quit_not_verified')
        return result

    def command(args, label, timeout=30):
        result = fence.outcome(args, timeout)
        report['commands'].append({'stage': label, 'exit_code': result.code,
                                  'timed_out': result.timed_out, 'output_limited': result.limited,
                                  'pipe_failed': result.pipe_failed, 'progress': progress(result.output)})
        if not successful(result):
            raise PreflightError(label + '_failed')
        return result

    def stable(process, pins, key):
        report[key] = process.daemon_pin() == pins
        if not report[key]:
            raise PreflightError('daemon_instance_changed')

    try:
        # Refuse a competing singleton; never close an existing owner application.
        for name in ('cryptex-desktop', 'cryptexd'):
            existing = fence.outcome(['/usr/bin/pgrep', '-x', name], 5)
            if existing.code != 1 or existing.timed_out or existing.limited or existing.pipe_failed:
                raise PreflightError('close_existing_cryptex_first')
        print('\n1. Ayrı test profili hazırlanıyor. macOS izin penceresi açarsa şifreyi yalnızca oraya yaz.\n'
              'Sonraki Cryptex açıklamasında Open local memory seç.\n'
              'Ana pencere açılınca Inbox Note alanına "Cryptex pencere testi" yaz; KAYDETME.', flush=True)
        profile.mkdir(mode=0o700, parents=True, exist_ok=True)
        (profile / 'pilot-runtime.marker').write_bytes(b'm0-native-desktop-runtime-v1\n')
        creation_attempted = True
        try:
            command([package / 'profile-creator', 'pilot-fixture', 'signed-transfer-v1',
                     '--runtime-root', profile, '--confirm-isolated-pilot'], 'create_isolated_profile', 120)
        finally:
            if metadata.exists():
                device = str(uuid.UUID(_metadata(metadata)['device_id']))
        if device is None:
            raise PreflightError('created_identity_missing')
        baseline = canonical_hashes(profile)
        if not baseline:
            raise PreflightError('canonical_baseline_missing')
        main = launch('window_session')
        observe('opened_and_draft_entered', 'Ana pencere açıldı mı ve kaydetmeden test taslağını yazdın mı?')
        pins = main.daemon_pin()
        report['initial_owned_daemon_count'] = len(pins)
        observe('menu_reopen_kept_draft', 'Kırmızı pencere düğmesiyle kapat. Üst menü çubuğundaki Cryptex simgesinden\n'
                'Open Cryptex seç. Aynı taslak geri geldi mi?')
        stable(main, pins, 'menu_reopen_same_observed_daemon_pids')
        observe('dock_reopen_kept_draft', 'Pencereyi tekrar kırmızı düğmeyle kapat. Dock içindeki Cryptex simgesine tıkla.\n'
                'Aynı taslak geri geldi mi?')
        stable(main, pins, 'dock_reopen_same_observed_daemon_pids')
        print('\n2. Pencereyi tekrar kapat, sonra bu Terminale dön.', flush=True)
        observe('ready_for_second_launch', 'Cryptex penceresini kapattın mı?')
        command(['/usr/bin/env', 'HOME=' + str(home), binary], 'second_normal_launch', 30)
        observe('second_launch_kept_draft', 'İkinci başlatma mevcut pencereyi aynı taslakla geri getirdi mi?')
        stable(main, pins, 'second_launch_same_observed_daemon_pids')
        observe('quit_cancel_kept_draft', 'Cryptex içinde Command-Q yap. Taslağı silme onayında Cancel seç.\n'
                'Uygulama açık kaldı mı ve taslak duruyor mu?')
        stable(main, pins, 'quit_cancel_same_observed_daemon_pids')
        observe('quit_discard_confirmed', 'Tekrar Command-Q yap; bu kez Discard and Quit seç. Onay ekranını gördün mü?')
        finish('window_session', main)
        report['canonical_unchanged_after_quit'] = canonical_hashes(profile) == baseline
        if not report['canonical_unchanged_after_quit']:
            raise PreflightError('canonical_changed')

        print('\n3. Yeniden açılıyor. Açıklamada Open local memory seç. Bu kez not yazma.', flush=True)
        restarted = launch('restart')
        observe('restart_opened_without_draft', 'Ana pencere açıldı mı ve eski kaydedilmemiş taslak boş mu?')
        restarted.daemon_pin()
        # Process IDs can be reused after a completed launch; do not call PID inequality a fresh-session proof.
        observe('clean_quit_without_discard_prompt', 'Command-Q ile çık. Taslak silme sorusu olmadan kapandı mı?')
        finish('restart', restarted)
        report['canonical_unchanged_after_restart'] = canonical_hashes(profile) == baseline
        if not report['canonical_unchanged_after_restart']:
            raise PreflightError('canonical_changed')
        print('\n4. Son kontrol: açıklama tekrar açılacak. Bu kez Quit seç; belleği AÇMA.', flush=True)
        cancelled = launch('opening_cancel')
        observe('opening_explanation_cancel', 'Açıklamada Quit seçtin mi ve pencere kapandı mı?')
        result = finish('opening_cancel', cancelled)
        report['cancel_no_daemon_sampled'] = result['sampled_daemon_count'] == 0
        report['cancel_canonical_and_device_id_unchanged'] = (
            canonical_hashes(profile) == baseline
            and str(uuid.UUID(_metadata(metadata)['device_id'])) == device)
        if not report['cancel_no_daemon_sampled'] or not report['cancel_canonical_and_device_id_unchanged']:
            raise PreflightError('cancel_changed_existing_profile_or_started_daemon')
    except PreflightError as error:
        # Cleanup can fail too; retain the original fixed-code cause separately.
        report['primary_diagnostic'] = str(error)
        raise
    finally:
        safe = fence.stopped
        for label, process in owned:
            try:
                report['launches'][label] = process.finish()
                safe &= report['launches'][label].get('group_reaped') is True
            except (OSError, ValueError, PreflightError, subprocess.SubprocessError):
                safe = False
        report['captured_groups_reaped'] = safe
        if not safe:
            raise PreflightError('writer_unresolved')
        # The helper uses a different session. A reaped captured group alone is
        # not a whole-service shutdown proof. Only natural Desktop exit0 invokes
        # the product's verified daemon/helper shutdown contract. Retain the
        # test key and HOME after every forced/failed/uncertain launch.
        report['service_shutdown_via_product_exit_verified'] = all(
            natural_quit_ok(report['launches'][label]) for label, _ in owned)
        if not report['service_shutdown_via_product_exit_verified']:
            raise PreflightError('service_shutdown_unverified_profile_retained')
        if creation_attempted:
            if device is None or not metadata.exists() or str(uuid.UUID(_metadata(metadata)['device_id'])) != device:
                raise PreflightError('cleanup_identity_missing_or_changed')
            command(['/usr/bin/security', 'delete-generic-password', '-s', 'Cryptex-Dev-DeviceCredentials',
                     '-a', 'device-event-signing-seed:' + device], 'cleanup_exact_test_key')
            report['credential_cleanup_verified'] = True
        else:
            report['credential_cleanup_not_needed'] = True
    report['outcome'] = 'bounded_window_checks_passed_with_owner_observations'


def main():
    os.umask(0o077)
    package = Path(__file__).resolve().parent
    report = {'schema': 1, 'scope': 'normal entry in isolated temporary HOME; not installed-product or M8 acceptance',
              'outcome': 'incomplete', 'ci_startup_passed': False,
              'not_tested': ['independent out-of-group helper enumeration', 'OS permission denial', 'fresh native session replay', 'real personal-profile migration',
                             'cross-device sync', 'public signing/distribution']}
    home = None
    print('Cryptex pencere ve kapanış testi (yaklaşık 5–10 dakika).\n'
          'Önce açık olan diğer Cryptex uygulamalarından Quit ile çık.\n'
          'Terminali açık tut. Adımlarda E/H yanıtı ver; kod yapıştırman gerekmiyor.\n'
          'Şifreni yalnızca macOS izin penceresine yaz. Kişisel profilin kullanılmaz.', flush=True)
    try:
        meta = json.loads((package / 'diagnostic.json').read_text(encoding='utf-8'), object_pairs_hook=unique_object)
        verify_package(package, meta)
        validate_platform(meta)
        report.update(source=meta['source'], collector_source=meta['collector_source'])
        home = Path(tempfile.mkdtemp(prefix='cryptex-window-home-')).resolve()
        exercise(package, home, report)
    except PreflightError as error:
        report.update(outcome='failed', diagnostic=str(error))
    except (OSError, ValueError, TypeError, KeyError, KeyboardInterrupt, RuntimeError, subprocess.SubprocessError):
        report.update(outcome='collector_incomplete')
    finally:
        if home is not None:
            cleanup_key = report.get('credential_cleanup_verified') or report.get('credential_cleanup_not_needed')
            if cleanup_key and report.get('captured_groups_reaped'):
                try:
                    if home.resolve() != home or not stat.S_ISDIR(home.lstat().st_mode):
                        raise OSError('test_root_changed')
                    shutil.rmtree(home)
                    report['runtime_cleanup_verified'] = True
                except OSError:
                    report['outcome'] = 'runtime_cleanup_incomplete'
            else:
                print('Test profili inceleme için korundu: ' + str(home), flush=True)
    output = package / ('MAC-PENCERE-SONUC-' + uuid.uuid4().hex[:8] + '.zip')
    with zipfile.ZipFile(output, 'x', compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json', json.dumps(report, indent=2) + '\n')
    print('\nSonuç: ' + report['outcome'] + '\nGönderilecek dosya: ' + str(output), flush=True)
    return 0 if report['outcome'] == 'bounded_window_checks_passed_with_owner_observations' and report.get('runtime_cleanup_verified') else 1


if __name__ == '__main__':
    sys.exit(main())
