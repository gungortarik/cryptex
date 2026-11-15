"""Export an explicitly FAILED startup candidate for interactive OS diagnosis."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import ssl
import stat
import zipfile
from datetime import datetime, timezone


def plain_bytes(root, relative):
    path = PurePosixPath(relative)
    if path.is_absolute() or '..' in path.parts or '\\' in relative:
        raise ValueError('diagnostic_member_invalid')
    file = root / relative
    metadata = file.lstat()
    if (file.resolve() != root.resolve() / relative or not stat.S_ISREG(metadata.st_mode)
            or metadata.st_nlink != 1 or metadata.st_size > 512 * 1024 * 1024):
        raise ValueError('diagnostic_member_unsafe')
    return file.read_bytes()


def export_failed(bundle, files, creator, destination, source, architecture, expires):
    if not re.fullmatch(r'[0-9a-f]{40}', source) or architecture not in ('arm64', 'x86_64'):
        raise ValueError('diagnostic_identity_invalid')
    if not expires.startswith('notAfter='):
        raise ValueError('diagnostic_expiry_missing')
    expiry = datetime.fromtimestamp(ssl.cert_time_to_seconds(expires[9:]), timezone.utc)
    members = {}
    required = {'Contents/MacOS/cryptex-desktop', 'Contents/MacOS/cryptexd',
                'Contents/Resources/frontend/index.html'}
    public_resources = {'Contents/Resources/startup-check.txt',
                        'Contents/Resources/transport/syncthing',
                        'Contents/Resources/transport/LICENSE.txt',
                        'Contents/Resources/transport/AUTHORS.txt'}
    for item in files:
        name = item['path']
        if name not in required | public_resources and not name.startswith('Contents/Resources/frontend/'):
            raise ValueError('diagnostic_inventory_invalid')
        data = plain_bytes(bundle, name)
        if len(data) != item['size'] or hashlib.sha256(data).hexdigest() != item['sha256']:
            raise ValueError('diagnostic_inventory_changed')
        members['development.app/' + name] = data
    if not required.issubset(item['path'] for item in files):
        raise ValueError('diagnostic_inventory_incomplete')
    for name in ('m8-startup.json', 'm8-startup.cms'):
        relative = 'Contents/Resources/' + name
        members['development.app/' + relative] = plain_bytes(bundle, relative)
    members['profile-creator'] = plain_bytes(creator.parent, creator.name)
    helpers = {'MAC-BASLANGIC-TANI.command': 'mac_development_diagnostic_owner.py',
               'mac_identity_preflight.py': 'mac_identity_preflight.py',
               'mac_development_startup_check.py': 'mac_development_startup_check.py',
               'mac_transport_check.py': 'mac_transport_check.py'}
    for name, source_name in helpers.items():
        members[name] = Path(__file__).with_name(source_name).read_bytes()
    meta = {'schema': 1, 'diagnostic_only': True, 'ci_startup_passed': False,
            'source': source, 'architecture': architecture, 'expires_utc': expiry.isoformat(),
            'files': {name: hashlib.sha256(data).hexdigest() for name, data in members.items()}}
    members['diagnostic.json'] = (json.dumps(meta, indent=2) + '\n').encode()
    executables = {'MAC-BASLANGIC-TANI.command', 'profile-creator',
                   'development.app/Contents/MacOS/cryptex-desktop',
                   'development.app/Contents/MacOS/cryptexd',
                   'development.app/Contents/Resources/transport/syncthing'}
    with zipfile.ZipFile(destination, 'x', compression=zipfile.ZIP_DEFLATED) as archive:
        for name, data in members.items():
            info = zipfile.ZipInfo('Cryptex-Mac-Baslangic-Tani/' + name)
            info.create_system = 3
            info.external_attr = (0o100700 if name in executables else 0o100600) << 16
            archive.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED)
