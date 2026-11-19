"""Bundle the same product revision for the two enrolled owner devices."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import uuid
import zipfile

from mac_lan_setup import source_members
from windows_lan_setup import HASHES

ROOT = Path(__file__).resolve().parents[2]
FOLDER = 'Cryptex-M9-Gecis/'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def build(artifact, transport_packet, revision, ci_run, output, *, reader=False):
    source = subprocess.check_output(['git', 'rev-parse', revision + '^{commit}'], cwd=ROOT, text=True).strip()
    provenance = json.loads((artifact / 'provenance.json').read_text(encoding='utf-8-sig'))
    if (provenance['source_commit'] != source or provenance['run_id'] != ci_run
            or provenance['target'] != 'x86_64-pc-windows-msvc'):
        raise ValueError('windows_provenance_mismatch')
    payload = {}
    for name in ('cryptexd.exe', 'cryptex-desktop.exe'):
        records = [r for r in provenance['files'] if r['name'] == name]
        raw = (artifact / name).read_bytes()
        if (len(records) != 1 or not 0 < len(raw) <= 64 * 1024 * 1024
                or records[0]['bytes'] != len(raw) or records[0]['sha256'].lower() != digest(raw)):
            raise ValueError('windows_product_hash_mismatch')
        payload[name] = raw
    with zipfile.ZipFile(transport_packet) as archive:
        for name, expected in HASHES.items():
            if not name.startswith('transport/'):
                continue
            matches = [i for i in archive.infolist() if i.filename.endswith('/payload/' + name)]
            if len(matches) != 1 or matches[0].file_size > 64 * 1024 * 1024:
                raise ValueError('transport_member_invalid')
            raw = archive.read(matches[0])
            if digest(raw) != expected:
                raise ValueError('transport_digest_mismatch')
            payload[name] = raw

    source_zip = subprocess.check_output(['git', 'archive', '--format=zip', source], cwd=ROOT)
    with zipfile.ZipFile(io.BytesIO(source_zip)) as archive:
        deps = {Path(i.filename).name: archive.read(i) for i in source_members(archive)
                if re.fullmatch(r'scripts/m8/(?:mac_[a-z_]+|windows_(?:vm_update|setup_native|pairing_diagnostic)|lan_(?:canonical_verification|transfer_diagnostic))\.py', i.filename)}
    members = dict(deps)
    members.update({'payload/' + name: raw for name, raw in payload.items()})
    members['source.zip'] = source_zip
    members['m9_owner_migration.py'] = Path(__file__).with_name('m9_owner_migration.py').read_bytes()
    plan = dict(schema=1, purpose='cryptex-owner-m9-layout', source_commit=source, ci_run=ci_run,
                source_sha256=digest(source_zip), run_id=str(uuid.uuid5(uuid.NAMESPACE_URL, 'cryptex/owner-m9/' + source)),
                hashes={name: digest(raw) for name, raw in payload.items()})
    members['candidate.json'] = json.dumps(plan, indent=2).encode()
    if reader:
        from package_m9_reader import reader_members
        folder, members = reader_members(members, plan)
        return write_packet(folder, members, output)
    for op, flag in (('GECIS', ''), ('AC', ' --open')):
        members['M9-' + op + '-MAC.command'] = (
            '#!/bin/zsh\nset -eu\ncd -- "${0:A:h}"\nexec /usr/bin/python3 -B m9_owner_migration.py' + flag + '\n').encode()
        members['M9-' + op + '-VM.cmd'] = (
            '@echo off\r\ncd /d "%~dp0"\r\nif not exist m9_owner_migration.py (echo ZIPi tamamen ayiklayin. & pause & exit /b 1)\r\n'
            'py -3 -B m9_owner_migration.py' + flag + '\r\nif errorlevel 1 (pause & exit /b 1)\r\n'
            + ('pause\r\n' if not flag else '')).encode()
    members['BASLA.txt'] = (
        'Ayni ZIPi Mac ve Windows VMde tamamen ayikla. Once iki Cryptex uygulamasindan normal cik.\n'
        'Mac: Command-Q. Windows VM: sistem tepsisi > Quit Cryptex.\n'
        'M9-GECIS-MAC.command ve M9-GECIS-VM.cmd dosyalarini kendi cihazinda ac. Ikisi ayni anda calisabilir.\n'
        'Terminali acik tut. Mac derleme yapar; Keychain sorarsa yalnizca macOS penceresine yanit ver.\n'
        'Sonucta her cihaz ayni klasore M9-GECIS-SONUC-*.zip birakir. Ikisini de sohbete gonder.\n'
        'Ayni paketi yeniden ayiklasan da gecis makbuzu cihazda tutulur. Profil ve eslestirme yerinde kalir.\n'
        'Ek yazma kapali. Basarili gecisten sonra M9-AC-MAC.command veya M9-AC-VM.cmd ile ac.\n'
        'Syncthing 2.1.5 MPL-2.0: https://github.com/syncthing/syncthing/tree/v2.1.5\n').encode()

    return write_packet(FOLDER, members, output)


def write_packet(folder, members, output):
    output = output.resolve()
    if output == ROOT or ROOT in output.parents:
        raise ValueError('output_inside_repository')
    with zipfile.ZipFile(output, 'x') as archive:
        for name, raw in members.items():
            info = zipfile.ZipInfo(folder + name)
            info.create_system = 3
            info.external_attr = (0o100700 if name.endswith('.command') else 0o100600) << 16
            archive.writestr(info, raw, compress_type=zipfile.ZIP_DEFLATED)
    with zipfile.ZipFile(output) as archive:
        for name, raw in members.items():
            if archive.read(folder + name) != raw:
                raise ValueError('package_readback_failed')
    return digest(output.read_bytes())


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--artifact', type=Path, required=True)
    parser.add_argument('--transport-packet', type=Path, required=True)
    parser.add_argument('--revision', required=True)
    parser.add_argument('--ci-run', required=True)
    parser.add_argument('--output', type=Path, required=True)
    print(build(**vars(parser.parse_args())))
