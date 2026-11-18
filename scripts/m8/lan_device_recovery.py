"""Run the explicit Desktop recovery check; collect only closed public receipts."""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import threading
import time
import uuid
import zipfile

from lan_transfer_diagnostic import BRAIN, MAC, WINDOWS, Refused, read_metadata
from lan_canonical_verification import command as verify_command, parse_output, validate_receipt, compare
from mac_identity_preflight import unique_object

BASELINE = '8f18e7737d78aa78d1c07fb7df99ec04ea0f767360398e42fd47af079e6ed7b7'
STAGES = ['start_existing_profile', 'capture_while_disconnected', 'interrupt_owned_daemon_after_start',
          'restart_and_receive', 'allow_peer_receipt_delivery', 'offline_restart', 'completed']
ERRORS = {'arguments', 'existing_profile_required', 'startup', 'identity', 'connections',
          'capture_uncertain', 'search', 'peer_deadline', 'interruption', 'cleanup'}
DRIVER_FIELDS = {'schema', 'purpose', 'run_id', 'brain_id', 'local_device_id', 'peer_device_id',
                 'outcome', 'error', 'midstream_bytes_interrupted', 'remote_durability_verified'}


def validate_driver(value, plan, local, peer):
    if (not isinstance(value, dict) or set(value) != DRIVER_FIELDS
            or type(value['schema']) is not int or value['schema'] != 1
            or value['purpose'] != 'cryptex-device-recovery' or value['run_id'] != plan['run_id']
            or value['brain_id'] != BRAIN or value['local_device_id'] != local or value['peer_device_id'] != peer
            or value['midstream_bytes_interrupted'] is not False
            or value['remote_durability_verified'] is not False
            or not ((value['outcome'] == 'local_steps_passed' and value['error'] is None)
                    or (value['outcome'] == 'incomplete' and value['error'] in ERRORS))):
        raise Refused('invalid_recovery_receipt')
    return value


def parse_driver(output, code, plan, local, peer):
    if not isinstance(output, str) or len(output) > 65536:
        raise Refused('recovery_output_bound')
    stages, receipt, cleanup_unknown = [], None, False
    for line in output.splitlines():
        if line == 'recovery_check_refused: recovery_incomplete':
            continue
        if line == 'recovery_cleanup_unconfirmed':
            cleanup_unknown = True
            continue
        value = json.loads(line, object_pairs_hook=unique_object)
        if isinstance(value, dict) and set(value) == {'recovery_stage'} and receipt is None:
            stages.append(value['recovery_stage'])
            if stages != STAGES[:len(stages)]:
                raise Refused('recovery_stage_order')
        elif receipt is None:
            receipt = validate_driver(value, plan, local, peer)
        else:
            raise Refused('duplicate_recovery_receipt')
    if receipt is None or not stages:
        raise Refused('missing_recovery_receipt')
    passed = receipt['outcome'] == 'local_steps_passed'
    if passed and (code != 0 or stages != STAGES or cleanup_unknown):
        raise Refused('incomplete_recovery_evidence')
    if not passed and code == 0:
        raise Refused('recovery_exit_mismatch')
    return dict(receipt=receipt, stages=stages, cleanup_unconfirmed=cleanup_unknown)


def read_plan(package):
    plan = read_metadata(package/'recovery.json')
    keys = {'schema', 'purpose', 'run_id', 'source_commit', 'source_sha256', 'hashes'}
    from windows_vm_update import FILES
    if (set(plan) != keys or type(plan['schema']) is not int or plan['schema'] != 1
            or plan['purpose'] != 'cryptex-device-recovery-packet'
            or str(uuid.UUID(plan['run_id'])) != plan['run_id']
            or not re.fullmatch('[a-f0-9]{40}', plan['source_commit'])
            or not re.fullmatch('[a-f0-9]{64}', plan['source_sha256'])
            or not isinstance(plan['hashes'], dict) or set(plan['hashes']) != FILES
            or any(not isinstance(v, str) or not re.fullmatch('[a-f0-9]{64}', v) for v in plan['hashes'].values())):
        raise Refused('invalid_recovery_plan')
    return plan


def mac_builder(source, work, commit, run, report):
    from mac_lan_setup import build
    return build(source, work, commit, run, report, recovery_check=True)


def prepare(package, plan, report, system):
    if system == 'Windows':
        import ctypes
        if ctypes.windll.shell32.IsUserAnAdmin():
            raise Refused('standard_user_required')
        from windows_vm_update import install
        from windows_setup_native import run
        base = Path(os.environ['LOCALAPPDATA'])
        app = install(package, base, dict(hashes=plan['hashes'], product_source=plan['source_commit'],
                                          tool_source=plan['source_commit']))
        def execute(args, timeout):
            return run(args, timeout, capture_status=True)
        return app/'cryptex-desktop.exe', app/'cryptexd.exe', base/'Cryptex-Dev/runtime/p0-2', execute
    if system == 'Darwin':
        if os.geteuid() == 0 or int(platform.mac_ver()[0].split('.')[0]) < 26:
            raise Refused('standard_macos_26_user_required')
        from mac_existing_update import prepare as build_existing
        from mac_identity_preflight import run_bounded
        desktop = build_existing(package, Path.home(), plan, report, builder=mac_builder, create_launcher=False)
        def execute(args, timeout):
            outcome = run_bounded([str(a) for a in args], timeout)
            if outcome.timed_out or outcome.limited or outcome.pipe_failed:
                raise Refused('bounded_command_incomplete')
            return outcome.code, outcome.output
        return desktop, desktop.parent/'cryptexd', Path.home()/'Library/Application Support/Cryptex-Dev/runtime/p0-2', execute
    raise Refused('unsupported_platform')


