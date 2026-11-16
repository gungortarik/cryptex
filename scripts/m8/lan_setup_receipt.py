"""Bounded owner setup observations; never pairing or network authority."""
import hashlib
import io
import json
from pathlib import Path
import re
import uuid
import zipfile

from lan_acceptance_receipt import unique_object
from lan_acceptance_preflight import Refused
from mac_lan_setup import BUILD_LABELS, BUILD_CODES

STAGES = ('product_built_and_signed', 'fresh_profile_created', 'first_note_committed',
          'transport_enrolled_local_only', 'initialized_startup_restart', 'persistent_profile_preserved')
BASE = {'schema', 'purpose', 'platform', 'pairing_authorized', 'network_delivery_verified',
        'convergence_verified', 'run_id', 'source_commit', 'outcome'}
DIGESTS = {'signer_certificate_sha256', 'first_note_sha256', 'release_inventory_sha256'}


def validate(report, source, session):
    if not re.fullmatch('[a-f0-9]{40}', source) or str(uuid.UUID(session)) != session:
        raise Refused('expected_setup_binding')
    if not isinstance(report, dict) or not BASE <= set(report):
        raise Refused('setup_receipt_shape')
    if (type(report['schema']) is not int or report['schema'] != 1
            or report['purpose'] != 'cryptex-persistent-lan-setup' or report['platform'] != 'Darwin'
            or report['run_id'] != session or report['source_commit'] != source):
        raise Refused('setup_receipt_binding')
    if any(report[k] is not False for k in ('pairing_authorized', 'network_delivery_verified', 'convergence_verified')):
        raise Refused('setup_receipt_scope')
    optional = {'build_tools', 'build_failure'}
    allowed = BASE | set(STAGES) | DIGESTS | {'failed_stage', 'error'} | optional
    if set(report) - allowed or any(report[k] is not True for k in STAGES if k in report):
        raise Refused('setup_receipt_fields')
    if 'build_tools' in report:
        versions = report['build_tools']
        if (not isinstance(versions, dict) or set(versions) - {'node', 'pnpm'}
                or any(not isinstance(v, str) or not re.fullmatch(r'\d{1,4}\.\d{1,4}\.\d{1,4}', v)
                       for v in versions.values())):
            raise Refused('setup_tool_versions')
    if 'build_failure' in report:
        diagnostic = report['build_failure']
        if (not isinstance(diagnostic, dict)
                or set(diagnostic) != {'stage', 'exit_code', 'timed_out', 'output_limited', 'pipe_failed', 'codes'}
                or not isinstance(diagnostic['stage'], str) or diagnostic['stage'] not in BUILD_LABELS
                or type(diagnostic['exit_code']) is not int or not -(2**31) <= diagnostic['exit_code'] < 2**31
                or any(type(diagnostic[k]) is not bool for k in ('timed_out', 'output_limited', 'pipe_failed'))
                or not isinstance(diagnostic['codes'], list) or len(diagnostic['codes']) > len(BUILD_CODES)
                or any(not isinstance(c, str) or c not in BUILD_CODES for c in diagnostic['codes'])
                or report['outcome'] != 'refused_or_incomplete'):
            raise Refused('setup_build_diagnostic')
    for key in DIGESTS:
        if key in report and (not isinstance(report[key], str) or not re.fullmatch('[a-f0-9]{64}', report[key])):
            raise Refused('setup_receipt_digest')
    if 'first_note_sha256' in report:
        expected = hashlib.sha256(('Cryptex LAN acceptance ' + session + ': Mac first public note.').encode()).hexdigest()
        if report['first_note_sha256'] != expected:
            raise Refused('setup_note_mismatch')
    if report['outcome'] == 'prepared_local_only':
        if set(report) - {'build_tools'} != BASE | set(STAGES) | DIGESTS:
            raise Refused('setup_receipt_incomplete_success')
    elif report['outcome'] == 'refused_or_incomplete':
        if 'failed_stage' not in report:
            raise Refused('setup_receipt_failure_missing')
        for key in ('failed_stage', 'error'):
            if key in report and (not isinstance(report[key], str) or not re.fullmatch('[a-z_]{1,100}', report[key])):
                raise Refused('setup_receipt_diagnostic')
    else:
        raise Refused('setup_receipt_outcome')
    return report


def read(path, source, session):
    try:
        with Path(path).open('rb') as stream:
            raw = stream.read(65537)
        if len(raw) > 65536:
            raise Refused('setup_receipt_size')
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            infos = archive.infolist()
            if (len(infos) != 1 or infos[0].filename != 'result.json'
                    or infos[0].orig_filename != 'result.json' or infos[0].file_size > 16384
                    or infos[0].flag_bits & 1):
                raise Refused('setup_receipt_archive')
            with archive.open(infos[0]) as stream:
                data = stream.read(16385)
            if len(data) > 16384:
                raise Refused('setup_receipt_json_size')
            report = json.loads(data.decode('utf-8'), object_pairs_hook=unique_object)
    except (OSError, ValueError, UnicodeError, zipfile.BadZipFile, RuntimeError, NotImplementedError, RecursionError):
        raise Refused('setup_receipt_unreadable') from None
    return validate(report, source, session), hashlib.sha256(raw).hexdigest()
