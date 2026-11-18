"""One shared run plan, exact committed Mac source and CI Windows daemon."""
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


def build(artifact, revision, output):
    source = subprocess.check_output(['git', 'rev-parse', revision+'^{commit}'], cwd=ROOT, text=True).strip()
    provenance = json.loads((artifact/'provenance.json').read_text(encoding='utf-8-sig'))
    daemon = (artifact/'cryptexd.exe').read_bytes()
    digest = hashlib.sha256(daemon).hexdigest()
    if (provenance['source_commit'] != source or provenance['daemon_sha256'] != digest
            or len(daemon) > 64*1024*1024):
        raise ValueError('artifact_provenance_mismatch')
    raw = subprocess.check_output(['git', 'archive', '--format=zip', source], cwd=ROOT)
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        source_members(archive)
        members = {Path(info.filename).name: archive.read(info) for info in archive.infolist()
                   if info.filename.startswith('scripts/m8/') and info.filename.endswith('.py')
                   and '/' not in info.filename.removeprefix('scripts/m8/')}
    if 'lan_canonical_verification.py' not in members:
        raise ValueError('collector_not_committed')
    run_id = str(uuid.uuid4())
    members.update({'source.zip': raw, 'payload/cryptexd.exe': daemon,
        'verification.json': (json.dumps(dict(schema=1, purpose='cryptex-local-verification-packet',
            run_id=run_id, source_commit=source, source_sha256=hashlib.sha256(raw).hexdigest(),
            windows_daemon_sha256=digest), indent=2)+'\n').encode(),
        'BUTUNLUK-WINDOWS.cmd': b'@echo off\r\ncd /d "%~dp0"\r\npy -3 -B lan_canonical_verification.py\r\npause\r\n',
        'BUTUNLUK-MAC.command': b'#!/bin/zsh\nset -eu\ncd -- "${0:A:h}"\n/usr/bin/python3 -B lan_canonical_verification.py\n',
        'BASLA.txt': (
            'Ayni ZIPi Windows VM ve kendi Macinde ayikla. Ana Windows cihazinda calistirma.\n'
            'Ikisinde de Cryptex normal bicimde kapali olmali: Mac Command-Q; VM Quit Cryptex.\n'
            'Profil silme, yeniden eslestirme veya yeni not gerekmez. End Task kullanma.\n'
            'VM: BUTUNLUK-WINDOWS.cmd; Mac: BUTUNLUK-MAC.command dosyasina cift tikla.\n'
            'Mac mevcut yerel sertifikanla ayri bir dogrulama uygulamasi derler; eski uygulama degismez.\n'
            'Keychain izni sorulursa sifreyi sadece macOS penceresine yaz.\n'
            'Islem bitince her cihazdaki BUTUNLUK-SONUC-*.zip dosyasini gonder.\n'
            'Rapor imzali kayit kumelerini yerelde dogrular; not icerigi veya anahtar disa cikmaz.\n'
            'Daemon kilitleri disinda profil verisi degistirilmez; yarim kalmis islem varsa onarim yapilmaz.\n'
            'Bu paket ag kesintisi testi veya M8 tamamlandi kaniti degildir.\n'
            'Islemden sonra normal Cryptex kisayolunu kullanabilirsin.\n').encode()})
    output = output.resolve()
    if output == ROOT or ROOT in output.parents:
        raise ValueError('output_inside_repository')
    with zipfile.ZipFile(output, 'x', zipfile.ZIP_DEFLATED) as archive:
        for name, data in members.items():
            info = zipfile.ZipInfo('Cryptex-Butunluk/'+name)
            info.create_system = 3
            info.external_attr = (0o100700 if name.endswith('.command') else 0o100600) << 16
            archive.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED)
    with zipfile.ZipFile(output) as archive:
        for name, data in members.items():
            if archive.read('Cryptex-Butunluk/'+name) != data:
                raise ValueError('packet_readback_mismatch')
    return dict(source=source, run_id=run_id, sha256=hashlib.sha256(output.read_bytes()).hexdigest())


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--artifact', type=Path, required=True)
    parser.add_argument('--revision', default='HEAD')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(build(args.artifact, args.revision, args.output)))
