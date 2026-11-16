"""Package exact already-verified Windows CI products plus explicit setup tools."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import urllib.request
import zipfile
from windows_lan_setup import HASHES,SESSION,PRODUCT

ROOT=Path(__file__).resolve().parents[2]

def payload(artifact):
    provenance=json.loads((artifact/'provenance.json').read_text())
    if provenance['source_commit']!=PRODUCT or provenance['run_id']!='43905651278':
        raise ValueError('product_provenance_mismatch')
    data={}
    for name in ('cryptex-desktop.exe','cryptexd.exe'):
        data[name]=(artifact/name).read_bytes()
    url='https://github.com/syncthing/syncthing/releases/download/v2.1.5/syncthing-windows-amd64-v2.1.5.zip'
    with urllib.request.urlopen(url,timeout=60) as response: raw=response.read(64*1024*1024+1)
    if len(raw)>64*1024*1024: raise ValueError('transport_archive_bound')
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        matches=[i for i in archive.infolist() if i.filename=='syncthing-windows-amd64-v2.1.5/syncthing.exe']
        if len(matches)!=1 or matches[0].file_size>64*1024*1024: raise ValueError('transport_archive_member')
        data['transport/syncthing.exe']=archive.read(matches[0])
    for name in ('LICENSE.txt','AUTHORS.txt'):
        data['transport/'+name]=(ROOT/'third_party/syncthing/2.1.5'/name).read_bytes()
    for name,digest in HASHES.items():
        if hashlib.sha256(data[name]).hexdigest()!=digest: raise ValueError('payload_hash_mismatch')
    return data

def build(artifact,output,revision,vm=False):
    commit=subprocess.check_output(['git','rev-parse',revision+'^{commit}'],cwd=ROOT,text=True).strip()
    members={'payload/'+n:d for n,d in payload(artifact).items()}
    for name in ('windows_lan_setup.py','windows_setup_native.py','windows_lan_open.py'):
        members[name]=subprocess.check_output(['git','show',commit+':scripts/m8/'+name],cwd=ROOT)
    members['setup.json']=json.dumps(dict(schema=1,run_id=SESSION,source_commit=commit,product_source=PRODUCT)).encode()
    members['WINDOWS-ESITLEME-KUR.cmd']=b'@echo off\r\ncd /d "%~dp0"\r\npy -3 -B windows_lan_setup.py\r\npause\r\n'
    members['BASLA.txt']=('Windows kalici LAN test kurulumu\n\n'
        'Bu paketi Mac ile ayni agdaki ANA WINDOWS bilgisayarinda ZIPten cikar.\n'
        'Acik Cryptex uygulamasindan Quit ile cik. WINDOWS-ESITLEME-KUR.cmd dosyasini ac.\n'
        'Gosterilen public test notunu kabul ediyorsan E yaz. Yonetici hakki gerekmez.\n'
        'Mevcut Python 3 kullanilir; Windows guvenlik ayarlari degistirilmez.\n'
        'Mevcut normal profil varsa kurulum durur. Hata olursa profil/anahtar silme.\n'
        'Olusan ESITLEME-KURULUM-SONUC-Windows-*.zip dosyasini sohbete gonder.\n'
        'Basarili kurulumdan sonra CRYPTEX-AC.cmd ile normal uygulamayi ac.\n'
        'Pencere acilisini ve eslestirmeyi ayri kontrol edecegiz. Bu paket ag teslimini kanitlamaz.\n'
        'Syncthing 2.1.5 MPL-2.0, upstream source: https://github.com/syncthing/syncthing/tree/v2.1.5\n').encode()
    folder='Cryptex-Windows-Esitleme-Kurulum/'
    if vm:
        folder='Cryptex-Windows-VM-Kurulum/'
        del members['WINDOWS-ESITLEME-KUR.cmd']
        members['VM-ESITLEME-KUR.cmd']=(
            '@echo off\r\ncd /d "%~dp0"\r\n'
            'echo Bu paketi yalnizca test Windows VM icinde calistirin.\r\n'
            'py -3 -B -c "import sys; sys.exit(sys.version_info < (3, 9))" >nul 2>&1\r\n'
            'if errorlevel 1 (\r\n'
            '  echo Python 3.9 veya ustu ve py baslaticisi bulunamadi. Bu mesaji bildir.\r\n'
            '  pause\r\n  exit /b 1\r\n)\r\n'
            'py -3 -B windows_lan_setup.py\r\npause\r\n').encode()
        members['BASLA.txt']=(
            'Windows VM kalici LAN test kurulumu\n\n'
            'ZIPi VM icindeki Downloads klasorune kopyala ve Tumunu ayikla.\n'
            'Ana bilgisayarda calistirma. Snapshot alindigindan emin ol.\n'
            'Ana bilgisayarin profilini veya anahtarlarini VMye kopyalama.\n'
            'VMde acik Cryptex varsa Quit ile cik. VM-ESITLEME-KUR.cmd dosyasini cift tikla.\n'
            'Yonetici olarak calistirma. Python 3.9+ ve py baslaticisi gerekir.\n'
            'Gosterilen public test notunu kabul ediyorsan E yaz ve Entera bas.\n'
            'Yeni cihaz kimligi VM icinde olusur; Macin mevcut test Brain kimligi kullanilir.\n'
            'VMde mevcut normal profil varsa durur; eski profilleri silmez veya tasimaz.\n'
            'Hata veya Windows guvenlik uyarisi olursa dur; korumayi kapatma, istisna ekleme.\n'
            'Olusan ESITLEME-KURULUM-SONUC-Windows-*.zip dosyasini sohbete gonder.\n'
            'Sonuc VMnin Downloads klasorundedir. Kurulumu ikinci kez calistirma.\n'
            'Basarili kurulum CRYPTEX-AC.cmd olusturur; normal pencereyi sonra kontrol edecegiz.\n'
            'Bu adim eslestirme, ag ayari veya ag teslimi yapmaz. Mac kurulumunu tekrarlama.\n'
            'VM testi ana Windows yayin/imza kabulunun yerine gecmez.\n'
            'Syncthing 2.1.5 MPL-2.0, upstream source: https://github.com/syncthing/syncthing/tree/v2.1.5\n').encode()
    output=output.resolve()
    if output==ROOT or ROOT in output.parents: raise ValueError('output_inside_repository')
    with zipfile.ZipFile(output,'x',zipfile.ZIP_DEFLATED) as archive:
        for name,data in members.items(): archive.writestr(folder+name,data)
    with zipfile.ZipFile(output) as archive:
        if len(archive.infolist())!=len(members): raise ValueError('packet_member_count')
        for name,data in members.items():
            if archive.read(folder+name)!=data: raise ValueError('packet_readback')
    return commit,hashlib.sha256(output.read_bytes()).hexdigest()

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--artifact',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True);parser.add_argument('--revision',default='HEAD')
    parser.add_argument('--vm',action='store_true')
    args=parser.parse_args();print(build(args.artifact,args.output,args.revision,args.vm))
