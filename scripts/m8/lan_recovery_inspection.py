"""Inspect the interrupted acceptance run without capture, settings update or fault injection."""
import hashlib
import json
import os
from pathlib import Path
import platform
import uuid
import zipfile

import lan_device_recovery as r
from mac_identity_preflight import unique_object

TARGET_RUN = '4bbd029c-2ab2-426f-aec5-725b67cbcaf7'
STAGES = ['inspect_existing_profile','inspection_completed']
FIELDS = {'schema','purpose','run_id','brain_id','local_device_id','peer_device_id','outcome','error',
          'capture_invoked','settings_updated','process_interrupted'}


def parse_inspection(output, code, plan, local, peer):
    if not isinstance(output,str) or len(output)>65536:
        raise r.Refused('inspection_output_bound')
    stages, observations, receipt, cleanup = [], [], None, False
    for line in output.splitlines():
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
            observations.append(r.validate_observation(value['recovery_observation']))
        else:
            if (not isinstance(value,dict) or set(value)!=FIELDS
                    or type(value['schema']) is not int or value['schema']!=1
                    or value['purpose']!='cryptex-device-recovery-inspection'
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
    if passed and (code!=0 or cleanup or stages!=STAGES or any(v['outcome']=='error' for v in observations)):
        raise r.Refused('incomplete_inspection_evidence')
    if not passed and code==0:
        raise r.Refused('inspection_exit_mismatch')
    return dict(receipt=receipt,stages=stages,observations=observations,cleanup_unconfirmed=cleanup)


def verify(execute, args, plan, local, peer):
    code,output=execute(args,90)
    if code:
        raise r.Refused('stopped_profile_verification_refused')
    return r.parse_output(output,plan,local,peer)


def collect(package, plan, system, report, prepare=r.prepare):
    desktop,daemon,root,execute=prepare(package,plan,report,system)
    local,peer=(r.WINDOWS,r.MAC) if system=='Windows' else (r.MAC,r.WINDOWS)
    report['desktop_sha256']=hashlib.sha256(desktop.read_bytes()).hexdigest()
    report['daemon_sha256']=hashlib.sha256(daemon.read_bytes()).hexdigest()
    args=r.verify_command(daemon,root,plan,local,peer)
    report['before']=verify(execute,args,plan,local,peer)
    print('Mevcut kayitlar dogrulandi. Yeni not eklemeden baslangic ve eski islem makbuzu kontrol ediliyor.',flush=True)
    try:
        code,output=execute([desktop,'--cryptex-device-recovery-inspect',plan['run_id'],r.BRAIN,local,peer],240)
        report['driver']=parse_inspection(output,code,plan,local,peer)
    finally:
        # Always collect post-run state, including a refused native startup.
        # No expectation of ten records: the prior Mac attempt may have written.
        report['after']=verify(execute,args,plan,local,peer)
    before,after=report['before'],report['after']
    report['same_local_set_before_after']=all(before[k]==after[k] for k in
        ('canonical_set_sha256','canonical_records','canonical_devices'))
    report['outcome']='diagnostics_collected'
    report['native_inspection_completed']=report['driver']['receipt']['outcome']=='inspection_completed'


def main():
    os.umask(0o077)
    package,system=Path(__file__).resolve().parent,platform.system()
    report=dict(schema=1,purpose='cryptex-recovery-inspection-result',os=system,outcome='incomplete',
                credentials_exported=False,old_app_changed=False,pairing_changed=False,
                recovery_accepted=False)
    try:
        plan=r.read_plan(package,'inspection.json','cryptex-device-recovery-inspection-packet')
        if plan['run_id']!=TARGET_RUN:
            raise r.Refused('wrong_inspection_target')
        report.update(run_id=plan['run_id'],source_commit=plan['source_commit'])
        collect(package,plan,system,report)
    except Exception as error:
        # Errors from arbitrary tools/OS never leave as unbounded text.
        safe={'wrong_inspection_target','invalid_recovery_plan','inspection_output_bound',
              'invalid_observation','inspection_stage_order','trailing_inspection_output',
              'invalid_inspection_receipt','missing_inspection_receipt','incomplete_inspection_evidence',
              'inspection_exit_mismatch','stopped_profile_verification_refused','quit_cryptex_before_update',
              'wrong_mac_profile','wrong_vm_profile','standard_user_required','bounded_command_incomplete'}
        report.update(outcome='incomplete',error=str(error) if str(error) in safe else 'inspection_unavailable')
    output=package/('KESINTI-TANI-SONUC-'+system+'-'+uuid.uuid4().hex[:12]+'.zip')
    with zipfile.ZipFile(output,'x',zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json',json.dumps(report,indent=2)+'\n')
    print('\nGonderilecek sonuc: '+str(output),flush=True)
    print('Yeni not veya kesinti uygulanmadi. Eski kesinti paketini tekrar calistirma; bu raporu gonder.',flush=True)
    return 0 if report['outcome']=='diagnostics_collected' else 2


if __name__=='__main__':
    raise SystemExit(main())
