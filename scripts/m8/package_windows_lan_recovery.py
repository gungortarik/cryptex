"""Source-bound one-click recovery; reuses pinned retained product, no binary replacement."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile
from windows_lan_recovery import APP,PRIOR,SESSION
ROOT=Path(__file__).resolve().parents[2]

def build(revision,output):
    commit=subprocess.check_output(['git','rev-parse',revision+'^{commit}'],cwd=ROOT,text=True).strip()
    members={name:subprocess.check_output(['git','show',commit+':scripts/m8/'+name],cwd=ROOT) for name in
             ('windows_lan_recovery.py','windows_setup_native.py','windows_lan_setup.py','windows_lan_open.py')}
    members['recovery.json']=json.dumps(dict(schema=1,run_id=SESSION,app=APP,prior_receipt_sha256=PRIOR,source_commit=commit)).encode()
    members['WINDOWS-ONAR.cmd']=b'@echo off\r\ncd /d "%~dp0"\r\npy -3 -B windows_lan_recovery.py\r\npause\r\n'
    members['BASLA.txt']=('Onceki kurulumu yaptigin ana Windows bilgisayarinda bu ZIPi ayikla.\n'
        'Cryptex aciksa Quit ile cik. WINDOWS-ONAR.cmd dosyasini normal cift tikla ac.\n'
        'Yonetici olarak calistirma. Profil/anahtar/dosya silme.\n'
        'Paket mevcut uygulamanin SHA256 degerlerini ve profilin yalnizca baslangic isareti icerdigini kontrol eder.\n'
        'Ilk deneme sadece --help komutudur; profil veya anahtar olusturmaz.\n'
        'Baslatma basariliysa gosterilen public not ve devam sorusuna E diyebilirsin.\n'
        'Esitleme/guvenlik duvari/App Control ayarlari degistirilmez.\n'
        'Downloads icindeki ESITLEME-ONARIM-SONUC-Windows-*.zip dosyasini gonder.\n').encode()
    output=output.resolve()
    if output==ROOT or ROOT in output.parents:raise ValueError('output_inside_repository')
    with zipfile.ZipFile(output,'x',zipfile.ZIP_DEFLATED) as archive:
        for name,data in members.items():archive.writestr('Cryptex-Windows-Onarim/'+name,data)
    with zipfile.ZipFile(output) as archive:
        for name,data in members.items():
            if archive.read('Cryptex-Windows-Onarim/'+name)!=data:raise ValueError('packet_readback')
    return commit,hashlib.sha256(output.read_bytes()).hexdigest()
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--revision',default='HEAD');p.add_argument('--output',type=Path,required=True)
    a=p.parse_args();print(build(a.revision,a.output))
