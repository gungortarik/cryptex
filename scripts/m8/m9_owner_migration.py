"""Offline layout rollout for the enrolled owner pair. Only cryptexd opens the profile."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import stat
import subprocess
import sys
import tempfile
import uuid
import zipfile

from lan_canonical_verification import command, parse_output, validate_receipt
from lan_transfer_diagnostic import BRAIN, MAC, WINDOWS, Refused, directory, read_metadata


# Last independently compared owner receipts (M8 recovery).
PREVIOUS_COUNT = 16
PREVIOUS_DIGEST = 'bd2c949fc008d2363caf2eb4d9a7bdc8f217e07fe3e3d698e130bc4f4cac3abd'
MIGRATED = 'profile_layout=m9 production_v2_activation=blocked'


def publish(path, value):
    """Publish once. A crash before publication leaves only an unused temp file."""
    fd, temp = tempfile.mkstemp(dir=path.parent)
    try:
        with os.fdopen(fd, 'w', encoding='utf-8') as out:
            json.dump(value, out)
            out.flush()
            os.fsync(out.fileno())
        # Neither platform can replace a baseline published by another run.
        if os.name == 'nt':
            os.rename(temp, path)
        else:
            libc = ctypes.CDLL('/usr/lib/libSystem.B.dylib', use_errno=True)
            if libc.renamex_np(os.fsencode(temp), os.fsencode(path), 4):  # RENAME_EXCL
                raise OSError(ctypes.get_errno(), 'receipt_publish_failed')
            with directory(path.parent) as parent:
                os.fsync(parent)
    finally:
        if os.path.exists(temp):
            os.unlink(temp)


def verify_identity(root, local):
    brain = read_metadata(root / 'identity/brain_identity.json')
    device = read_metadata(root / 'identity/local_device.json')
    if brain.get('brain_id') != BRAIN or (device.get('brain_id'), device.get('device_id')) != (BRAIN, local):
        raise Refused('wrong_enrolled_profile')


def require_protected_layout(root):
    lock = root / 'daemon.lock'
    if (not stat.S_ISDIR(lock.lstat().st_mode) or os.path.lexists(root / 'm9-lock.prepare')
            or not stat.S_ISREG((lock / 'reader-m9.pid').lstat().st_mode)):
        raise Refused('protected_layout_missing')


def transition(root, state, plan, local, peer, verify, migrate, report):
    baseline = state / 'before.json'
    def legacy_layout():
        return (not os.path.lexists(root / 'm9-lock.prepare')
                and stat.S_ISREG((root / 'daemon.lock').lstat().st_mode))

    report['stage'] = 'verify_before'
    if not os.path.lexists(baseline):
        if not legacy_layout():
            raise Refused('original_m8_receipt_required')
        before = validate_receipt(verify(), plan['run_id'], local, peer)
        count = before['canonical_records']
        if count < PREVIOUS_COUNT or (count == PREVIOUS_COUNT and before['canonical_set_sha256'] != PREVIOUS_DIGEST):
            raise Refused('earlier_verified_history_conflict')
        publish(baseline, before)
    else:
        before = validate_receipt(read_metadata(baseline), plan['run_id'], local, peer)
        if legacy_layout() and verify() != before:
            raise Refused('saved_baseline_changed')
    report['before'] = before
    report['stage'] = 'migrate_or_resume'
    if migrate().strip() != MIGRATED:
        raise Refused('migration_receipt_missing')
    require_protected_layout(root)
    report['profile_layout'] = 'm9'
    report['production_v2_activation'] = 'blocked'
    report['stage'] = 'verify_after'
    after = validate_receipt(verify(), plan['run_id'], local, peer)
    report['after'] = after
    if before != after:
        raise Refused('signed_history_changed')
    marker = state / 'verified.json'
    if not os.path.lexists(marker):
        publish(marker, after)
    if read_metadata(marker) != before:
        raise Refused('completion_receipt_conflict')
    report.update(stage='complete', outcome='migrated_and_verified')


def get_app(package, base, state, plan, system, opening):
    install_record = state / 'application.json'
    windows = system == 'Windows'
    parent = base / 'lan-apps' if windows else state
    if not os.path.lexists(install_record):
        if opening:
            raise Refused('migration_not_completed')
        if windows:
            from windows_vm_update import install
            app = install(package, base.parent, plan)
            record = {'folder': app.name}
        else:
            from mac_existing_update import prepare
            build = {}
            desktop = prepare(package, Path.home(), plan, build, create_launcher=False)
            app = state / uuid.uuid4().hex
            app.mkdir(mode=0o700)
            # Publish the signed bundle outside the purgeable build cache.
            desktop.parents[2].rename(app / 'Cryptex-LAN.app')
            binaries = app / 'Cryptex-LAN.app/Contents/MacOS'
            record = {'folder': app.name, 'hashes': {
                name: hashlib.sha256((binaries / name).read_bytes()).hexdigest()
                for name in ('cryptex-desktop', 'cryptexd')}}
        publish(install_record, record)
    record = read_metadata(install_record)
    folder = record.get('folder')
    if not isinstance(folder, str) or not re.fullmatch('[0-9a-f]{32}', folder):
        raise Refused('invalid_app_location')
    app = parent / folder
    if windows:
        return app / 'cryptex-desktop.exe', app / 'cryptexd.exe'
    binaries = app / 'Cryptex-LAN.app/Contents/MacOS'
    with directory(binaries):
        for name in ('cryptex-desktop', 'cryptexd'):
            if hashlib.sha256((binaries / name).read_bytes()).hexdigest() != record['hashes'][name]:
                raise Refused('installed_mac_product_changed')
    return binaries / 'cryptex-desktop', binaries / 'cryptexd'


def execute(package, system, opening, report):
    plan = read_metadata(package / 'candidate.json')
    if (plan.get('schema') != 1 or plan.get('purpose') != 'cryptex-owner-m9-layout'
            or not re.fullmatch('[0-9a-f]{40}', plan['source_commit'])
            or str(uuid.UUID(plan['run_id'])) != plan['run_id']):
        raise Refused('invalid_migration_plan')
    report.update(run_id=plan['run_id'], source_commit=plan['source_commit'])
    if system == 'Windows':
        if ctypes.windll.shell32.IsUserAnAdmin():
            raise Refused('standard_vm_user_required')
        from windows_setup_native import Pins, run
        from windows_vm_update import verify as verify_app, open_app
        base = Path(os.environ['LOCALAPPDATA']) / 'Cryptex-Dev'
        local, peer = WINDOWS, MAC
    elif system == 'Darwin':
        if os.geteuid() == 0 or int(platform.mac_ver()[0].split('.')[0]) < 26:
            raise Refused('standard_macos_26_user_required')
        from mac_identity_preflight import checked, run_bounded
        from mac_existing_update import require_closed
        base = Path.home() / 'Library/Application Support/Cryptex-Dev'
        local, peer = MAC, WINDOWS
    else:
        raise Refused('unsupported_platform')

    root = base / 'runtime/p0-2'
    state = base / ('m9-migration-' + plan['run_id'])
    with directory(root):
        verify_identity(root, local)
        if not opening:
            state.mkdir(mode=0o700, exist_ok=True)
        with directory(state):
            if system == 'Darwin' and (state.stat().st_uid != os.getuid() or state.stat().st_mode & 0o077):
                raise Refused('private_migration_state_required')
            if opening:
                validate_receipt(read_metadata(state / 'verified.json'), plan['run_id'], local, peer)
                require_protected_layout(root)
            report['stage'] = 'prepare_m9_app'
            print('M9 uygulamasi denetleniyor. Mac ilk calistirmada yerel imzali uygulamayi derler.', flush=True)
            desktop, daemon = get_app(package, base, state, plan, system, opening)

            def perform(runner):
                def verify():
                    return parse_output(runner(command(daemon, root, plan, local, peer)), plan, local, peer)

                def migrate():
                    verify_identity(root, local)
                    return runner([str(daemon), 'lifecycle', 'migrate-profile-m9', '--runtime-root', str(root)])

                transition(root, state, plan, local, peer, verify, migrate, report)

            if system == 'Windows':
                with Pins() as pins:
                    verify_app(desktop.parent, plan, pins)
                    if opening:
                        open_app(desktop.parent, plan)
                    else:
                        perform(lambda args: run(args, 180))
            else:
                with directory(daemon.parent):
                    if opening:
                        return subprocess.call([str(desktop)])
                    require_closed()
                    perform(lambda args: checked(run_bounded, args, 'daemon_command', 180))
    return 0


def main():
    os.umask(0o077)
    system = platform.system()
    package = Path(__file__).resolve().parent
    opening = sys.argv[1:] == ['--open']
    report = dict(schema=1, purpose='cryptex-owner-m9-layout-result', os=system, stage='preflight', outcome='refused')
    if sys.argv[1:] and not opening:
        return 2
    try:
        code = execute(package, system, opening, report)
    except Exception as error:
        label = str(error)
        report['error'] = label if re.fullmatch('[a-z0-9_]{1,100}', label) else 'operation_unavailable'
        if isinstance(getattr(error, 'winerror', None), int):
            report['winerror'] = error.winerror
        print('Islem durdu: ' + report['error'], flush=True)
        code = 2
    if not opening:
        output = package / ('M9-GECIS-SONUC-' + system + '-' + uuid.uuid4().hex[:12] + '.zip')
        with zipfile.ZipFile(output, 'x', zipfile.ZIP_DEFLATED) as archive:
            archive.writestr('result.json', json.dumps(report, indent=2) + '\n')
        print('Gonderilecek sonuc: ' + str(output), flush=True)
        if code == 0:
            print('M9 gecisi dogrulandi. Korunan kayit sayisi: ' + str(report['after']['canonical_records']))
    return code


if __name__ == '__main__':
    raise SystemExit(main())
