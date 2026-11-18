"""Exact-source, default-off Desktop recovery packet for the existing owner pair."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import urllib.request
import uuid
import zipfile
from mac_lan_setup import source_members
from windows_lan_setup import HASHES

ROOT=Path(__file__).resolve().parents[2]


def build(artifact, revision, output, *, inspection=False, windows_inspection_run=None, queued_note=False, vm_queued_note=False):
    source=subprocess.check_output(['git','rev-parse',revision+'^{commit}'],cwd=ROOT,text=True).strip()
    provenance=json.loads((artifact/'provenance.json').read_text(encoding='utf-8-sig'))
    if provenance['source_commit'] != source or provenance['feature'] != 'm8-device-recovery-check':
        raise ValueError('recovery_artifact_provenance')
    data={}
    for name in ('cryptex-desktop.exe','cryptexd.exe'):
        raw=(artifact/name).read_bytes()
        if len(raw)>64*1024*1024 or hashlib.sha256(raw).hexdigest()!=provenance['hashes'][name]:
            raise ValueError('recovery_artifact_hash')
        data[name]=raw
    url='https://github.com/syncthing/syncthing/releases/download/v2.1.5/syncthing-windows-amd64-v2.1.5.zip'
    with urllib.request.urlopen(url,timeout=60) as response: raw=response.read(64*1024*1024+1)
    if len(raw)>64*1024*1024: raise ValueError('transport_archive_bound')
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        infos=[i for i in archive.infolist() if i.filename=='syncthing-windows-amd64-v2.1.5/syncthing.exe']
        if len(infos)!=1 or infos[0].file_size>64*1024*1024: raise ValueError('transport_archive_member')
        data['transport/syncthing.exe']=archive.read(infos[0])
    for name in ('LICENSE.txt','AUTHORS.txt'):
        data['transport/'+name]=(ROOT/'third_party/syncthing/2.1.5'/name).read_bytes()
    for name in ('transport/syncthing.exe','transport/LICENSE.txt','transport/AUTHORS.txt'):
        if hashlib.sha256(data[name]).hexdigest()!=HASHES[name]: raise ValueError('transport_hash')
    raw=subprocess.check_output(['git','archive','--format=zip',source],cwd=ROOT)
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        source_members(archive)
        members={Path(i.filename).name:archive.read(i) for i in archive.infolist()
                 if i.filename.startswith('scripts/m8/') and i.filename.endswith('.py')
                 and '/' not in i.filename.removeprefix('scripts/m8/')}
        members['worker_failure_codes.json']=archive.read('scripts/m8/worker_failure_codes.json')
    if 'lan_device_recovery.py' not in members: raise ValueError('collector_not_committed')
    plan=dict(schema=1,purpose='cryptex-device-recovery-packet',run_id=str(uuid.uuid4()),
              source_commit=source,source_sha256=hashlib.sha256(raw).hexdigest(),
              hashes={n:hashlib.sha256(d).hexdigest() for n,d in data.items()})
    members.update({'payload/'+n:d for n,d in data.items()})
    members.update({'source.zip':raw,'recovery.json':(json.dumps(plan,indent=2)+'\n').encode(),
        'KESINTI-WINDOWS.cmd':b'@echo off\r\ncd /d "%~dp0"\r\npy -3 -B lan_device_recovery.py\r\npause\r\n',
        'KESINTI-MAC.command':b'#!/bin/zsh\nset -eu\ncd -- "${0:A:h}"\n/usr/bin/python3 -B lan_device_recovery.py\n',
        'BASLA.txt':(
            'Ayni ZIPi kendi Macine ve Windows VMine TUMUNU AYIKLA ile cikart.\n'
            'Iki Cryptexi normal Quit / Command-Q ile kapat. Pause sync ayarina dokunman gerekmiyor.\n'
            'Mac: KESINTI-MAC.command. Derleme bitince Windows VM: KESINTI-WINDOWS.cmd.\n'
            'Ana Windows cihazinda calistirma. Profilleri silme veya yeniden eslestirme.\n'
            'Terminal pencerelerini acik, iki cihazi ayni agda ve uyanik tut.\n'
            'Mac mevcut yerel sertifikayla ayri test uygulamasi derler. Keychain izni macOS penceresinde verilir.\n'
            'Paket her cihazda iki kisa Public test notu olusturur; notlar kalici tutulur.\n'
            'Not girmen, mesaj kopyalaman, Search yapman veya End Task kullanman gerekmiyor.\n'
            'Windows daemonu; Mac ise test Desktop surecini aniden kapatir. Mac daemonu kendi yardimcisini normal guvenli yolla kapatir.\n'
            'Esitleme kapaliyken yeniden acilip kayitlar okunur; basarida onceki Pause sync ayari geri gelir.\n'
            'Hata halinde otomatik kilit temizleme, profil onarimi veya guvenlik ayari degisikligi yapilmaz.\n'
            'Iki KESINTI-SONUC-*.zip dosyasini gonder. Sonuclari incelemeden paketi tekrar calistirma.\n'
            'Bu test bekleyen veriyle surec kesintisi/toparlanma icindir; guc kaybi veya baytin ortasinda kesinti kaniti degildir.\n'
            'Eski uygulama ve kisayol degismez. Sonuc raporunda not icerigi ve ozel anahtar bulunmaz.\n'
            'Syncthing 2.1.5 MPL-2.0: https://github.com/syncthing/syncthing/tree/v2.1.5\n').encode()})
    folder='Cryptex-Kesinti'
    if inspection or windows_inspection_run or queued_note or vm_queued_note:
        folder='Cryptex-Kesinti-Tani'
        plan.update(purpose='cryptex-device-recovery-inspection-packet',
                    run_id='4bbd029c-2ab2-426f-aec5-725b67cbcaf7')
        for name in ('recovery.json','KESINTI-WINDOWS.cmd','KESINTI-MAC.command'):
            members.pop(name)
        members.update({
            'inspection.json':(json.dumps(plan,indent=2)+'\n').encode(),
            'TANI-WINDOWS.cmd':b'@echo off\r\ncd /d "%~dp0"\r\npy -3 -B lan_recovery_inspection.py\r\npause\r\n',
            'TANI-MAC.command':b'#!/bin/zsh\nset -eu\ncd -- "${0:A:h}"\n/usr/bin/python3 -B lan_recovery_inspection.py\n',
            'BASLA.txt':(
                'Cryptex aciksa iki cihazda da normal Quit / Command-Q ile kapat.\n'
                'ZIPi tamamen cikart. Mac: TANI-MAC.command. Windows VM: TANI-WINDOWS.cmd.\n'
                'Iki cihazda ayni anda baslatman gerekmiyor. Terminal acik, cihaz uyanik kalsin.\n'
                'Yeni not eklenmez, kesinti uygulanmaz, esitleme ayarlari degistirilmez.\n'
                'Normal servis acilisi daha once var olan gelen veriyi isleyebilir; once/sonra ozetleri toplanir.\n'
                'Mac ayri uygulamayi mevcut yerel sertifikayla derler; eski kisayol ve kimlik korunur.\n'
                'macOS Keychain izni isterse yalnizca macOS penceresinde ver.\n'
                'KESINTI-TANI-SONUC ZIPlerini gonder. Eski KESINTI paketini tekrar calistirma.\n'
                'Syncthing 2.1.5 MPL-2.0: https://github.com/syncthing/syncthing/tree/v2.1.5\n').encode()})
    if queued_note or vm_queued_note:
        from lan_recovery_inspection import QUEUED_RUN
        folder = 'Cryptex-Bekleyen-Not'
        plan['run_id'] = QUEUED_RUN
        members['inspection.json'] = (json.dumps(plan,indent=2)+'\n').encode()
        members.pop('TANI-MAC.command')
        members.pop('TANI-WINDOWS.cmd')
        members['bekleyen_not.py'] = (
            'from lan_recovery_inspection import QUEUED_RUN, main\n'
            'raise SystemExit(main(QUEUED_RUN, queued_note=True))\n').encode()
        members['MAC-KONTROL.command'] = b'#!/bin/zsh\nset -eu\ncd -- "${0:A:h}"\n/usr/bin/python3 -B bekleyen_not.py\n'
        members['WINDOWS-KONTROL.cmd'] = (
            b'@echo off\r\ncd /d "%~dp0"\r\nif not exist bekleyen_not.py (echo ZIP dosyasini tamamen ayiklayin. & pause & exit /b 1)\r\n'
            b'py -3 -B bekleyen_not.py\r\npause\r\n')
        members['BASLA.txt'] = (
            'Iki cihazda Cryptex aciksa normal Quit ile kapat. ZIPi iki cihazda tamamen ayikla.\n'
            'Once Mac: MAC-KONTROL.command. MAC HAZIR yazinca VM: WINDOWS-KONTROL.cmd.\n'
            'Iki cihazi uyanik ve ayni agda, terminal pencerelerini acik tut.\n'
            'Kontrol mevcut Mac notunu arar, normal kapanis ve yeniden acilista dogrular.\n'
            'Iki BEKLEYEN-NOT-SONUC ZIPini gonder.\n'
            'Syncthing 2.1.5 MPL-2.0: https://github.com/syncthing/syncthing/tree/v2.1.5\n').encode()
        if vm_queued_note:
            folder = 'Cryptex-VM-Not-Kontrol'
            members.pop('source.zip')
            members.pop('MAC-KONTROL.command')
            members['BASLA.txt'] = (
                'Windows VM icinde Cryptex aciksa Quit ile kapat. ZIPi tamamen ayikla.\n'
                'WINDOWS-KONTROL.cmd dosyasina cift tikla.\n'
                'Mevcut Mac notu yerel kayitlarda aranir ve VM uygulamasi yeniden acilarak kontrol edilir.\n'
                'BEKLEYEN-NOT-SONUC-Windows ZIPini gonder.\n'
                'Syncthing 2.1.5 MPL-2.0: https://github.com/syncthing/syncthing/tree/v2.1.5\n').encode()
    elif windows_inspection_run:
        run_id = str(uuid.UUID(windows_inspection_run))
        if run_id != windows_inspection_run:
            raise ValueError('invalid_inspection_run')
        folder = 'Cryptex-VM-Baslatma'
        for name in ('source.zip', 'TANI-MAC.command', 'TANI-WINDOWS.cmd'):
            members.pop(name)
        plan['run_id'] = run_id
        members['inspection.json'] = (json.dumps(plan, indent=2)+'\n').encode()
        members['inspect_current.py'] = (
            'import sys\nfrom lan_recovery_inspection import main\n'
            f'if sys.platform == "win32":\n    raise SystemExit(main("{run_id}", start_only=True))\n'
            'raise SystemExit("Windows VM required")\n').encode()
        members['VM-KONTROL.cmd'] = b'@echo off\r\ncd /d "%~dp0"\r\npy -3 -B inspect_current.py\r\npause\r\n'
        members['BASLA.txt'] = ('Yalniz Windows VM: Cryptex aciksa normal Quit ile kapat. ZIPi tamamen ayikla.\n'
                                'VM-KONTROL.cmd dosyasina cift tikla. TANI satirini paylas.\n'
                                'Bu paket mevcut baglantiyi okur. Not eklemez, kesinti uygulamaz.\n'
                                'Syncthing 2.1.5 MPL-2.0: https://github.com/syncthing/syncthing/tree/v2.1.5\n').encode()
    output=output.resolve()
    if output==ROOT or ROOT in output.parents: raise ValueError('output_inside_repository')
    with zipfile.ZipFile(output,'x',zipfile.ZIP_DEFLATED) as archive:
        for name,data in members.items():
            info=zipfile.ZipInfo(folder+'/'+name);info.create_system=3
            info.external_attr=(0o100700 if name.endswith('.command') else 0o100600)<<16
            archive.writestr(info,data,compress_type=zipfile.ZIP_DEFLATED)
    with zipfile.ZipFile(output) as archive:
        for name,data in members.items():
            if archive.read(folder+'/'+name)!=data: raise ValueError('packet_readback')
    return dict(source=source,run_id=plan['run_id'],sha256=hashlib.sha256(output.read_bytes()).hexdigest())


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--artifact',type=Path,required=True)
    parser.add_argument('--revision',default='HEAD')
    parser.add_argument('--output',type=Path,required=True)
    mode=parser.add_mutually_exclusive_group()
    mode.add_argument('--inspect',action='store_true')
    mode.add_argument('--vm-inspect-run')
    mode.add_argument('--queued-note',action='store_true')
    mode.add_argument('--vm-queued-note',action='store_true')
    args=parser.parse_args()
    print(json.dumps(build(args.artifact,args.revision,args.output,inspection=args.inspect,
                           windows_inspection_run=args.vm_inspect_run,queued_note=args.queued_note,
                           vm_queued_note=args.vm_queued_note)))
