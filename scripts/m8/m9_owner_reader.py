"""Install the tested reader beside the existing app and observe normal sync."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time
import uuid
import zipfile

from lan_transfer_diagnostic import BRAIN, MAC, WINDOWS, Refused, directory, read_metadata, unique
from m9_owner_migration import get_app, require_protected_layout, verify_identity

SIDES = {'Darwin': (MAC, WINDOWS), 'Windows': (WINDOWS, MAC)}
RESULT = 'cryptex-owner-m9-reader-result'
EXPECTED_BLOB = hashlib.sha256(b'abc').hexdigest()


def hex_string(value, length=64):
    return isinstance(value, str) and len(value) == length and re.fullmatch('[a-f0-9]+', value) is not None


def read_plan(package):
    plan = read_metadata(package / 'candidate.json')
    if (plan.get('purpose') != 'cryptex-owner-m9-reader' or plan.get('schema') != 1
            or not hex_string(plan.get('source_commit'), 40) or not hex_string(plan.get('source_sha256'))
            or str(uuid.UUID(plan['run_id'])) != plan['run_id']
            or not hex_string(plan['hashes'].get('cryptexd.exe'))):
        raise Refused('invalid_reader_plan')
    return plan


def validate(receipt, plan, system, attempt, local_build):
    local, peer = SIDES[system]
    if (set(receipt) != {'schema', 'purpose', 'attempt', 'brain', 'local', 'peer', 'local_nonce',
                         'peer_nonce', 'local_build', 'peer_build', 'resolved_blob'}
            or type(receipt['schema']) is not int or receipt['schema'] != 1
            or receipt['purpose'] != 'cryptex-verified-m9-reader'
            or not hex_string(attempt, 32) or receipt['attempt'] != attempt
            or (receipt['brain'], receipt['local'], receipt['peer']) != (BRAIN, local, peer)
            or receipt['resolved_blob'] != EXPECTED_BLOB
            or any(not hex_string(receipt[k]) for k in ('local_nonce', 'peer_nonce', 'local_build', 'peer_build'))
            or receipt['local_nonce'] == receipt['peer_nonce'] or receipt['local_build'] != local_build
            or receipt['local_build' if system == 'Windows' else 'peer_build'] != plan['hashes']['cryptexd.exe']):
        raise Refused('reader_receipt_mismatch')
    return receipt


def compare(first, second, plan):
    if {first.get('os'), second.get('os')} != set(SIDES):
        raise Refused('two_platforms_required')
    receipts = []
    for item in (first, second):
        if (item.get('schema') != 1 or item.get('purpose') != RESULT
                or item.get('outcome') != 'local_peer_verified' or item.get('run_id') != plan['run_id']
                or item.get('production_v2_activation') != 'blocked'
                or item.get('source_commit') != plan['source_commit']):
            raise Refused('incomplete_or_different_run')
        receipts.append(validate(item['receipt'], plan, item['os'], item['attempt'], item['daemon_sha256']))
    a, b = receipts
    if (a['attempt'] == b['attempt'] or any(a['local_' + k] != b['peer_' + k]
            or b['local_' + k] != a['peer_' + k] for k in ('build', 'nonce'))):
        raise Refused('different_reader_sessions')
    return True


def read_archive(path):
    if path.stat().st_size > 32768:
        raise Refused('result_archive_bound')
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        if len(entries) != 1 or entries[0].filename != 'result.json' or entries[0].file_size > 16384:
            raise Refused('invalid_result_archive')
        value = json.loads(archive.read(entries[0]), object_pairs_hook=unique)
    if not isinstance(value, dict):
        raise Refused('invalid_result_archive')
    return value


def wait_for_receipt(path, process, report, plan, *, timeout=600, clock=time.monotonic, sleep=time.sleep):
    start = clock()
    progress = start + 30
    while clock() - start < timeout:
        if process.poll() is not None:
            raise Refused('desktop_exited_before_verification')
        if path.exists():
            return validate(read_metadata(path), plan, report['os'], report['attempt'], report['daemon_sha256'])
        if clock() >= progress:
            print('Imzali okuyucu yaniti bekleniyor (%d sn).' % int(clock() - start), flush=True)
            progress = clock() + 30
        sleep(1)
    raise Refused('peer_verification_timeout')


def run_local(package, plan, system, report):
    if system not in SIDES:
        raise Refused('unsupported_platform')
    windows = system == 'Windows'
    if windows:
        if ctypes.windll.shell32.IsUserAnAdmin():
            raise Refused('standard_vm_user_required')
        from windows_setup_native import Pins, run
        from windows_vm_update import verify
        base = Path(os.environ['LOCALAPPDATA']) / 'Cryptex-Dev'
    else:
        if os.geteuid() == 0 or int(platform.mac_ver()[0].split('.')[0]) < 26:
            raise Refused('standard_macos_26_user_required')
        from mac_existing_update import require_closed
        base = Path.home() / 'Library/Application Support/Cryptex-Dev'
    root = base / 'runtime/p0-2'
    state = base / ('m9-reader-' + plan['run_id'])
    local, _peer = SIDES[system]
    with directory(root):
        verify_identity(root, local)
        require_protected_layout(root)
        report['stage'] = 'check_closed'
        if windows:
            shell = Path(os.environ['SYSTEMROOT']) / 'System32/WindowsPowerShell/v1.0/powershell.exe'
            code, _ = run([shell, '-NoProfile', '-NonInteractive', '-Command',
                           'if (Get-Process cryptex-desktop,cryptexd -ErrorAction SilentlyContinue) {exit 2}; exit 0'],
                          20, capture_status=True)
            if code:
                raise Refused('quit_cryptex_before_update')
        else:
            require_closed()
        with directory(base):
            state.mkdir(mode=0o700, exist_ok=True)
        with directory(state):
            if not windows and (state.stat().st_uid != os.getuid() or state.stat().st_mode & 0o077):
                raise Refused('private_state_required')
            report['stage'] = 'prepare_reader'
            desktop, daemon = get_app(package, base, state, plan, system, False)
            attempt = uuid.uuid4().hex
            receipt = root / 'transport/m9-control-staging' / (attempt + '.receipt')
            if os.path.lexists(receipt):
                raise Refused('unexpected_existing_receipt')
            env = {k: v for k, v in os.environ.items() if not k.upper().startswith('CRYPTEX')}
            env['CRYPTEX_M9_READER_ATTEMPT'] = attempt
            app_guard = Pins() if windows else directory(daemon.parent)
            with app_guard as pin:
                if windows:
                    verify(desktop.parent, plan, pin)
                report.update(attempt=attempt, daemon_sha256=hashlib.sha256(daemon.read_bytes()).hexdigest(),
                              stage='start_normal_desktop')
                child = subprocess.Popen([str(desktop)], cwd=desktop.parent, env=env, close_fds=True,
                                         stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    label = 'VM HAZIR.' if windows else 'MAC HAZIR. Windows VM icinde M9-OKUYUCU-VM.cmd dosyasini simdi ac.'
    print(label + ' Iki Cryptex de acik kalsin.', flush=True)
    print('Open local memory veya Keychain izni sorulursa cihazda yanitla.', flush=True)
    settings = root / 'connection-settings/connections.json'
    if settings.exists() and read_metadata(settings).get('paused') is True:
        print('Sync paused. Local connections ekraninda Pause sync isaretini kaldirip kaydet.', flush=True)
    report['stage'] = 'wait_for_paired_reader'
    result = wait_for_receipt(receipt, child, report, plan)
    report.update(receipt=result, outcome='local_peer_verified', stage='complete')


def main():
    os.umask(0o077)
    package = Path(__file__).resolve().parent
    system = platform.system()
    if len(sys.argv) == 4 and sys.argv[1] == '--compare':
        plan = read_plan(package)
        compare(read_archive(Path(sys.argv[2])), read_archive(Path(sys.argv[3])), plan)
        print('Iki cihaz ayni oturumda birbirinin M9 okuyucusunu dogruladi.')
        return 0
    if len(sys.argv) != 1:
        return 2
    report = dict(schema=1, purpose=RESULT, os=system, outcome='incomplete', stage='preflight',
                  production_v2_activation='blocked')
    try:
        plan = read_plan(package)
        report.update(run_id=plan['run_id'], source_commit=plan['source_commit'])
        run_local(package, plan, system, report)
    except Exception as error:
        code = str(error)
        report['error'] = code if re.fullmatch('[a-z0-9_]{1,80}', code) else 'reader_check_unavailable'
        print('Kontrol tamamlanamadi: ' + report['error'], flush=True)
    output = package / ('M9-OKUYUCU-SONUC-' + system + '-' + uuid.uuid4().hex[:12] + '.zip')
    with zipfile.ZipFile(output, 'x', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json', json.dumps(report, indent=2) + '\n')
    print('Gonderilecek sonuc: ' + str(output), flush=True)
    if report['outcome'] != 'local_peer_verified':
        return 2
    print('Bu cihaz karsi cihazin M9 okuyucusunu dogruladi.', flush=True)
    other_system = 'Windows' if system == 'Darwin' else 'Darwin'
    for other in package.glob('M9-OKUYUCU-SONUC-' + other_system + '-*.zip'):
        try:
            compare(report, read_archive(other), plan)
        except (Refused, ValueError, KeyError, TypeError, OSError, zipfile.BadZipFile):
            continue
        print('Iki cihazin ayni oturumdaki karsilikli dogrulamasi tamamlandi.', flush=True)
        break
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
