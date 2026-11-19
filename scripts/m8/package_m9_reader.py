"""Package the reader candidate with only the two platform launchers."""
import argparse
import json
from pathlib import Path
import uuid


def reader_members(members, plan):
    plan = dict(plan, purpose='cryptex-owner-m9-reader', run_id=str(uuid.uuid4()))
    members['candidate.json'] = json.dumps(plan, indent=2).encode()
    members['m9_owner_reader.py'] = Path(__file__).with_name('m9_owner_reader.py').read_bytes()
    members['M9-OKUYUCU-MAC.command'] = (
        '#!/bin/zsh\nset -eu\ncd -- "${0:A:h}"\nexec /usr/bin/python3 -B m9_owner_reader.py\n').encode()
    members['M9-OKUYUCU-VM.cmd'] = (
        '@echo off\r\ncd /d "%~dp0"\r\nif not exist m9_owner_reader.py '
        '(echo ZIP dosyasini tamamen ayiklayin. & pause & exit /b 1)\r\n'
        'py -3 -B m9_owner_reader.py\r\nset "reader_result=%errorlevel%"\r\npause\r\nexit /b %reader_result%\r\n').encode()
    members['BASLA.txt'] = (
        'ZIP dosyasını Mac ve Windows VM içinde tamamen ayıkla. İki cihaz aynı yerel ağda ve uyanık olsun.\n'
        'Açık Cryptex varsa önce normal çıkış yap. Mac’te Command-Q, VM’de sistem tepsisinden Quit Cryptex.\n'
        'Mac’te M9-OKUYUCU-MAC.command dosyasını aç. İlk açılışta uygulama Mac’te derlenip imzalanır.\n'
        'Mac terminalinde MAC HAZIR görünce Windows VM’de M9-OKUYUCU-VM.cmd dosyasını aç.\n'
        'Mac Open local memory veya Keychain izni sorarsa kendi cihazında yanıtla. Her iki uygulamayı açık bırak.\n'
        'İki terminal sonucu kendiliğinden yazar. Not ekleme veya arama gerekmiyor.\n'
        'İki M9-OKUYUCU-SONUC ZIP dosyasını bu sohbete gönder. Her biri çalıştırdığın dosyanın yanındadır.\n'
        'Aynı klasörde iki sonuç varsa paket bunları karşılaştırır. Ek yazma ve ek aktarımı kapalıdır.\n'
        'Syncthing 2.1.5 MPL-2.0 https://github.com/syncthing/syncthing/tree/v2.1.5\n').encode()
    return 'Cryptex-M9-Okuyucu/', members


if __name__ == '__main__':
    from package_m9_owner_migration import build
    parser = argparse.ArgumentParser()
    parser.add_argument('--artifact', type=Path, required=True)
    parser.add_argument('--transport-packet', type=Path, required=True)
    parser.add_argument('--revision', required=True)
    parser.add_argument('--ci-run', required=True)
    parser.add_argument('--output', type=Path, required=True)
    print(build(**vars(parser.parse_args()), reader=True))
