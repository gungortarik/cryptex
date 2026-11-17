"""Exact committed source, signed only on the owner's Mac; no profile payload."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import zipfile

from mac_lan_setup import source_members

ROOT = Path(__file__).resolve().parents[2]


def build(revision, destination):
    commit = subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', '--verify',
                                      revision + '^{commit}'], text=True).strip()
    raw = subprocess.check_output(['git', '-C', str(ROOT), 'archive', '--format=zip', commit])
    with zipfile.ZipFile(io.BytesIO(raw)) as source:
        source_members(source)
        members = {Path(i.filename).name: source.read(i) for i in source.infolist()
                   if i.filename.startswith('scripts/m8/')
                   and '/' not in i.filename.removeprefix('scripts/m8/')
                   and i.filename.endswith('.py')}
    if 'mac_existing_update.py' not in members:
        raise ValueError('updater_not_committed')
    members['source.zip'] = raw
    members['update.json'] = (json.dumps(dict(schema=1, purpose='cryptex-existing-mac-update',
        source_commit=commit, source_sha256=hashlib.sha256(raw).hexdigest()), indent=2)+'\n').encode()
    members['MAC-GUNCELLE.command'] = (
        '#!/bin/zsh\nset -eu\ncd -- "${0:A:h}"\n/usr/bin/python3 -B mac_existing_update.py\n').encode()
    members['BASLA.txt'] = (
        'Mac uyku sonrası bağlantı düzeltmesi\n\n'
        'Windows VM açık kalsın. Mac Cryptex’ten Command-Q ile çık.\n'
        'ZIP’i Mac’te aç, MAC-GUNCELLE.command dosyasına çift tıkla.\n'
        'Yeni uygulama ayrı klasörde derlenir; mevcut profil ve eşleştirme korunur.\n'
        'Mevcut yerel sertifikan kullanılır. macOS izin sorarsa şifreyi yalnızca macOS penceresine yaz.\n'
        'Derleme bitince uygulama açılır: Open local memory seç. Terminali açık bırak.\n'
        'Aynı klasörde MAC-GUNCELLEME-SONUC-*.zip oluşur; uygulama açıkken gönder.\n'
        'Sonraki açılışlarda bu klasördeki CRYPTEX-AC-*.command kullanılır.\n'
        'Hata olursa profil silme veya yeniden eşleştirme yapma; sonuç ZIP’ini gönder.\n'
        'Paket sonucu derleme/kurulum kanıtıdır; canlı aktarım ve uyku sonrası kabul ayrıca doğrulanır.\n'
    ).encode()
    destination = destination.resolve()
    if destination == ROOT or ROOT in destination.parents:
        raise ValueError('packet_output_inside_repository')
    with zipfile.ZipFile(destination, 'x', zipfile.ZIP_DEFLATED) as archive:
        for name, data in members.items():
            entry = zipfile.ZipInfo('Cryptex-Mac-Guncelle/' + name)
            entry.create_system = 3
            entry.external_attr = (0o100700 if name.endswith('.command') else 0o100600) << 16
            archive.writestr(entry, data, compress_type=zipfile.ZIP_DEFLATED)
    with zipfile.ZipFile(destination) as archive:
        for name, data in members.items():
            if archive.read('Cryptex-Mac-Guncelle/' + name) != data:
                raise ValueError('packet_readback_changed')
    return commit, hashlib.sha256(destination.read_bytes()).hexdigest()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--revision', default='HEAD')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(build(args.revision, args.output))
