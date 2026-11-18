"""Inspect the interrupted acceptance run without capture, settings update or fault injection."""
import hashlib
import json
import os
from pathlib import Path
import platform
import threading
import uuid
import zipfile

import lan_device_recovery as r
from mac_identity_preflight import unique_object

TARGET_RUN = '4bbd029c-2ab2-426f-aec5-725b67cbcaf7'
QUEUED_BASELINE = '58400d9ac30516f4d3a502d7365ec16a4444b540fc03162d3717ee095b023e5e'
QUEUED_BASELINE_RECORDS = 11
QUEUED_RUN = '813cf272-39bd-4cbf-82a9-ddf75e48c7ac'
STAGES = ['inspect_existing_profile','inspection_completed']
FIELDS = {'schema','purpose','run_id','brain_id','local_device_id','peer_device_id','outcome','error',
          'capture_invoked','settings_updated','process_interrupted'}


def parse_inspection(output, code, plan, local, peer, *, start_only=False, queued_note=False):
    if not isinstance(output,str) or len(output)>65536:
        raise r.Refused('inspection_output_bound')
    stages, observations, receipt, cleanup = [], [], None, False
    current_failure = None
    last_settings_error = None
    purpose = 'cryptex-device-recovery-inspection'
    if queued_note:
        purpose = 'cryptex-queued-note-inspection'
    elif start_only:
        purpose = 'cryptex-connection-start-inspection'
    for line in output.splitlines():
        preceding_error = last_settings_error
        last_settings_error = None
        if line == 'recovery_check_refused: recovery_incomplete':
            continue
        if line == 'recovery_cleanup_unconfirmed':
            cleanup = True
            continue
        value=json.loads(line,object_pairs_hook=unique_object)
        if receipt is not None:
            raise r.Refused('trailing_inspection_output')
        if isinstance(value,dict) and set(value)=={'recovery_stage'}:
            stages.append(value['recovery_stage'])
            if stages != STAGES[:len(stages)]:
                raise r.Refused('inspection_stage_order')
        elif isinstance(value,dict) and set(value)=={'recovery_observation'}:
            item = r.validate_observation(value['recovery_observation'])
            observations.append(item)
            if item['step'] == 'connections_read' and item['outcome'] == 'error':
                last_settings_error = item['code']
        elif isinstance(value,dict) and set(value)=={'current_start_failure'}:
            failure = value['current_start_failure']
            labels = json.loads(Path(__file__).with_name('worker_failure_codes.json').read_text(encoding='utf-8'))
            if (local != r.WINDOWS or current_failure is not None
                    or not isinstance(failure, dict) or set(failure) != {'stage','code','detail'}
                    or failure['stage'] not in ('bootstrap','exchange','identity_read','identity_mismatch','monitor')
                    or type(failure['code']) is not int or str(failure['code']) not in labels
                    or (failure['detail'] is not None and (type(failure['detail']) is not int
                        or not -(2**63) <= failure['detail'] < 2**63))):
                raise r.Refused('invalid_current_start_failure')
            deadline = failure['stage'] == 'exchange' and failure['code'] == 402
            if preceding_error != ('settings_start_rest_deadline' if deadline else 'settings_start'):
                raise r.Refused('invalid_current_start_failure')
            current_failure = dict(failure, label=labels[str(failure['code'])])
        else:
            if (not isinstance(value,dict) or set(value)!=FIELDS
                    or type(value['schema']) is not int or value['schema']!=1
                    or value['purpose']!=purpose
                    or value['run_id']!=plan['run_id'] or value['brain_id']!=r.BRAIN
                    or value['local_device_id']!=local or value['peer_device_id']!=peer
                    or any(value[k] is not False for k in ('capture_invoked','settings_updated','process_interrupted'))
                    or not ((value['outcome']=='inspection_completed' and value['error'] is None)
                            or (value['outcome']=='incomplete' and value['error'] in r.ERRORS))):
                raise r.Refused('invalid_inspection_receipt')
            receipt=value
    if receipt is None or not stages:
        raise r.Refused('missing_inspection_receipt')
    passed=receipt['outcome']=='inspection_completed'
    if queued_note and passed:
        expected = [('connections_read', 'ready')]
        if local == r.MAC:
            # The Mac driver reports successful IPC before validating the capture receipt.
            expected.extend([('resolve_queued', 'ok'), ('resolve_queued', 'committed')])
        expected.extend([('queued_before_restart','found'), ('queued_after_restart','found')])
        required_steps = {step for step, _ in expected}
        observed = [(v['step'],v['outcome']) for v in observations
                    if v['step'] in required_steps and v['outcome'] != 'begin']
        if observed != expected or plan['run_id'] != QUEUED_RUN:
            raise r.Refused('incomplete_inspection_evidence')
    if start_only and passed and not any(item['step']=='connections_read' and item['outcome'] in ('ready','paused')
                                         for item in observations):
        raise r.Refused('incomplete_inspection_evidence')
    if passed and (code!=0 or cleanup or stages!=STAGES or any(v['outcome']=='error' for v in observations)):
        raise r.Refused('incomplete_inspection_evidence')
    if not passed and code==0:
        raise r.Refused('inspection_exit_mismatch')
    result = dict(receipt=receipt,stages=stages,observations=observations,cleanup_unconfirmed=cleanup)
    if current_failure is not None:
        result['current_start_failure'] = current_failure
    return result


