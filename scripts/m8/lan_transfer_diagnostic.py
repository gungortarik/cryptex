#!/usr/bin/python3
"""Session-bound observations, never a canonical writer or trust verifier."""
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import socket
import stat
import uuid
import zipfile

BRAIN = 'b2ad8bb6-2d65-4690-b3c6-6a7b3f5adac4'
WINDOWS = 'bde7cefd-2f40-40f1-a22e-ae1fb123e8a8'
MAC = '1d17c886-a6cf-4422-aba5-fc985656741d'
SIDES = {'Windows': (WINDOWS, MAC, '192.168.50.26', 59957, '192.168.50.104', 61942),
         'Darwin': (MAC, WINDOWS, '192.168.50.104', 61942, '192.168.50.26', 59957)}


class Refused(Exception):
    pass


@contextmanager
def directory(path):
    """Pin every existing directory; never create or follow aliases."""
    path = Path(path)
    if not path.is_absolute() or '..' in path.parts:
        raise Refused('unsafe_path')
    if os.name == 'nt':
        from windows_setup_native import Pins, Refused as NativeRefused
        try:
            with Pins() as pins:
                pins.chain(path)
                yield path
        except NativeRefused:
            raise Refused('path_unavailable') from None
    else:
        descriptors = []
        try:
            flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
            fd = os.open('/', flags)
            descriptors.append(fd)
            for name in path.parts[1:]:
                fd = os.open(name, flags, dir_fd=fd)
                descriptors.append(fd)
            yield fd
        finally:
            for fd in reversed(descriptors):
                os.close(fd)


def unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise Refused('duplicate_metadata_key')
        result[key] = value
    return result


def read_metadata(path):
    if os.name == 'nt':
        from windows_pairing_diagnostic import read_json, Refused as NativeRefused
        try:
            return read_json(path)
        except NativeRefused:
            raise Refused('metadata_unavailable') from None
    with directory(path.parent) as parent:
        fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
                     dir_fd=parent)
        with os.fdopen(fd, 'rb') as stream:
            before = os.fstat(stream.fileno())
            if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1
                    or before.st_uid != os.getuid()):
                raise Refused('unsafe_metadata')
            raw = stream.read(1024 * 1024 + 1)
            after = os.fstat(stream.fileno())
            if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
                raise Refused('metadata_changed')
    if len(raw) > 1024 * 1024:
        raise Refused('metadata_bound')
    value = json.loads(raw, object_pairs_hook=unique)
    if not isinstance(value, dict):
        raise Refused('metadata_shape')
    return value


def inventory(path):
    result = dict(files=0, directories=0, unsafe=0, sealed_segments=0,
                  open_files=0, sealed_bytes=0, open_bytes=0, segments=[])
    with directory(path) as handle, os.scandir(handle) as entries:
        for index, entry in enumerate(entries):
            if index >= 4096:
                raise Refused('inventory_bound')
            # Windows DirEntry's cached metadata can report st_nlink=0.
            # Query the pinned directory entry explicitly for link counts.
            info = (os.stat(entry.path, follow_symlinks=False) if os.name == 'nt'
                    else entry.stat(follow_symlinks=False))
            if stat.S_ISLNK(info.st_mode) or getattr(info, 'st_file_attributes', 0) & 0x400:
                result['unsafe'] += 1
            elif stat.S_ISDIR(info.st_mode):
                result['directories'] += 1
            elif stat.S_ISREG(info.st_mode):
                result['files'] += 1
                if info.st_nlink != 1:
                    result['unsafe'] += 1
                    continue
                if re.fullmatch(r'[0-9]{20}\.seg', entry.name):
                    result['sealed_segments'] += 1
                    result['sealed_bytes'] += info.st_size
                    if len(result['segments']) < 128:
                        result['segments'].append([entry.name[:20], info.st_size])
                elif entry.name.endswith('.open'):
                    result['open_files'] += 1
                    result['open_bytes'] += info.st_size
            else:
                result['unsafe'] += 1
    result['segments'].sort()
    result['segment_list_truncated'] = result['sealed_segments'] > 128
    return result


