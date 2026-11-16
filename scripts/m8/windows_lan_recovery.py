"""Targeted recovery of the known marker-only Windows setup attempt."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import uuid
import zipfile
from windows_setup_native import Pins,Refused,run
from windows_lan_setup import verify_payload,enroll,SESSION,PRODUCT

APP='2db9fbd6b1db4c5fa38cbb25afe915e3'
PRIOR='32b78e35d9029fb2f519264dcd5e7937bfc5dd5a2fb99fbccc112de5db4875f3'
MARKER=b'm0-native-desktop-runtime-v1\n'

def marker_only(root):
    if {p.name for p in root.iterdir()}!={'pilot-runtime.marker'}:
        raise Refused('profile_not_marker_only')
    if (root/'pilot-runtime.marker').read_bytes()!=MARKER:
        raise Refused('profile_marker_mismatch')

def recover(base,report,ask,runner=run,app_name=APP):
    if not re.fullmatch('[a-f0-9]{32}',app_name): raise Refused('retained_app_binding')
    root=base/'Cryptex-Dev/runtime/p0-2'
    app=base/'Cryptex-Dev/lan-apps'/app_name
    with Pins() as pins:
        report['failed_stage']='inspect_retained_state'
        pins.chain(root)
        pins.pin(root/'pilot-runtime.marker',False)
        marker_only(root)
        pins.chain(app)
        if {p.name for p in app.iterdir()}!={'cryptex-desktop.exe','cryptexd.exe','transport'}:
            raise Refused('retained_app_state_changed')
        verify_payload(app,pins)
        report['marker_only_profile_verified']=True
        report['retained_product_verified']=True
        report['failed_stage']='launch_probe'
        output=runner([app/'cryptexd.exe','--help'],15)
        if 'cryptexd' not in output: raise Refused('help_marker_missing')
        marker_only(root)
        report['launch_probe_passed']=True
        report['profile_preserved_after_probe']=True
        if not ask(): raise Refused('owner_declined_resume')
        marker_only(root)  # Refuse any state change while owner was reviewing.
        report['resume_authorized']=True
        enroll(root,app,report,runner)
        return app

def main():
    package=Path(__file__).resolve().parent
    report=dict(schema=1,purpose='cryptex-windows-lan-launch-recovery',platform='Windows',run_id=SESSION,
                product_source=PRODUCT,prior_receipt_sha256=PRIOR,pairing_authorized=False,
                network_delivery_verified=False,convergence_verified=False,normal_window_verified=False,
                initialized_startup_restart=False,outcome='refused_or_incomplete',failed_stage='preflight')
    try:
        plan=json.loads((package/'recovery.json').read_text(encoding='utf-8'))
        if (plan['run_id']!=SESSION or plan['app']!=APP or plan['prior_receipt_sha256']!=PRIOR
                or not re.fullmatch('[a-f0-9]{40}',plan['source_commit'])): raise Refused('recovery_binding')
        report['source_commit']=plan['source_commit']
        if os.name!='nt' or ctypes.windll.shell32.IsUserAnAdmin(): raise Refused('standard_windows_user_required')
        powershell=Path(os.environ['SYSTEMROOT'])/'System32/WindowsPowerShell/v1.0/powershell.exe'
        run([powershell,'-NoProfile','-NonInteractive','-Command',
             'if (Get-Process cryptex-desktop,cryptexd -ErrorAction SilentlyContinue) { exit 2 }'],15)
        def ask():
            print('Baslatma denemesi gecti. Mevcut bos profilden devam edilebilir.\n'
                  'Kaydedilecek ilk public not:\nCryptex LAN acceptance '+SESSION+': Windows first public note.')
            return input('Bu profile kuruluma devam? [E/H] ').strip().upper()=='E'
        app=recover(Path(os.environ['LOCALAPPDATA']),report,ask)
        report['failed_stage']='write_normal_launcher'
        for name in ('windows_lan_open.py','windows_setup_native.py','windows_lan_setup.py'):
            with (app/name).open('xb') as stream:stream.write((package/name).read_bytes())
        with (package/'CRYPTEX-AC.cmd').open('x',encoding='utf-8',newline='\r\n') as stream:
            stream.write('@echo off\n"'+sys.executable+'" -B "'+str(app/'windows_lan_open.py')+'"\npause\n')
        report.pop('failed_stage',None)
        report['outcome']='recovered_local_only'
        print('Kalici kurulum tamamlandi. Normal acilis icin CRYPTEX-AC.cmd hazir.')
    except Refused as error:
        report['error']=str(error)
        if type(error.winerror) is int and 0<=error.winerror<2**32:
            report['win32_error']=error.winerror
            print('Windows hata kodu: '+str(error.winerror))
        print('Islem durdu: '+str(error)+'. Profili silme; sonuc dosyasini gonder.')
    except Exception:
        report['error']='recovery_internal_failure'
        print('Islem durdu. Profili silme; sonuc dosyasini gonder.')
    output=Path(os.environ['USERPROFILE'])/'Downloads'/('ESITLEME-ONARIM-SONUC-Windows-'+uuid.uuid4().hex[:12]+'.zip')
    with zipfile.ZipFile(output,'x',zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json',json.dumps(report,indent=2))
    print('Sonuc: '+str(output))
if __name__=='__main__':main()
