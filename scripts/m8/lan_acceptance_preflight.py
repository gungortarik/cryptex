#!/usr/bin/python3
"""Read-only device setup receipt for the real Windows/Mac LAN acceptance run.

No profile is created, opened by Cryptex, modified or deleted. Only public LAN
addresses, platform and fixed-path existence states are collected.
"""
import argparse
import hashlib
import ipaddress
import json
import os
from pathlib import Path
import platform
import re
import stat
import subprocess
import uuid
import zipfile


class Refused(Exception):
    pass


def run(args):
    try:
        outcome = subprocess.run(args, capture_output=True, timeout=10, check=False)
    except (OSError, subprocess.TimeoutExpired):
        raise Refused('system_observation_unavailable') from None
    if outcome.returncode or len(outcome.stdout) > 65536:
        raise Refused('system_observation_failed')
    return outcome.stdout.decode('utf-8', errors='strict')


def private_addresses(text):
    ranges = [ipaddress.ip_network(n) for n in ('10.0.0.0/8', '172.16.0.0/12', '192.168.0.0/16')]
    addresses = set()
    for raw in re.findall(r'(?<![0-9.])(?:[0-9]{1,3}\.){3}[0-9]{1,3}(?![0-9.])', text):
        try:
            address = ipaddress.IPv4Address(raw)
        except ValueError:
            continue
        if any(address in network for network in ranges):
            addresses.add(str(address))
    return sorted(addresses)


def path_state(path):
    """Inspect existence without opening any profile or following an alias."""
    if not path.is_absolute():
        raise Refused('profile_locator_not_absolute')
    cursor = Path(path.anchor)
    for name in path.parts[1:]:
        cursor = cursor / name
        try:
            metadata = cursor.lstat()
        except FileNotFoundError:
            return 'absent'
        except OSError:
            return 'unavailable'
        if (stat.S_ISLNK(metadata.st_mode)
                or getattr(metadata, 'st_file_attributes', 0) & 0x400):
            return 'unsafe_alias'
        if cursor != path and not stat.S_ISDIR(metadata.st_mode):
            return 'unsafe_parent'
    return 'directory' if stat.S_ISDIR(metadata.st_mode) else 'file' if stat.S_ISREG(metadata.st_mode) else 'unsafe_object'


def collect(system=None, home=None, environment=None, runner=run):
    system = system or platform.system()
    home = Path.home() if home is None else Path(home)
    environment = os.environ if environment is None else environment
    if system == 'Darwin':
        base = home / 'Library/Application Support/Cryptex-Dev'
        interfaces = runner(['/sbin/ifconfig', '-a'])
        addresses = private_addresses('\n'.join(
            re.findall(r'^\s*inet\s+(\S+)', interfaces, flags=re.MULTILINE)))
        version = runner(['/usr/bin/sw_vers', '-productVersion']).strip()
        if not re.fullmatch(r'\d+(?:\.\d+){0,2}', version):
            raise Refused('os_version_invalid')
    elif system == 'Windows':
        if not environment.get('LOCALAPPDATA') or not environment.get('SystemRoot'):
            raise Refused('windows_locator_missing')
        base = Path(environment['LOCALAPPDATA']) / 'Cryptex-Dev'
        shell = Path(environment['SystemRoot']) / 'System32/WindowsPowerShell/v1.0/powershell.exe'
        addresses = private_addresses(runner([str(shell), '-NoProfile', '-NonInteractive', '-Command',
            '$ErrorActionPreference="Stop"; Get-NetIPAddress -AddressFamily IPv4 | Where-Object AddressState -eq Preferred | Select-Object -ExpandProperty IPAddress']))
        version = platform.version()
    else:
        raise Refused('windows_or_macos_required')
    normal = base / 'runtime/p0-2'
    # Never enumerate other test roots or inspect any key, note, journal or DB.
    profile = {name: path_state(normal / relative) for name, relative in {
        'root': '.', 'brain_metadata': 'identity/brain_identity.json',
        'local_device_metadata': 'identity/local_device.json',
        'managed_transport': 'managed-transport',
        'connection_preferences': 'connection-settings',
    }.items()}
    return {
        'schema': 1, 'purpose': 'cryptex-real-lan-acceptance-preflight',
        'os': system, 'os_version': version, 'architecture': platform.machine(),
        'private_ipv4_candidates': addresses,
        'normal_development_profile': profile,
        'legacy_development_profile': path_state(base / 'p0-2'),
        'profile_writes': False, 'credentials_read': False,
        'memory_content_read': False, 'network_transfer_tested': False,
        'outcome': 'observed' if addresses else 'no_private_lan_address',
    }


