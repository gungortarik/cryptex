"""Build the bounded diagnostic from committed sources without product binaries."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[2]


def build(output):
    source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    files = {name: subprocess.check_output(['git', 'show', source + ':scripts/m8/' + name], cwd=ROOT)
             for name in ('lan_transfer_diagnostic.py', 'windows_setup_native.py', 'windows_pairing_diagnostic.py')}
    files['TANI-WINDOWS.cmd'] = b'@echo off\r\ncd /d "%~dp0"\r\npy -3 -B lan_transfer_diagnostic.py\r\npause\r\n'
    files['TANI-MAC.command'] = b'#!/bin/bash\ncd -- "$(dirname -- "$0")" || exit 1\n/usr/bin/python3 -B lan_transfer_diagnostic.py\nread -r -p "Kapatmak icin Enter..."\n'
    files['BASLA.txt'] = (
        'Cryptex her iki cihazda da acik kalsin. Ayarlari degistirme.\n'
        'ZIPi Windows VM ve Mac uzerinde ayri ayri ayikla. Ana Windows bilgisayarinda calistirma.\n'
        'VM: TANI-WINDOWS.cmd; Mac: TANI-MAC.command dosyasina cift tikla.\n'
        'Yonetici/sudo kullanma. Kurulum veya anahtar erisimi gerekmez.\n'
        'Her cihazda bu klasorde olusan AKTARIM-TANI-SONUC-*.zip dosyasini gonder.\n'
        'Arac sabit test profilinin kimlik/ayar metadatasini ve sabit klasorlerin dosya sayi/boyutlarini okur.\n'
        'Not govdeleri, anahtarlar, API kimlik bilgileri veya ham gunlukler okunmaz.\n'
        'Yalnizca 192.168.50.26:59957 ve 192.168.50.104:61942 aktarim portlarina TCP baglantisi denenir.\n'
        'Hicbir uygulama verisi gonderilmez. Profil, ayar, guvenlik duvari ve uygulama surecleri degistirilmez.\n'
        'TCP basarisi cihaz kimligi, not teslimi veya kalici kayit kaniti degildir.\n').encode()
    files['provenance.json'] = json.dumps(dict(source_commit=source,
        files={name: hashlib.sha256(data).hexdigest() for name, data in files.items()}), indent=2).encode()
    output = output.resolve()
    if ROOT == output or ROOT in output.parents:
        raise ValueError('output_inside_repository')
    with zipfile.ZipFile(output, 'x', zipfile.ZIP_DEFLATED) as archive:
        for name, data in files.items():
            info = zipfile.ZipInfo('Cryptex-Aktarim-Tani/' + name)
            info.create_system = 3
            info.external_attr = (0o100700 if name.endswith('.command') else 0o100600) << 16
            archive.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED)
    with zipfile.ZipFile(output) as archive:
        assert len(archive.infolist()) == len(files)
        for name, data in files.items():
            assert archive.read('Cryptex-Aktarim-Tani/' + name) == data
    return source, hashlib.sha256(output.read_bytes()).hexdigest()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True, type=Path)
    print(build(parser.parse_args().output))
