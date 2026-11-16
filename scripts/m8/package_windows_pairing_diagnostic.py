"""Package read-only VM diagnostic from exact committed sources."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

ROOT=Path(__file__).resolve().parents[2]


def build(output):
    commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    members={name:subprocess.check_output(['git','show',commit+':scripts/m8/'+name],cwd=ROOT)
             for name in ('windows_pairing_diagnostic.py','windows_setup_native.py')}
    members['TANI.cmd']=(b'@echo off\r\ncd /d "%~dp0"\r\n'
                         b'py -3 -B windows_pairing_diagnostic.py\r\npause\r\n')
    members['BASLA.txt']=(
        'Yalnizca eslestirme yaptigin Windows VM icinde calistir.\n'
        'ZIPi Tumunu ayikla ile ac, TANI.cmd dosyasina cift tikla. Yonetici olarak acma.\n'
        'Cryptex acik kalabilir. Anahtarlar, not icerikleri ve ham gunlukler alinmaz.\n'
        'Bu arac uygulama baslatmaz/kapatmaz, ag veya profil ayarlarini degistirmez.\n'
        'Yalnizca sabit VM kimligi icin eslestirme durumunu ve sinirli dosya sayilarini okur.\n'
        'Canli baglantinin dahili hata kodunu okuyamaz; bu rapor tani icindir, yetki kaniti degildir.\n'
        'VMnin Downloads klasorunde olusan VM-ESLESTIRME-TANI-*.zip dosyasini gonder.\n').encode()
    members['provenance.json']=json.dumps({'schema':1,'source_commit':commit,'read_only_diagnostic':True}).encode()
    output=output.resolve()
    if output==ROOT or ROOT in output.parents: raise ValueError('output_inside_repository')
    with zipfile.ZipFile(output,'x',zipfile.ZIP_DEFLATED) as archive:
        for name,data in members.items(): archive.writestr('Cryptex-VM-Eslesme-Tani/'+name,data)
    with zipfile.ZipFile(output) as archive:
        assert len(archive.infolist())==len(members)
        for name,data in members.items():
            assert archive.read('Cryptex-VM-Eslesme-Tani/'+name)==data
    return commit,hashlib.sha256(output.read_bytes()).hexdigest()


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--output',type=Path,required=True)
    print(build(parser.parse_args().output))
