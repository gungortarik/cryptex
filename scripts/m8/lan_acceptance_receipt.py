"""Bounded setup-receipt input for the real LAN packet builder.

Receipts are untrusted observations, never identity or pairing authority.
Every profile/path condition must be checked again before actual initialization.
"""
import hashlib
import io
import ipaddress
import json
from pathlib import Path
import re
import uuid
import zipfile

from lan_acceptance_preflight import Refused, private_addresses

FIELDS = {
    'schema', 'purpose', 'os', 'os_version', 'architecture',
    'private_ipv4_candidates', 'normal_development_profile',
    'legacy_development_profile', 'profile_writes', 'credentials_read',
    'memory_content_read', 'network_transfer_tested', 'outcome',
    'source', 'collector_sha256',
}
PROFILE_FIELDS = {
    'root', 'brain_metadata', 'local_device_metadata',
    'managed_transport', 'connection_preferences',
}


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise Refused('duplicate_json_key')
        result[key] = value
    return result


def validate(report, system, source, collector_digest):
    if not isinstance(report, dict) or set(report) != FIELDS:
        raise Refused('receipt_shape')
    if (type(report['schema']) is not int or report['schema'] != 1
            or report['purpose'] != 'cryptex-real-lan-acceptance-preflight'
            or report['os'] != system or system not in ('Windows', 'Darwin')
            or report['outcome'] != 'observed'
            or report['source'] != source
            or report['collector_sha256'] != collector_digest):
        raise Refused('receipt_binding')
    if any(report[key] is not False for key in (
            'profile_writes', 'credentials_read', 'memory_content_read', 'network_transfer_tested')):
        raise Refused('receipt_scope')
    version = report['os_version']
    if not isinstance(version, str) or not re.fullmatch(r'\d{1,5}(?:\.\d{1,5}){0,3}', version):
        raise Refused('receipt_version')
    if system == 'Darwin' and (int(version.split('.')[0]) < 26 or report['architecture'] != 'arm64'):
        raise Refused('unsupported_mac_packet_target')
    if system == 'Windows' and report['architecture'] not in ('AMD64', 'x86_64'):
        raise Refused('unsupported_windows_packet_target')
    addresses = report['private_ipv4_candidates']
    if (not isinstance(addresses, list) or not 1 <= len(addresses) <= 32
            or any(not isinstance(a, str) or len(a) > 15 for a in addresses)
            or len(set(addresses)) != len(addresses)
            or sorted(addresses) != private_addresses('\n'.join(addresses))):
        raise Refused('receipt_lan_addresses')
    profile = report['normal_development_profile']
    if not isinstance(profile, dict) or set(profile) != PROFILE_FIELDS:
        raise Refused('receipt_profile_shape')
    if any(value != 'absent' for value in profile.values()):
        raise Refused('normal_profile_not_absent')
    if report['legacy_development_profile'] not in ('absent', 'directory'):
        raise Refused('legacy_locator_unsafe')
    return report


def read_receipt(path, system, source, collector_digest):
    if not re.fullmatch('[0-9a-f]{40}', source) or not re.fullmatch('[0-9a-f]{64}', collector_digest):
        raise Refused('expected_binding_invalid')
    try:
        with Path(path).open('rb') as stream:
            raw = stream.read(65537)
        if len(raw) > 65536:
            raise Refused('receipt_archive_limit')
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            infos = archive.infolist()
            if (len(infos) != 1 or infos[0].filename != 'result.json'
                    or infos[0].file_size > 16384 or infos[0].flag_bits & 1):
                raise Refused('receipt_archive_shape')
            with archive.open(infos[0]) as item:
                data = item.read(16385)
            if len(data) > 16384:
                raise Refused('receipt_json_limit')
            report = json.loads(data.decode('utf-8'), object_pairs_hook=unique_object)
    except (OSError, ValueError, UnicodeError, zipfile.BadZipFile, RuntimeError, NotImplementedError, RecursionError):
        raise Refused('receipt_unreadable') from None
    return validate(report, system, source, collector_digest), hashlib.sha256(raw).hexdigest()


def build_plan(windows, mac, *, windows_address, mac_address, route_prefix, next_hop, source):
    """Caller supplies the current OS-selected direct route, not ZIP routing text."""
    if (windows_address not in windows['private_ipv4_candidates']
            or mac_address not in mac['private_ipv4_candidates']
            or windows_address == mac_address or next_hop != '0.0.0.0'
            or not re.fullmatch('[0-9a-f]{40}', source)):
        raise Refused('direct_lan_route_required')
    try:
        network = ipaddress.IPv4Network(route_prefix, strict=True)
        local, peer = map(ipaddress.IPv4Address, (windows_address, mac_address))
    except ValueError:
        raise Refused('direct_lan_route_required') from None
    if (network.prefixlen < 8 or local not in network or peer not in network
            or local in (network.network_address, network.broadcast_address)
            or peer in (network.network_address, network.broadcast_address)):
        raise Refused('direct_lan_route_required')
    return {
        'schema': 1, 'purpose': 'cryptex-persistent-lan-acceptance-plan',
        'run_id': str(uuid.uuid4()), 'product_source': source,
        'windows': {'architecture': windows['architecture'], 'lan_address': windows_address},
        'mac': {'architecture': mac['architecture'], 'lan_address': mac_address},
        'profile_policy': 'create_only_if_normal_development_root_still_absent',
        'legacy_profile_policy': 'do_not_open_adopt_move_or_delete',
        'recheck_at_launch_required': True,
        'pairing_authorized': False,
        'network_delivery_verified': False,
        'durable_convergence_verified': False,
    }
