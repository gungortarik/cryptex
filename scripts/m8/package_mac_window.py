"""Repackage pinned public signed bytes; never rebuild, resign or export a key."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import zipfile

from mac_identity_preflight import unique_object

HELPERS = {
    'MAC-PENCERE.command': 'mac_window_owner.py',
    'mac_window_process.py': 'mac_window_process.py',
    'mac_identity_preflight.py': 'mac_identity_preflight.py',
    'mac_development_diagnostic_owner.py': 'mac_development_diagnostic_owner.py',
    'mac_development_startup_check.py': 'mac_development_startup_check.py',
    'mac_transport_check.py': 'mac_transport_check.py',
}


def build(source_zip, destination, expected_sha256, collector_source):
    if not re.fullmatch('[0-9a-f]{40}', collector_source):
        raise ValueError('collector_source_invalid')
    if hashlib.sha256(source_zip.read_bytes()).hexdigest() != expected_sha256:
        raise ValueError('source_archive_changed')
    prefix = 'Cryptex-Mac-Baslangic-Tani/'
    with zipfile.ZipFile(source_zip) as archive:
        infos = archive.infolist()
        if len(infos) > 4096 or sum(i.file_size for i in infos) > 512 * 1024 * 1024:
            raise ValueError('archive_too_large')
        names = [i.filename for i in infos]
        if len(set(names)) != len(names):
            raise ValueError('archive_duplicate_member')
        if archive.testzip() is not None:
            raise ValueError('archive_corrupt')
        meta = json.loads(archive.read(prefix + 'diagnostic.json'), object_pairs_hook=unique_object)
        if (meta.get('schema') != 1 or meta.get('diagnostic_only') is not True
                or meta.get('ci_startup_passed') is not False
                or not re.fullmatch('[0-9a-f]{40}', meta.get('source', ''))
                or meta.get('architecture') not in ('arm64', 'x86_64')):
            raise ValueError('source_metadata_invalid')
        if datetime.now(timezone.utc) >= datetime.fromisoformat(meta['expires_utc']):
            raise ValueError('source_expired')
        members = {}
        public = {'Contents/MacOS/cryptex-desktop', 'Contents/MacOS/cryptexd',
                  'Contents/Resources/m8-startup.json', 'Contents/Resources/m8-startup.cms',
                  'Contents/Resources/startup-check.txt', 'Contents/Resources/transport/syncthing',
                  'Contents/Resources/transport/LICENSE.txt', 'Contents/Resources/transport/AUTHORS.txt'}
        for name, digest in meta['files'].items():
            path = PurePosixPath(name)
            if path.is_absolute() or '..' in path.parts or '\\' in name:
                raise ValueError('source_path_invalid')
            if name == 'profile-creator' or name.startswith('development.app/'):
                relative = name.removeprefix('development.app/')
                if name != 'profile-creator' and relative not in public and not relative.startswith('Contents/Resources/frontend/'):
                    raise ValueError('source_member_not_public')
                data = archive.read(prefix + name)
                if hashlib.sha256(data).hexdigest() != digest:
                    raise ValueError('source_member_changed')
                members[name] = data
        required = {'profile-creator', 'development.app/Contents/MacOS/cryptex-desktop',
                    'development.app/Contents/MacOS/cryptexd',
                    'development.app/Contents/Resources/frontend/index.html',
                    'development.app/Contents/Resources/m8-startup.json',
                    'development.app/Contents/Resources/m8-startup.cms'}
        if not required.issubset(members):
            raise ValueError('source_members_missing')
    for name, file in HELPERS.items():
        members[name] = Path(__file__).with_name(file).read_bytes()
    members['BASLA.txt'] = (
        'Cryptex Mac pencere/kapanış testi — macOS 26+, ' + meta['architecture'] + '\n\n'
        'ZIP dosyasına çift tıkla. Açılan klasörde MAC-PENCERE.command dosyasına çift tıkla.\n'
        'Önce diğer Cryptex uygulamalarını Quit ile kapat. Terminali açık tut.\n'
        'Kod yapıştırmak gerekmez: ekrandaki adımları uygula, Terminalde E/H yazıp Enter bas.\n'
        'Paket önce ayrı test profilini hazırlar. İlk iki açılışta Open local memory seç.\n'
        'En sondaki açıklama iptal kontrolünde Terminal isteyince Quit seç.\n'
        'Paket seni taslakla gizle/aç, Dock, ikinci başlatma, Quit Cancel ve tam çıkıştan geçirir.\n'
        'Test taslağını KAYDETME. Hiçbir cihazı emekliye ayırma veya eşleştirme yapma.\n'
        'Şifreyi yalnızca macOS izin penceresine yaz; Terminale yazma.\n'
        'Hata ya da belirsizlik varsa H seç. Başarısız sonuç ZIP dosyasını da gönder.\n'
        'Sonunda aynı klasördeki MAC-PENCERE-SONUC-*.zip dosyasını gönder.\n\n'
        'Kişisel profil kullanılmaz; uygulama ayrı geçici test profilini açar.\n'
        'macOS kullanıcı/Keychain ortamı değiştirilmez.\n'
        'Yalnızca bu testin oluşturduğu anahtar ve geçici profil temizlenir.\n'
        'Görsel gözlemler sahibi tarafından bildirilir; otomatik süreç/kapanış sonuçlarından ayrıdır.\n'
        'Bu, kalıcı kurulum veya M8 kabulü değildir. Başsız CI Keychain aşaması geçmedi.\n'
        'Sistem uygulamayı engellerse korumaları kapatma; görünen mesajı paylaş.\n'
        'İmza geçerlilik sonu (UTC): ' + meta['expires_utc'] + '\n'
    ).encode('utf-8')
    meta.update(collector_source=collector_source, window_test='existing_profile_window_v2',
                scope='shared product UI via explicit existing-profile diagnostic; not default-profile acceptance',
                source_archive_sha256=expected_sha256,
                files={name: hashlib.sha256(data).hexdigest() for name, data in members.items()})
    members['diagnostic.json'] = (json.dumps(meta, indent=2) + '\n').encode('utf-8')
    executable = {'MAC-PENCERE.command', 'profile-creator', 'development.app/Contents/MacOS/cryptex-desktop',
                  'development.app/Contents/MacOS/cryptexd', 'development.app/Contents/Resources/transport/syncthing'}
    with zipfile.ZipFile(destination, 'x', compression=zipfile.ZIP_DEFLATED) as archive:
        for name, data in members.items():
            info = zipfile.ZipInfo('Cryptex-Mac-Pencere/' + name)
            info.create_system = 3
            info.external_attr = (0o100700 if name in executable else 0o100600) << 16
            archive.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('source', type=Path)
    parser.add_argument('destination', type=Path)
    parser.add_argument('--expected-sha256', required=True)
    parser.add_argument('--collector-source', required=True)
    args = parser.parse_args()
    build(args.source, args.destination, args.expected_sha256, args.collector_source)
