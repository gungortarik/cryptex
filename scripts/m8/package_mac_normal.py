"""Normal-launch collector over byte-identical, previously tested signed images."""
import argparse
import hashlib
import json
from pathlib import Path
import tempfile
import zipfile

from package_mac_window import build as window_package


def build(source, destination, expected_sha256, collector_source):
    with tempfile.TemporaryDirectory(prefix='cryptex-normal-package-') as temporary:
        intermediate = Path(temporary) / 'window.zip'
        window_package(source, intermediate, expected_sha256, collector_source)
        with zipfile.ZipFile(intermediate) as archive:
            members = {i.filename.removeprefix('Cryptex-Mac-Pencere/'): archive.read(i) for i in archive.infolist()}
    meta = json.loads(members.pop('diagnostic.json'))
    members['mac_window_owner.py'] = members.pop('MAC-PENCERE.command')
    for target, filename in (('MAC-NORMAL.command', 'mac_normal_owner.py'),
                             ('mac_normal_profile.py', 'mac_normal_profile.py')):
        members[target] = Path(__file__).with_name(filename).read_bytes()
    members['BASLA.txt'] = (
        'Cryptex normal açılış testi — macOS 26+, Apple Silicon\n\n'
        'ZIP dosyasını Mac’te aç. MAC-NORMAL.command dosyasına çift tıkla.\n'
        'Önce diğer Cryptex uygulamalarından Quit ile çık. Terminali açık tut.\n'
        'Mevcut varsayılan profil varsa test onu değiştirmeden durur; sonucu yine gönder.\n'
        'Konum boşsa daemon yeni, yalnızca test için bir profil hazırlar.\n'
        'İki kez Open local memory ile aç, ana pencereyi doğrula ve Command-Q ile çık.\n'
        'Not kaydetme; güvenlik/eşleştirme işlemi yapma. Terminalde yalnızca E/H yanıtı ver.\n'
        'Şifreyi yalnızca macOS izin penceresine yaz.\n'
        'Başarıda test profili ve yalnızca onun anahtarı temizlenir; belirsizlikte korunur.\n'
        'Bitince MAC-NORMAL-SONUC-*.zip dosyasını gönder.\n'
        'Bu test normal konumu ve yeniden açılışı sınar; kişisel profil taşıma veya M8 kabulü değildir.\n'
    ).encode('utf-8')
    meta.pop('window_test', None)
    meta.update(normal_test='absent_default_root_v1',
                scope='normal no-argument opening/restart with synthetic fixture at absent default root',
                files={name: hashlib.sha256(data).hexdigest() for name, data in members.items()})
    members['diagnostic.json'] = (json.dumps(meta, indent=2) + '\n').encode('utf-8')
    executables = {'MAC-NORMAL.command', 'profile-creator', 'development.app/Contents/MacOS/cryptex-desktop',
                   'development.app/Contents/MacOS/cryptexd', 'development.app/Contents/Resources/transport/syncthing'}
    with zipfile.ZipFile(destination, 'x', compression=zipfile.ZIP_DEFLATED) as archive:
        for name, data in members.items():
            info = zipfile.ZipInfo('Cryptex-Mac-Normal/' + name)
            info.create_system = 3
            info.external_attr = (0o100700 if name in executables else 0o100600) << 16
            archive.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('source', type=Path)
    parser.add_argument('destination', type=Path)
    parser.add_argument('--expected-sha256', required=True)
    parser.add_argument('--collector-source', required=True)
    args = parser.parse_args()
    build(args.source, args.destination, args.expected_sha256, args.collector_source)
