"""Explicit persistent Windows enrollment. No peer grant or network acceptance."""
import hashlib
import json
import os
from pathlib import Path
import sys
import uuid
import zipfile
import ctypes
from windows_setup_native import Pins, Refused, run

SESSION='b2ad8bb6-2d65-4690-b3c6-6a7b3f5adac4'
PRODUCT='3d8e5b4756f7a6c91ae245c630a48efc645c0a50'
HASHES={
 'cryptex-desktop.exe':'730a36ae3da6d6a32f5fc5ea160f631989e7d572e3529044a9b5fa75b85ee8c2',
 'cryptexd.exe':'57e5555af1f1bcdf4c1757a66ab5b68c5dbdd466af2f834dac54e05933b0acb9',
 'transport/syncthing.exe':'36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344',
 'transport/LICENSE.txt':'3f3d9e0024b1921b067d6f7f88deb4a60cbe7a78e76c64e3f1d7fc3b779b9d04',
 'transport/AUTHORS.txt':'5a0044d13ddf6f013bdd5c2bc419bf45d6123c356567510237e82f304d113d48'}

def verify_payload(payload,pins):
    pins.chain(payload)
    pins.pin(payload/'transport')
    for name,digest in HASHES.items():
        path=payload/name
        pins.pin(path,False)
        if path.stat().st_size>64*1024*1024 or hashlib.sha256(path.read_bytes()).hexdigest()!=digest:
            raise Refused('payload_hash_mismatch')

def setup(payload,base,session,report,runner=run):
    """base is supplied by normal entry; tests use a fresh disposable CI base."""
    if session!=SESSION: raise Refused('session_mismatch')
    note=('Cryptex LAN acceptance '+session+': Windows first public note.').encode()
    with Pins() as pins:
        verify_payload(payload,pins)
        pins.chain(base)
        parent=pins.child(base,'Cryptex-Dev')
        runtime=pins.child(parent,'runtime')
        # Refuse before retaining any product or touching existing profile data.
        root=pins.child(runtime,'p0-2',fresh=True)
        report['fresh_profile_created']=True
        apps=pins.child(parent,'lan-apps')
        app=pins.child(apps,uuid.uuid4().hex,fresh=True)
        pins.child(app,'transport',fresh=True)
        for name in HASHES:
            with (app/name).open('xb') as out: out.write((payload/name).read_bytes())
        verify_payload(app,pins)
        (root/'pilot-runtime.marker').write_bytes(b'm0-native-desktop-runtime-v1\n')
        daemon=app/'cryptexd.exe'
        def step(label,args,marker,stdin=b'',timeout=60):
            report['failed_stage']=label
            print('Cryptex: '+label,flush=True)
            output=runner([daemon,*args],timeout,input_data=stdin)
            if marker and marker not in output.splitlines(): raise Refused('receipt_marker_missing')
        step('public_brain_create',['pilot-fixture','lan-profile-v1','--runtime-root',root,'--brain-id',session,
                                   '--confirm-isolated-pilot'],'lan_profile_fixture=ready')
        step('local_key_enrollment',['identity','enroll-local','--runtime-root',root,'--environment','dev'],None)
        step('first_note_capture',['identity','first-note-local-dev','--runtime-root',root,
                                  '--confirm-public-first-note-stdin'],'first_note=committed_public',note)
        report['first_note_committed']=True
        report['first_note_sha256']=hashlib.sha256(note).hexdigest()
        step('transport_enrollment',['transport','enroll-local-dev','--runtime-root',root,'--confirm-local-only',
                                     '--package',app/'transport'],'transport_enrollment=local_only',timeout=180)
        report['transport_enrolled_local_only']=True
        report['persistent_profile_preserved']=True
        # Ordinary no-argument opening; no pilot override, fixture or path argument.
        return app

def main():
    report=dict(schema=1,purpose='cryptex-windows-persistent-lan-setup',platform='Windows',run_id=SESSION,
                product_source=PRODUCT,pairing_authorized=False,network_delivery_verified=False,
                convergence_verified=False,normal_window_verified=False,initialized_startup_restart=False,
                outcome='refused_or_incomplete',failed_stage='preflight')
    package=Path(__file__).resolve().parent
    try:
        if os.name!='nt' or not os.environ.get('LOCALAPPDATA'): raise Refused('windows_required')
        if ctypes.windll.shell32.IsUserAnAdmin(): raise Refused('standard_user_required')
        plan=json.loads((package/'setup.json').read_text(encoding='utf-8'))
        if plan['run_id']!=SESSION or plan['product_source']!=PRODUCT: raise Refused('package_binding')
        report['source_commit']=plan['source_commit']
        print('Yeni kalici Windows test profili kurulacak. Ilk public not:\n'
              'Cryptex LAN acceptance '+SESSION+': Windows first public note.\n'
              'Mevcut profiller silinmez; eslestirme bu adimda yapilmaz.')
        if input('Kuruluma devam? [E/H] ').strip().upper()!='E': raise Refused('owner_declined')
        powershell=Path(os.environ['SYSTEMROOT'])/'System32/WindowsPowerShell/v1.0/powershell.exe'
        active=run([powershell,'-NoProfile','-NonInteractive','-Command',
                    "if (Get-Process cryptex-desktop,cryptexd -ErrorAction SilentlyContinue) { exit 2 }"],15)
        app=setup(package/'payload',Path(os.environ['LOCALAPPDATA']),SESSION,report)
        # The launcher verifies the retained binaries again before ordinary opening.
        for name in ('windows_lan_open.py','windows_setup_native.py','windows_lan_setup.py'):
            with (app/name).open('xb') as stream: stream.write((package/name).read_bytes())
        launcher=package/'CRYPTEX-AC.cmd'
        with launcher.open('x',encoding='utf-8',newline='\r\n') as stream:
            stream.write('@echo off\n"'+sys.executable+'" -B "'+str(app/'windows_lan_open.py')+'"\npause\n')
        report.pop('failed_stage',None)
        report['outcome']='enrolled_local_only'
        print('Kurulum tamamlandi. Normal pencere kontrolu: CRYPTEX-AC.cmd dosyasini ac.')
    except Refused as error:
        report['error']=str(error)
        print('Kurulum durdu: '+str(error)+'. Profili silme; sonuc ZIP dosyasini gonder.')
    except Exception:
        report['error']='setup_internal_failure'
        print('Kurulum durdu. Profili silme; sonuc ZIP dosyasini gonder.')
    output=Path(os.environ['USERPROFILE'])/'Downloads'/('ESITLEME-KURULUM-SONUC-Windows-'+uuid.uuid4().hex[:12]+'.zip')
    with zipfile.ZipFile(output,'x',zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json',json.dumps(report,indent=2))
    print('Sonuc: '+str(output))

if __name__=='__main__': main()
