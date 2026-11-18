"""Collect daemon-verified local aggregates; never infer network delivery."""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import uuid
import zipfile

from lan_transfer_diagnostic import BRAIN, MAC, WINDOWS, Refused, read_metadata

FIELDS = {'schema', 'purpose', 'run_id', 'brain_id', 'local_device_id', 'peer_device_id',
          'canonical_set_sha256', 'canonical_records', 'canonical_devices',
          'signatures_and_chains_verified', 'remote_durability_verified',
          'network_delivery_verified', 'derived_results_verified'}


def validate_receipt(value, run_id, local, peer):
    if (not isinstance(value, dict) or set(value) != FIELDS
            or type(value['schema']) is not int or value['schema'] != 1
            or value['purpose'] != 'cryptex-local-canonical-verification'
            or value['run_id'] != run_id or value['brain_id'] != BRAIN
            or value['local_device_id'] != local or value['peer_device_id'] != peer
            or not isinstance(value['canonical_set_sha256'], str)
            or not re.fullmatch('[a-f0-9]{64}', value['canonical_set_sha256'])
            or type(value['canonical_records']) is not int or value['canonical_records'] < 1
            or type(value['canonical_devices']) is not int or value['canonical_devices'] < 2
            or value['signatures_and_chains_verified'] is not True
            or any(value[field] is not False for field in ('remote_durability_verified',
                   'network_delivery_verified', 'derived_results_verified'))):
        raise Refused('invalid_local_receipt')
    return value


def compare(windows, mac, run_id):
    a = validate_receipt(windows, run_id, WINDOWS, MAC)
    b = validate_receipt(mac, run_id, MAC, WINDOWS)
    fields = ('canonical_set_sha256', 'canonical_records', 'canonical_devices')
    return dict(schema=1, purpose='cryptex-local-set-comparison', run_id=run_id,
                same_verified_canonical_set=all(a[key] == b[key] for key in fields),
                network_delivery_verified=False, interruption_verified=False,
                remote_protection_claim=False)


def read_plan(package):
    plan = read_metadata(package / 'verification.json')
    if (set(plan) != {'schema', 'purpose', 'run_id', 'source_commit', 'source_sha256', 'windows_daemon_sha256'}
            or type(plan['schema']) is not int or plan['schema'] != 1
            or plan['purpose'] != 'cryptex-local-verification-packet'
            or str(uuid.UUID(plan['run_id'])) != plan['run_id']
            or not re.fullmatch('[a-f0-9]{40}', plan['source_commit'])
            or any(not re.fullmatch('[a-f0-9]{64}', plan[key]) for key in
                   ('source_sha256', 'windows_daemon_sha256'))):
        raise Refused('invalid_verification_plan')
    return plan


def command(daemon, root, plan, local, peer):
    return [str(daemon), 'replication', 'verify-local', '--runtime-root', str(root),
            '--environment', 'dev', '--run-id', plan['run_id'], '--brain-id', BRAIN,
            '--local-device-id', local, '--peer-device-id', peer]


def parse_output(output, plan, local, peer):
    if isinstance(output, bytes):
        output = output.decode('utf-8')
    if not isinstance(output, str) or len(output) > 16384:
        raise Refused('daemon_output_bound')
    from mac_identity_preflight import unique_object
    lines = output.splitlines()
    if not lines:
        raise Refused('missing_local_receipt')
    # The signed Mac development daemon emits fixed key-load stages on stderr;
    # the bounded runner combines streams. Accept only those exact successful
    # stages before a single final receipt, never arbitrary logs or failed stages.
    for line in lines[:-1]:
        stage = json.loads(line, object_pairs_hook=unique_object)
        if (not isinstance(stage, dict) or set(stage) != {'m8ProfileStage', 'phase'}
                or stage['m8ProfileStage'] not in ('key_lock', 'key_store')
                or stage['phase'] not in ('begin', 'complete')):
            raise Refused('unexpected_daemon_output')
    return validate_receipt(json.loads(lines[-1], object_pairs_hook=unique_object),
                            plan['run_id'], local, peer)


def collect(package, plan, system, report):
    if system == 'Windows':
        import ctypes
        from windows_setup_native import Pins, run
        if ctypes.windll.shell32.IsUserAnAdmin():
            raise Refused('standard_user_required')
        root = Path(os.environ['LOCALAPPDATA']) / 'Cryptex-Dev/runtime/p0-2'
        daemon = package / 'payload/cryptexd.exe'
        with Pins() as pins:
            pins.chain(root)
            pins.chain(daemon.parent)
            pins.pin(daemon, False)
            with daemon.open('rb') as stream:
                raw = stream.read(64 * 1024 * 1024 + 1)
            if len(raw) > 64 * 1024 * 1024 or hashlib.sha256(raw).hexdigest() != plan['windows_daemon_sha256']:
                raise Refused('daemon_hash_mismatch')
            report['daemon_sha256'] = plan['windows_daemon_sha256']
            output = run(command(daemon, root, plan, WINDOWS, MAC), 90)
        local, peer = WINDOWS, MAC
    elif system == 'Darwin':
        if os.geteuid() == 0 or int(platform.mac_ver()[0].split('.')[0]) < 26:
            raise Refused('standard_macos_26_user_required')
        from mac_existing_update import prepare
        from mac_identity_preflight import run_bounded
        root = Path.home() / 'Library/Application Support/Cryptex-Dev/runtime/p0-2'
        build_plan = {key: plan[key] for key in ('source_commit', 'source_sha256')}
        desktop = prepare(package, Path.home(), build_plan, report, create_launcher=False)
        daemon = desktop.parent / 'cryptexd'
        report['daemon_sha256'] = hashlib.sha256(daemon.read_bytes()).hexdigest()
        outcome = run_bounded(command(daemon, root, plan, MAC, WINDOWS), 90)
        if outcome.code or outcome.timed_out or outcome.limited or outcome.pipe_failed:
            raise Refused('daemon_verification_refused')
        output = outcome.output
        local, peer = MAC, WINDOWS
    else:
        raise Refused('unsupported_platform')
    return parse_output(output, plan, local, peer)


def main():
    os.umask(0o077)
    package = Path(__file__).resolve().parent
    system = platform.system()
    report = dict(schema=1, purpose='cryptex-canonical-verification-result', os=system,
                  outcome='refused', canonical_writes=False, credentials_exported=False,
                  network_delivery_verified=False, interruption_verified=False)
    try:
        plan = read_plan(package)
        report.update(run_id=plan['run_id'], source_commit=plan['source_commit'])
        print('Cryptex normal bicimde kapali olmali. Profil ve eslestirme korunur.\n'
              'Mac derlemesi ve Keychain izni gerekebilir; sifreyi yalnizca macOS penceresine yaz.', flush=True)
        receipt = collect(package, plan, system, report)
        # Build helper outcome is installation evidence, not this verification.
        report.update(outcome='local_canonical_verified', receipt=receipt)
    except Exception as error:
        code = str(error)
        report.update(outcome='refused', error=code if re.fullmatch('[a-z_]{1,100}', code) else 'verification_unavailable')
    output = package / ('BUTUNLUK-SONUC-' + system + '-' + uuid.uuid4().hex[:12] + '.zip')
    with zipfile.ZipFile(output, 'x', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json', json.dumps(report, indent=2) + '\n')
    print('\nGonderilecek sonuc: ' + str(output), flush=True)
    print('Bu rapor tek basina ag aktarimi, kesinti testi veya M8 tamamlandi demek degildir.', flush=True)
    return 0 if report['outcome'] == 'local_canonical_verified' else 2


if __name__ == '__main__':
    raise SystemExit(main())