def package(destination, revision):
    if not re.fullmatch(r'[0-9a-f]{40}', revision):
        raise Refused('source_revision_invalid')
    script = Path(__file__).read_bytes()
    launch = ('@echo off\r\ncd /d "%~dp0"\r\n'
              'where py >nul 2>nul\r\nif errorlevel 1 (\r\n'
              '  python ESITLEME-HAZIRLA.command\r\n) else (\r\n'
              '  py -3 ESITLEME-HAZIRLA.command\r\n)\r\npause\r\n').encode('ascii')
    metadata = json.dumps({'source': revision, 'script_sha256': hashlib.sha256(script).hexdigest()}, indent=2).encode()
    readme = ('Windows-Mac gercek esitleme testi: salt okunur hazirlik\n\n'
              'ZIP dosyasini once ayikla. Mac: ESITLEME-HAZIRLA.command dosyasina cift tikla.\n'
              'Windows: ESITLEME-HAZIRLA.cmd dosyasina cift tikla.\n'
              'Olusan ESITLEME-HAZIRLIK-SONUC-*.zip dosyasini gonder.\n'
              'Yalnizca isletim sistemi, mimari, yerel ag IPv4 adresleri ve sabit profil konumlarinin varligi raporlanir.\n'
              'Profil, anahtar, not ve ayarlar okunmaz/degistirilmez; uygulama baslatilmaz.\n'
              'Bu sonuc aktarimin veya M8 kabulunun gectigi anlamina gelmez.\n').encode('utf-8')
    with zipfile.ZipFile(destination, 'x', zipfile.ZIP_DEFLATED) as archive:
        for name, data in {'ESITLEME-HAZIRLA.command': script, 'ESITLEME-HAZIRLA.cmd': launch,
                           'package.json': metadata, 'BASLA.txt': readme}.items():
            info = zipfile.ZipInfo('Cryptex-LAN-Hazirlik/' + name)
            info.create_system = 3
            info.external_attr = (0o100700 if name.endswith('.command') else 0o100600) << 16
            archive.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--package', type=Path)
    parser.add_argument('--revision')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if args.package:
        package(args.package, args.revision or '')
        return
    directory = Path(__file__).absolute().parent
    metadata = json.loads((directory / 'package.json').read_text(encoding='utf-8'))
    if hashlib.sha256(Path(__file__).read_bytes()).hexdigest() != metadata['script_sha256']:
        raise Refused('collector_package_mismatch')
    print('Yerel ag ve profil konumlari kontrol ediliyor. Profil/anahtar/not degistirilmiyor.', flush=True)
    try:
        report = collect()
    except (Refused, OSError, UnicodeError) as error:
        report = {'schema': 1, 'outcome': 'refused', 'reason': str(error) if isinstance(error, Refused) else 'observation_unavailable'}
    report['source'] = metadata['source']
    report['collector_sha256'] = metadata['script_sha256']
    output = args.output or directory
    destination = output / ('ESITLEME-HAZIRLIK-SONUC-' + platform.system() + '-' + uuid.uuid4().hex[:12] + '.zip')
    with zipfile.ZipFile(destination, 'x', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json', json.dumps(report, indent=2) + '\n')
    print('Hazirlik sonucu: ' + str(destination), flush=True)
    print('Bu ZIP dosyasini sohbetten gonder. Henuz aktarim testi yapilmadi.', flush=True)


if __name__ == '__main__':
    main()
