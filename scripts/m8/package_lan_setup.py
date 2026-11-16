"""Exact-source Mac local-signing/setup packet; no CI key or runtime data."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import uuid
import zipfile
from mac_lan_setup import source_members

ROOT = Path(__file__).resolve().parents[2]

def build(revision, destination, run_id):
    run_id = str(uuid.UUID(run_id))
    commit = subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', '--verify', revision+'^{commit}'], text=True).strip()
    raw = subprocess.check_output(['git', '-C', str(ROOT), 'archive', '--format=zip', commit])
    with zipfile.ZipFile(io.BytesIO(raw)) as source:
        source_members(source)
        helpers = {Path(i.filename).name: source.read(i) for i in source.infolist()
                   if i.filename.startswith('scripts/m8/') and '/' not in i.filename.removeprefix('scripts/m8/')
                   and i.filename.endswith('.py')}
    if 'mac_lan_setup.py' not in helpers:
        raise ValueError('setup_source_not_committed')
    members = {**helpers, 'source.zip': raw}
    members['MAC-ESITLEME-KUR.command'] = (
        '#!/bin/zsh\nset -eu\ncd -- "${0:A:h}"\n'
        '/usr/bin/python3 -B mac_lan_setup.py\n'
    ).encode()
    plan = dict(schema=1, purpose='cryptex-persistent-lan-setup', run_id=run_id, brain_id=run_id,
                source_commit=commit, source_sha256=hashlib.sha256(raw).hexdigest())
    members['setup.json'] = (json.dumps(plan, indent=2)+'\n').encode()
    members['BASLA.txt'] = (
        'Cryptex Windows–Mac kalıcı eşitleme: Mac kurulumu\n\n'
        'Önce açık Cryptex uygulamalarından Quit ile çık. ZIP’i kendi Mac’inde aç.\n'
        'MAC-ESITLEME-KUR.command dosyasına çift tıkla. Terminali açık bırak.\n'
        'İlk derleme birkaç dakika sürebilir; mevcut Rust, Node ve pnpm kullanılır.\n'
        'Yalnızca kendi Keychain’indeki mevcut yerel sertifikayla imzalar; özel anahtar dışarı çıkmaz.\n'
        'macOS izin isterse şifreni yalnızca macOS penceresine yaz.\n'
        'Paket ilk public test notunu gösterir; kuruluma E ile onay ver.\n'
        'Mevcut normal profil varsa dokunmadan durur. Eski profil kullanılmaz veya silinmez.\n'
        'Yeni profil, anahtarlar ve uygulama sonraki eşleştirme için kalıcı tutulur.\n'
        'Hata olursa yeniden denemek için profil silme; oluşan sonucu gönder.\n'
        'Bitince Downloads içindeki ESITLEME-KURULUM-SONUC-Darwin-*.zip dosyasını gönder.\n'
        'Bu adım kurulum + yeniden açılıştır. Eşleştirme ve gerçek LAN teslimi sonraki adımdır.\n'
    ).encode()
    destination = destination.resolve()
    if destination == ROOT or ROOT in destination.parents:
        raise ValueError('packet_output_inside_repository')
    with zipfile.ZipFile(destination, 'x', zipfile.ZIP_DEFLATED) as archive:
        for name, data in members.items():
            entry = zipfile.ZipInfo('Cryptex-Mac-Esitleme-Kurulum/'+name)
            entry.create_system = 3
            entry.external_attr = (0o100700 if name.endswith('.command') else 0o100600) << 16
            archive.writestr(entry, data, compress_type=zipfile.ZIP_DEFLATED)
    with zipfile.ZipFile(destination) as archive:
        for name, data in members.items():
            if archive.read('Cryptex-Mac-Esitleme-Kurulum/'+name) != data:
                raise ValueError('packet_readback_changed')
    return commit, hashlib.sha256(destination.read_bytes()).hexdigest()

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--revision', default='HEAD')
    parser.add_argument('--run-id', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(build(args.revision, args.output, args.run_id))
