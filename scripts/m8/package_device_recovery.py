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


def build(artifact, revision, output):
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
    output=output.resolve()
    if output==ROOT or ROOT in output.parents: raise ValueError('output_inside_repository')
    with zipfile.ZipFile(output,'x',zipfile.ZIP_DEFLATED) as archive:
        for name,data in members.items():
            info=zipfile.ZipInfo('Cryptex-Kesinti/'+name);info.create_system=3
            info.external_attr=(0o100700 if name.endswith('.command') else 0o100600)<<16
            archive.writestr(info,data,compress_type=zipfile.ZIP_DEFLATED)
    with zipfile.ZipFile(output) as archive:
        for name,data in members.items():
            if archive.read('Cryptex-Kesinti/'+name)!=data: raise ValueError('packet_readback')
    return dict(source=source,run_id=plan['run_id'],sha256=hashlib.sha256(output.read_bytes()).hexdigest())


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--artifact',type=Path,required=True)
    parser.add_argument('--revision',default='HEAD')
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    print(json.dumps(build(args.artifact,args.revision,args.output)))