def check_baseline(receipt):
    if (receipt['canonical_records'] != 10 or receipt['canonical_devices'] != 2
            or receipt['canonical_set_sha256'] != BASELINE):
        raise Refused('baseline_changed_do_not_repeat')


def check_result(report, plan, local, peer):
    if (report.get('outcome') != 'local_recovery_steps_verified' or report.get('run_id') != plan['run_id']
            or report.get('source_commit') != plan['source_commit']):
        raise Refused('incomplete_device_report')
    before = validate_receipt(report['before'], plan['run_id'], local, peer)
    check_baseline(before)
    driver = report['driver']
    validate_driver(driver['receipt'], plan, local, peer)
    if driver['receipt']['outcome'] != 'local_steps_passed' or driver['stages'] != STAGES or driver['cleanup_unconfirmed'] is not False:
        raise Refused('incomplete_device_steps')
    after = validate_receipt(report['after'], plan['run_id'], local, peer)
    if after['canonical_records'] != 14 or after['canonical_devices'] != 2:
        raise Refused('unexpected_post_test_set')
    return after


def compare_results(windows, mac, plan):
    a = check_result(windows, plan, WINDOWS, MAC)
    b = check_result(mac, plan, MAC, WINDOWS)
    matched = compare(a, b, plan['run_id'])['same_verified_canonical_set']
    return dict(schema=1, purpose='cryptex-device-recovery-comparison', run_id=plan['run_id'],
                queued_work_process_interruption_recovered=matched, same_verified_canonical_set=matched,
                offline_restart_search_verified=matched, midstream_bytes_interrupted=False,
                network_power_loss_tested=False, remote_protection_claim=False)


def collect(package, plan, system, report):
    desktop, daemon, root, execute = prepare(package, plan, report, system)
    local, peer = (WINDOWS, MAC) if system == 'Windows' else (MAC, WINDOWS)
    report['desktop_sha256'] = hashlib.sha256(desktop.read_bytes()).hexdigest()
    report['daemon_sha256'] = hashlib.sha256(daemon.read_bytes()).hexdigest()
    args = verify_command(daemon, root, plan, local, peer)
    code, output = execute(args, 90)
    if code:
        raise Refused('baseline_verification_refused')
    report['before'] = parse_output(output, plan, local, peer)
    check_baseline(report['before'])
    print('\nDerleme ve on kontrol tamam. Diger cihazda da KESINTI paketini baslat.\n'
          'Not girme veya Search yapma gerekmiyor. Terminal acik, cihazlar uyanik kalsin.\n'
          'Her cihaz iki kisa Public test kaydi ekleyecek; bunlar testten sonra kalacak.', flush=True)
    # A fixed progress heartbeat is useful even while native runners retain
    # output privately. No raw process output or note text is printed/exported.
    finished = threading.Event()
    started = time.monotonic()
    def heartbeat():
        while not finished.wait(30):
            print('Otomatik kontrol devam ediyor (' + str(int(time.monotonic()-started)) + ' sn).', flush=True)
    thread = threading.Thread(target=heartbeat, daemon=True)
    thread.start()
    try:
        code, output = execute([desktop, '--cryptex-device-recovery-check', plan['run_id'], BRAIN, local, peer], 3000)
    finally:
        finished.set()
        thread.join(2)
    report['driver'] = parse_driver(output, code, plan, local, peer)
    if report['driver']['receipt']['outcome'] != 'local_steps_passed':
        raise Refused('native_steps_incomplete')
    code, output = execute(args, 90)
    if code:
        raise Refused('final_verification_refused')
    report['after'] = parse_output(output, plan, local, peer)
    report['outcome'] = 'local_recovery_steps_verified'
    check_result(report, plan, local, peer)


def main():
    os.umask(0o077)
    package, system = Path(__file__).resolve().parent, platform.system()
    report = dict(schema=1, purpose='cryptex-device-recovery-result', os=system, outcome='incomplete',
                  credentials_exported=False, old_app_changed=False, pairing_changed=False,
                  midstream_bytes_interrupted=False, remote_protection_claim=False)
    try:
        plan = read_plan(package)
        report.update(run_id=plan['run_id'], source_commit=plan['source_commit'])
        collect(package, plan, system, report)
    except Exception as error:
        code = str(error)
        report.update(outcome='incomplete', error=code if re.fullmatch('[a-z_]{1,100}', code) else 'recovery_unavailable')
    output = package/('KESINTI-SONUC-'+system+'-'+uuid.uuid4().hex[:12]+'.zip')
    with zipfile.ZipFile(output, 'x', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json', json.dumps(report, indent=2)+'\n')
    print('\nGonderilecek sonuc: '+str(output), flush=True)
    print('Iki cihaz raporu birlikte kontrol edilmeden test tamamlandi sayilmaz.\n'
          'Paketi tekrar calistirma; sonucu gonder. Normal Cryptex kisayolun korunuyor.', flush=True)
    return 0 if report['outcome'] == 'local_recovery_steps_verified' else 2


if __name__ == '__main__':
    raise SystemExit(main())
