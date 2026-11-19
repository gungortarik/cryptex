"""Resume the existing four markers and compare stopped, verified receipts."""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import threading
import uuid
import zipfile

import lan_device_recovery as r
from lan_recovery_inspection import verify
from lan_transfer_diagnostic import collect as disk_inventory

OLD_DIGEST = {
    r.WINDOWS: 'bb6481fc541581f088cdd1caa77fac04917d6a288178c031b84f8787be5d5abf',
    r.MAC: 'a7afd7085e3b8974487a7efa3fe1844b392299924bda8fafe5a48789898d0e0e',
}
OFFLINE = ['offline_local_queued', 'offline_local_received', 'offline_peer_queued', 'offline_peer_received']


def check_before(receipt, local):
    if receipt['canonical_devices'] != 2:
        raise r.Refused('unexpected_initial_set')
    count = receipt['canonical_records']
    if count != 16 and (count != 15 or receipt['canonical_set_sha256'] != OLD_DIGEST[local]):
        raise r.Refused('unexpected_initial_set')


def check_local(report, plan, local, peer):
    system = 'Windows' if local == r.WINDOWS else 'Darwin'
    if (report.get('purpose') != 'cryptex-existing-marker-followup-result' or report.get('os') != system
            or plan['run_id'] != r.FOLLOWUP_RUN or report.get('run_id') != plan['run_id']
            or report.get('source_commit') != plan['source_commit']):
        raise r.Refused('unexpected_followup_identity')
    check_before(r.validate_receipt(report['before'], plan['run_id'], local, peer), local)
    driver = report['driver']
    state = r.validate_driver(driver['receipt'], plan, local, peer, followup=True)
    if (state['outcome'] != 'local_steps_passed' or driver['stages'] != r.FOLLOWUP_STAGES
            or driver['cleanup_unconfirmed'] is not False):
        raise r.Refused('incomplete_followup_steps')
    observations = [r.validate_observation(item) for item in driver['observations']]
    if (any(item['outcome'] == 'error' for item in observations)
            or [item['step'] for item in observations if item['outcome'] == 'committed'] != ['resolve_queued', 'resolve_received']
            or [item['step'] for item in observations if item['outcome'] == 'found'] != OFFLINE):
        raise r.Refused('incomplete_followup_steps')
    after = r.validate_receipt(report['after'], plan['run_id'], local, peer)
    if after['canonical_devices'] != 2 or after['canonical_records'] != 16:
        raise r.Refused('unexpected_final_set')
    if report['before']['canonical_records'] == 16 and report['before']['canonical_set_sha256'] != after['canonical_set_sha256']:
        raise r.Refused('unexpected_final_set')
    return after


def compare_results(windows, mac, plan):
    for report in (windows, mac):
        if report.get('outcome') != 'verified_on_this_device':
            raise r.Refused('incomplete_followup_steps')
    a = check_local(windows, plan, r.WINDOWS, r.MAC)
    b = check_local(mac, plan, r.MAC, r.WINDOWS)
    return r.compare(a, b, plan['run_id'])['same_verified_canonical_set']


def collect(package, plan, system, report, prepare=r.prepare):
    desktop, daemon, root, execute = prepare(package, plan, report, system)
    local, peer = (r.WINDOWS, r.MAC) if system == 'Windows' else (r.MAC, r.WINDOWS)
    report['desktop_sha256'] = hashlib.sha256(desktop.read_bytes()).hexdigest()
    report['daemon_sha256'] = hashlib.sha256(daemon.read_bytes()).hexdigest()
    args = r.verify_command(daemon, root, plan, local, peer)
    report['before'] = verify(execute, args, plan, local, peer)
    check_before(report['before'], local)
    if system == 'Darwin':
        print('MAC HAZIR. Windows VM icinde WINDOWS-KONTROL.cmd dosyasini simdi ac.', flush=True)
    print('Mevcut dort not kontrol ediliyor. Terminal acik kalsin.', flush=True)
    finished = threading.Event()

    def progress():
        while not finished.wait(30):
            print('Aktarim ve cevrimdisi acilis kontrolu devam ediyor.', flush=True)

    worker = threading.Thread(target=progress, daemon=True)
    worker.start()
    try:
        code, output = execute([desktop, '--cryptex-resume-existing-markers', plan['run_id'], r.BRAIN, local, peer], 1500)
        report['driver'] = r.parse_driver(output, code, plan, local, peer, followup=True)
    finally:
        finished.set()
        worker.join()
        # This inventory is a disk snapshot. It never dials the peer.
        try:
            report['disk_inventory'] = disk_inventory(root, system, probe=None)
        except (OSError, ValueError, r.Refused):
            report['disk_inventory'] = {'outcome': 'unavailable'}
    if report['driver']['cleanup_unconfirmed'] or report['driver']['receipt']['error'] == 'cleanup':
        raise r.Refused('cleanup_unconfirmed')
    report['after'] = verify(execute, args, plan, local, peer)
    check_local(report, plan, local, peer)
    report['outcome'] = 'verified_on_this_device'


def main():
    os.umask(0o077)
    package = Path(__file__).resolve().parent
    system = platform.system()
    report = dict(schema=1, purpose='cryptex-existing-marker-followup-result', os=system, outcome='incomplete')
    try:
        plan = r.read_plan(package, 'followup.json', 'cryptex-existing-marker-followup-packet')
        if plan['run_id'] != r.FOLLOWUP_RUN:
            raise r.Refused('unexpected_followup_identity')
        report.update(run_id=plan['run_id'], source_commit=plan['source_commit'])
        collect(package, plan, system, report)
    except Exception as error:
        report['error'] = str(error) if isinstance(error, r.Refused) and re.fullmatch('[a-z_]{1,100}', str(error)) else 'followup_unavailable'
    output = package / ('TOPARLANMA-SONUC-' + system + '-' + uuid.uuid4().hex[:12] + '.zip')
    with zipfile.ZipFile(output, 'x', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json', json.dumps(report, indent=2) + '\n')
    passed = report['outcome'] == 'verified_on_this_device'
    print('Bu cihazda 16 kayit ve cevrimdisi acilis dogrulandi.' if passed else 'Kontrol tamamlanamadi.', flush=True)
    print('Karsilastirma icin bu dosyayi gonder: ' + str(output), flush=True)
    return 0 if passed else 2


if __name__ == '__main__':
    raise SystemExit(main())