def observe(operation):
    try:
        return {'status': 'observed', 'value': operation()}
    except FileNotFoundError:
        return {'status': 'absent'}
    except Refused as error:
        return {'status': 'unavailable', 'reason': str(error)}
    except OSError as error:
        return {'status': 'unavailable', 'reason': 'io_error', 'errno': error.errno,
                'winerror': getattr(error, 'winerror', None)}
    except (ValueError, TypeError, KeyError, RecursionError):
        return {'status': 'unavailable', 'reason': 'invalid_metadata'}


def tcp_probe(local_ip, destination, port):
    # Connect and immediately close; no TLS, API credential or application data.
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as stream:
        stream.settimeout(3)
        stream.bind((local_ip, 0))
        stream.connect((destination, port))
    return 'tcp_connected_identity_not_verified'


def collect(root, system, probe=tcp_probe):
    local, peer, ip, port, peer_ip, peer_port = SIDES[system]
    brain = read_metadata(root / 'identity/brain_identity.json')
    device = read_metadata(root / 'identity/local_device.json')
    if (brain.get('brain_id') != BRAIN or device.get('brain_id') != BRAIN
            or device.get('device_id') != local):
        raise Refused('wrong_session_profile')
    report = dict(schema=1, purpose='cryptex-lan-transfer-diagnostic', os=system,
                  outcome='observed', atomic_snapshot=False, profile_writes=False,
                  memory_bodies_read=False, credentials_read=False,
                  canonical_signatures_verified=False, network_delivery_verified=False)
    paths = {'canonical_local': 'canonical/events/' + local.encode().hex(),
             'canonical_peer': 'canonical/events/' + peer.encode().hex(),
             'exchange_local': 'transport/incoming/' + local.encode().hex(),
             'exchange_peer': 'transport/incoming/' + peer.encode().hex(),
             'import_journal': 'canonical/import-journal',
             'import_staging': 'canonical/import-staging',
             'export_staging': 'transport/export-staging'}
    report['directories'] = {key: observe(lambda p=root / relative: inventory(p))
                             for key, relative in paths.items()}

    def settings():
        value = read_metadata(root / 'connection-settings/connections.json')
        if value.get('schema') != 1 or type(value.get('paused')) is not bool:
            raise Refused('settings_schema')
        peers = value.get('peers')
        if not isinstance(peers, list) or len(peers) > 64 or any(not isinstance(p, dict) for p in peers):
            raise Refused('settings_shape')
        return dict(paused=value['paused'], local_ip_matches=value.get('listen') == ip,
                    peer_count=len(peers), peer_endpoint_matches=any(
                        p.get('device_id') == peer and p.get('endpoint') == f'{peer_ip}:{peer_port}'
                        for p in peers))
    report['settings'] = observe(settings)
    report['tcp_local'] = observe(lambda: probe(ip, ip, port))
    report['tcp_peer'] = observe(lambda: probe(ip, peer_ip, peer_port))
    return report


def main():
    os.umask(0o077)
    system = platform.system()
    report = dict(schema=1, purpose='cryptex-lan-transfer-diagnostic', outcome='refused', os=system)
    try:
        if system not in SIDES:
            raise Refused('unsupported_platform')
        if system == 'Windows':
            import ctypes
            if ctypes.windll.shell32.IsUserAnAdmin():
                raise Refused('standard_user_required')
            root = Path(os.environ['LOCALAPPDATA']) / 'Cryptex-Dev/runtime/p0-2'
        else:
            if os.geteuid() == 0:
                raise Refused('standard_user_required')
            root = Path.home() / 'Library/Application Support/Cryptex-Dev/runtime/p0-2'
        report = collect(root, system)
    except Refused as error:
        report['error'] = str(error)
    except Exception:
        report['error'] = 'collector_unavailable'
    report['collector_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    output = Path(__file__).absolute().parent / f'AKTARIM-TANI-SONUC-{system}-{uuid.uuid4().hex[:12]}.zip'
    with zipfile.ZipFile(output, 'x', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json', json.dumps(report, indent=2) + '\n')
    print('Profil degistirilmedi. Gonderilecek sonuc: ' + str(output), flush=True)


if __name__ == '__main__':
    main()
