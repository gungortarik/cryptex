"""Install a pinned candidate beside the old app; never enroll or edit the profile."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid
import zipfile

from windows_setup_native import Pins, Refused, run
from windows_pairing_diagnostic import BRAIN, LOCAL, read_json

FILES={'cryptex-desktop.exe','cryptexd.exe','transport/syncthing.exe',
       'transport/LICENSE.txt','transport/AUTHORS.txt'}


def verify(app,manifest,pins):
    if set(manifest['hashes'])!=FILES: raise Refused('candidate_manifest')
    pins.chain(app)
    pins.pin(app/'transport')
    for name,digest in manifest['hashes'].items():
        path=app/name
        pins.pin(path,False)
        with path.open('rb') as stream: data=stream.read(64*1024*1024+1)
        if len(data)>64*1024*1024 or hashlib.sha256(data).hexdigest()!=digest:
            raise Refused('candidate_hash_mismatch')


def install(package,base,manifest,runner=run):
    root=base/'Cryptex-Dev/runtime/p0-2'
    with Pins() as pins:
        pins.chain(root)
        brain=read_json(root/'identity/brain_identity.json')
        local=read_json(root/'identity/local_device.json')
        if brain.get('brain_id')!=BRAIN or local.get('brain_id')!=BRAIN or local.get('device_id')!=LOCAL:
            raise Refused('wrong_vm_profile')
        verify(package/'payload',manifest,pins)
        # Owner must quit first. No termination, stop marker or security override.
        shell=Path(os.environ['SYSTEMROOT'])/'System32/WindowsPowerShell/v1.0/powershell.exe'
        runner([shell,'-NoProfile','-NonInteractive','-Command',
                "if (Get-Process cryptex-desktop,cryptexd -ErrorAction SilentlyContinue) { exit 2 }"],15)
        apps=pins.child(base/'Cryptex-Dev','lan-apps')
        app=pins.child(apps,uuid.uuid4().hex,fresh=True)
        pins.child(app,'transport',fresh=True)
        for name in FILES:
            with (app/name).open('xb') as stream: stream.write((package/'payload'/name).read_bytes())
        verify(app,manifest,pins)
        for name in ('windows_vm_update.py','windows_setup_native.py','windows_pairing_diagnostic.py'):
            with (app/name).open('xb') as stream: stream.write((package/name).read_bytes())
        with (app/'candidate.json').open('x',encoding='utf-8') as stream: json.dump(manifest,stream)
        return app


def open_app(app,manifest):
    with Pins() as pins:
        verify(app,manifest,pins)
        env={key:value for key,value in os.environ.items() if not key.upper().startswith('CRYPTEX')}
        subprocess.Popen([str(app/'cryptex-desktop.exe')],cwd=app,env=env,close_fds=True)


def main():
    package=Path(__file__).resolve().parent
    if sys.argv[1:]==['--open']:
        try: open_app(package,read_json(package/'candidate.json'))
        except Exception: print('Cryptex acilamadi. Profili silme; bu mesaji bildir.')
        return
    report=dict(schema=1,purpose='cryptex-vm-existing-profile-update',outcome='refused',
                normal_window_verified=False,network_delivery_verified=False)
    try:
        if os.name!='nt' or ctypes.windll.shell32.IsUserAnAdmin(): raise Refused('standard_windows_user_required')
        manifest=read_json(package/'candidate.json')
        report['product_source']=manifest['product_source']
        report['tool_source']=manifest['tool_source']
        app=install(package,Path(os.environ['LOCALAPPDATA']),manifest)
        report['outcome']='installed_beside_old_app'
        launcher=package/'CRYPTEX-AC.cmd'
        with launcher.open('x',encoding='utf-8',newline='\r\n') as stream:
            stream.write('@echo off\n"'+sys.executable+'" -B "'+str(app/'windows_vm_update.py')+'" --open\npause\n')
        open_app(app,manifest)
        report['launch_requested']=True
        print('Yeni uygulama aciliyor. Bundan sonra bu klasordeki CRYPTEX-AC.cmd dosyasini kullan.')
    except Refused as error:
        report['error']=str(error)
        print('Durdu: '+str(error)+'. Cryptex aciksa Quit ile cik; profili silme.')
    except Exception:
        report['error']='update_unavailable'
        print('Guncelleme tamamlanamadi. Profili silme; sonucu gonder.')
    output=Path(os.environ['USERPROFILE'])/'Downloads'/('VM-GUNCELLEME-SONUC-'+uuid.uuid4().hex[:12]+'.zip')
    with zipfile.ZipFile(output,'x',zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json',json.dumps(report,indent=2))
    print(str(output))
    return report.get('launch_requested') is True


if __name__=='__main__': main()
