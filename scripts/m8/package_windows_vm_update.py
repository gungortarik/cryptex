"""Stage exact CI product and unchanged pinned transport for the retained VM."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import zipfile

ROOT=Path(__file__).resolve().parents[2]
TRANSPORT_PACKET_SHA='4a3d8504b1953f5eb4b7de71bce3eee55caaa2c1718086e37f145c40610a79f4'


def build(artifact,transport_packet,product_source,run_id,output):
    if not re.fullmatch('[0-9a-f]{40}',product_source) or not run_id.isdecimal():
        raise ValueError('expected_provenance')
    provenance=json.loads((artifact/'provenance.json').read_text(encoding='utf-8-sig'))
    if provenance['source_commit']!=product_source or provenance['run_id']!=run_id:
        raise ValueError('product_provenance')
    payload={}
    for name in ('cryptex-desktop.exe','cryptexd.exe'):
        entries=[entry for entry in provenance['files'] if entry['name']==name]
        data=(artifact/name).read_bytes()
        if (len(entries)!=1 or len(data)>64*1024*1024 or len(data)!=entries[0]['bytes']
                or hashlib.sha256(data).hexdigest()!=entries[0]['sha256'].lower()):
            raise ValueError('product_hash')
        payload[name]=data
    raw=transport_packet.read_bytes()
    if hashlib.sha256(raw).hexdigest()!=TRANSPORT_PACKET_SHA: raise ValueError('transport_packet_hash')
    with zipfile.ZipFile(transport_packet) as archive:
        for name in ('syncthing.exe','LICENSE.txt','AUTHORS.txt'):
            payload['transport/'+name]=archive.read('Cryptex-Windows-VM-Kurulum/payload/transport/'+name)
    tool_source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    members={'payload/'+name:data for name,data in payload.items()}
    for name in ('windows_vm_update.py','windows_pairing_diagnostic.py','windows_setup_native.py'):
        members[name]=subprocess.check_output(['git','show',tool_source+':scripts/m8/'+name],cwd=ROOT)
    members['candidate.json']=json.dumps(dict(schema=1,product_source=product_source,tool_source=tool_source,
        run_id=run_id,hashes={name:hashlib.sha256(data).hexdigest() for name,data in payload.items()})).encode()
    members['VM-GUNCELLE.cmd']=b'@echo off\r\ncd /d "%~dp0"\r\npy -3 -B windows_vm_update.py\r\npause\r\n'
    members['BASLA.txt']=(
        'Bu paket yalnizca eslestirme yapilmis Windows VM icindir.\n'
        'VMde Cryptex tepsi menusunden Quit Cryptex ile cik. Maci degistirme.\n'
        'ZIPi VMnin Downloads klasorunde Tumunu ayikla ile ac.\n'
        'VM-GUNCELLE.cmd dosyasina cift tikla; yonetici olarak calistirma.\n'
        'Profil ve anahtarlar korunur; eslestirme veya yeni profil kurulumu yapilmaz.\n'
        'Yeni uygulama eski uygulamanin yanina kurulur ve normal bicimde acilir.\n'
        'Bu noktadan sonra bu klasorde olusan CRYPTEX-AC.cmd dosyasini kullan.\n'
        'VM-GUNCELLEME-SONUC-*.zip dosyasini Downloads klasorunden sohbete gonder.\n'
        'Local connections ekranini acip durumunu da gonder. Pause sync isaretli kalsin.\n'
        'Guvenlik engeli olursa korumayi kapatma veya istisna ekleme. Profili silme.\n'
        'Syncthing 2.1.5 MPL-2.0: https://github.com/syncthing/syncthing/tree/v2.1.5\n').encode()
    output=output.resolve()
    if output==ROOT or ROOT in output.parents: raise ValueError('output_inside_repository')
    with zipfile.ZipFile(output,'x',zipfile.ZIP_DEFLATED) as archive:
        for name,data in members.items(): archive.writestr('Cryptex-VM-Guncelle/'+name,data)
    with zipfile.ZipFile(output) as archive:
        if len(archive.infolist())!=len(members): raise ValueError('readback_count')
        for name,data in members.items():
            if archive.read('Cryptex-VM-Guncelle/'+name)!=data: raise ValueError('readback_bytes')
    return tool_source,hashlib.sha256(output.read_bytes()).hexdigest()


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--artifact',type=Path,required=True)
    parser.add_argument('--transport-packet',type=Path,required=True)
    parser.add_argument('--product-source',required=True)
    parser.add_argument('--run-id',required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    print(build(args.artifact,args.transport_packet,args.product_source,args.run_id,args.output))