def verify(execute, args, plan, local, peer):
    code,output=execute(args,90)
    if code:
        raise r.Refused('stopped_profile_verification_refused')
    return r.parse_output(output,plan,local,peer)


def include_windows_start_evidence(report, root, system, driver, collector=None):
    if system != 'Windows' or not any(
            item['step'] == 'connections_read' and item['outcome'] == 'error'
            and item['code'] in ('settings_start', 'settings_start_rest_deadline') for item in driver['observations']):
        return
    try:
        if collector is None:
            from windows_transport_failure_diagnostic import collect as collector
        report['windows_transport'] = collector(root)
    except Exception:
        # Optional disk evidence must never overwrite the native result.
        report['windows_transport'] = {'outcome': 'unavailable'}


def collect(package, plan, system, report, prepare=r.prepare, *, start_only=False, queued_note=False):
    desktop,daemon,root,execute=prepare(package,plan,report,system)
    local,peer=(r.WINDOWS,r.MAC) if system=='Windows' else (r.MAC,r.WINDOWS)
    report['desktop_sha256']=hashlib.sha256(desktop.read_bytes()).hexdigest()
    report['daemon_sha256']=hashlib.sha256(daemon.read_bytes()).hexdigest()
    args=r.verify_command(daemon,root,plan,local,peer)
    report['before']=verify(execute,args,plan,local,peer)
    if queued_note and system == 'Darwin':
        print('MAC HAZIR. Simdi Windows VM icinde WINDOWS-KONTROL.cmd dosyasini ac.', flush=True)
    print('Mevcut kayitlar dogrulandi. Baglanti baslatma durumu okunuyor.',flush=True)
    finished=threading.Event()
    def progress():
        while not finished.wait(30):
            print('Tani devam ediyor; terminali acik birak.',flush=True)
    heartbeat=threading.Thread(target=progress,daemon=True)
    heartbeat.start()
    try:
        # Two resolve deadlines, native settings admission, four guarded reads
        # and normal owned shutdown must fit before the outer cleanup bound.
        flag = '--cryptex-inspect-connection-start' if start_only else '--cryptex-device-recovery-inspect'
        if queued_note:
            flag = '--cryptex-inspect-queued-note'
        code,output=execute([desktop,flag,plan['run_id'],r.BRAIN,local,peer],300 if start_only else 900)
        report['driver']=parse_inspection(output,code,plan,local,peer,start_only=start_only,queued_note=queued_note)
        include_windows_start_evidence(report, root, system, report['driver'])
    finally:
        finished.set()
        heartbeat.join(2)
        # Always collect post-run state, including a refused native startup.
        # No expectation of ten records: the prior Mac attempt may have written.
        report['after']=verify(execute,args,plan,local,peer)
    before,after=report['before'],report['after']
    report['same_local_set_before_after']=all(before[k]==after[k] for k in
        ('canonical_set_sha256','canonical_records','canonical_devices'))
    report['outcome']='diagnostics_collected'
    report['native_inspection_completed']=report['driver']['receipt']['outcome']=='inspection_completed'
    if queued_note and report['native_inspection_completed']:
        if after['canonical_records'] <= QUEUED_BASELINE_RECORDS or after['canonical_set_sha256'] == QUEUED_BASELINE:
            raise r.Refused('queued_note_set_inconsistent')
        report['queued_note_verified_after_restart'] = True


def support_line(report):
    # A short handoff for the operator without copying the ZIP or raw logs.
    fields = ['TANI', report['os'], report.get('run_id', '')[:8]]
    for key in ('before', 'after'):
        if key in report:
            fields.append(key + '=' + str(report[key]['canonical_records']))
    driver = report.get('driver')
    if driver:
        fields.append('inspection=' + driver['receipt']['outcome'])
        if driver['receipt']['error']:
            fields.append('error=' + driver['receipt']['error'])
        if driver['cleanup_unconfirmed']:
            fields.append('cleanup=unconfirmed')
        for item in driver['observations']:
            if item['step'] == 'connections_read' and item['outcome'] != 'begin':
                fields.append('connection=' + (item['code'] or item['outcome']))
                if item['code'] == 'settings_start_rest_deadline':
                    fields.append('bu_baslatmada_402=dogrulandi')
                elif item['code'] == 'settings_start' and 'current_start_failure' not in driver:
                    fields.append('bu_baslatmada_402=belirlenemedi')
                break
        if 'current_start_failure' in driver:
            current = driver['current_start_failure']
            fields.append(f"guncel_hata={current['stage']}/{current['code']}/{current['label']}")
            if current['detail'] is not None:
                fields.append(f"alt_kod={current['detail']}")
        retained = report.get('windows_transport', {}).get('first_worker_failure', {})
        if retained.get('status') == 'observed' and retained['value']['code'] == 402:
            fields.append('saklanan_ilk_hata=402')
        first = next((item for item in driver['observations'] if item['outcome'] == 'error'), None)
        if first:
            fields.append('first=' + first['step'] + '/' + (first['code'] or 'error'))
    if report.get('error'):
        fields.append('collector=' + report['error'])
    return ' '.join(fields)


def main(target_run=TARGET_RUN, *, start_only=False, queued_note=False):
    os.umask(0o077)
    package,system=Path(__file__).resolve().parent,platform.system()
    report=dict(schema=1,purpose='cryptex-recovery-inspection-result',os=system,outcome='incomplete',
                credentials_exported=False,old_app_changed=False,pairing_changed=False,
                recovery_accepted=False)
    try:
        plan=r.read_plan(package,'inspection.json','cryptex-device-recovery-inspection-packet')
        if plan['run_id']!=target_run:
            raise r.Refused('wrong_inspection_target')
        report.update(run_id=plan['run_id'],source_commit=plan['source_commit'])
        collect(package,plan,system,report,start_only=start_only,queued_note=queued_note)
    except Exception as error:
        # Errors from arbitrary tools/OS never leave as unbounded text.
        safe={'wrong_inspection_target','invalid_recovery_plan','inspection_output_bound',
              'invalid_observation','inspection_stage_order','trailing_inspection_output',
              'invalid_inspection_receipt','missing_inspection_receipt','incomplete_inspection_evidence','invalid_current_start_failure',
              'inspection_exit_mismatch','stopped_profile_verification_refused','quit_cryptex_before_update',
              'wrong_mac_profile','wrong_vm_profile','standard_user_required','bounded_command_incomplete',
              'queued_note_set_inconsistent'}
        report.update(outcome='incomplete',error=str(error) if str(error) in safe else 'inspection_unavailable')
    prefix = 'BEKLEYEN-NOT-SONUC-' if queued_note else 'KESINTI-TANI-SONUC-'
    output=package/(prefix+system+'-'+uuid.uuid4().hex[:12]+'.zip')
    with zipfile.ZipFile(output,'x',zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json',json.dumps(report,indent=2)+'\n')
    print('\nAyrintili sonuc: '+str(output),flush=True)
    if queued_note:
        print('Mevcut not yeniden acilista dogrulandi.' if report.get('queued_note_verified_after_restart')
              else 'Not dogrulamasi tamamlanamadi.', flush=True)
        print('Karsilastirma icin bu cihazda olusan sonuc ZIP dosyasini paylas.', flush=True)
        return 0 if report.get('queued_note_verified_after_restart') else 2
    print('Asagidaki TANI satirini paylasman yeterli. Eski kesinti paketini tekrar calistirma.',flush=True)
    print(support_line(report),flush=True)
    return 0 if report['outcome']=='diagnostics_collected' else 2


if __name__=='__main__':
    raise SystemExit(main())
