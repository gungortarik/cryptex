"""Read-only VM transport failure observations; export fixed markers, never log lines."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import sys
import uuid
import zipfile

from windows_pairing_diagnostic import BRAIN, LOCAL, read_json, observation
from windows_setup_native import Pins, Refused, run

MARKERS = {
    'listener_started': ('listener starting', 'listener started'),
    'listener_stopped': ('listener shutting down', 'listener stopped'),
    'listen_failure': ('failed to listen', 'listen tcp', 'bind:'),
    'connection_established': ('established secure connection',),
    'connection_closed': ('connection closed', 'connection was closed'),
    'unexpected_device': ('unexpected device', 'unknown device'),
    'certificate_error': ('certificate verification', 'certificate is not valid', 'bad certificate'),
    'folder_error': ('failed initial scan', 'error on folder', 'folder marker missing'),
    'access_denied': ('access is denied', 'permission denied'),
    'database_stop_timeout': ('database failed to stop within 10s',),
    'shutdown_requested': ('shutting down', 'exiting'),
    'panic': ('panic:', 'fatal error:'),
}


def summarize(raw, truncated):
    text = raw.decode('utf-8', errors='replace')
    counts = {key: 0 for key in MARKERS}
    recent = []
    for line in text.splitlines():
        lower = line.lower()
        found = [key for key, patterns in MARKERS.items() if any(p in lower for p in patterns)]
        for key in found:
            counts[key] += 1
        if found:
            recent.append(found)
            recent = recent[-24:]
    return dict(tail_only=truncated, bytes_read=len(raw), marker_counts=counts,
                recent_marker_groups=recent, raw_lines_exported=False,
                limits='Markers can include earlier runs; absence does not rule out a failure.')


def read_log(path):
    import msvcrt
    import windows_setup_native as native
    with Pins() as pins:
        pins.chain(path.parent)
        # Existing log may still have a writer. Pin its opened identity, allow
        # shared writes, deny deletion, and reject reparse/hard-linked objects.
        handle = native.create_file(str(path), 0x80000000, 3, None, 3, 0x02200000, None)
        if handle == ctypes.c_void_p(-1).value:
            raise Refused('log_open_unavailable')
        try:
            info = native.INFO()
            if (not native.info_file(handle, ctypes.byref(info))
                    or info.attributes & (0x400 | 0x10) or info.links != 1):
                raise Refused('unsafe_log')
            fd = msvcrt.open_osfhandle(int(handle), os.O_RDONLY | os.O_BINARY)
            handle = None
            with os.fdopen(fd, 'rb') as stream:
                size = os.fstat(stream.fileno()).st_size
                start = max(0, size - 1024 * 1024)
                stream.seek(start)
                raw = stream.read(1024 * 1024)
            return summarize(raw, start > 0)
        finally:
            if handle is not None:
                native.close(handle)


def processes(runner=run):
    shell = Path(os.environ['SYSTEMROOT']) / 'System32/WindowsPowerShell/v1.0/powershell.exe'
    command = r'''$ErrorActionPreference='Stop'
$counts=@{}
foreach($n in @('cryptex-desktop','cryptexd','syncthing')) {
 $counts[$n]=@(Get-Process -Name $n -ErrorAction SilentlyContinue).Count
}
$rows=@(Get-NetTCPConnection -State Listen -LocalPort 59957 -ErrorAction SilentlyContinue)
$lan=@($rows | Where-Object { $_.LocalAddress -in @('192.168.50.26','0.0.0.0','::') }).Count
$loop=@($rows | Where-Object { $_.LocalAddress -in @('127.0.0.1','::1') }).Count
$addresses=@(Get-NetIPAddress -AddressFamily IPv4 -ErrorAction Stop)
$configured=@($addresses | Where-Object { $_.IPAddress -eq '192.168.50.26' -and $_.AddressState -eq 'Preferred' }).Count
[pscustomobject]@{desktop=$counts['cryptex-desktop'];daemon=$counts['cryptexd'];engine=$counts['syncthing'];listeners=$rows.Count;lan=$lan;loopback=$loop;expected_vm_address=$configured} | ConvertTo-Json -Compress'''
    value = json.loads(runner([shell, '-NoProfile', '-NonInteractive', '-Command', command], 20))
    if (set(value) != {'desktop', 'daemon', 'engine', 'listeners', 'lan', 'loopback', 'expected_vm_address'}
            or any(type(v) is not int or not 0 <= v <= 4096 for v in value.values())):
        raise Refused('process_observation_shape')
    return value


def first_failure(path):
    value = read_json(path)
    # Works both with loose source files and the single-file Windows zipapp.
    labels = json.loads(__loader__.get_data(str(Path(__file__).with_name('worker_failure_codes.json'))))
    if (set(value) != {'schema', 'code', 'os_code'} or type(value['schema']) is not int
            or value['schema'] != 1 or type(value['code']) is not int
            or str(value['code']) not in labels
            or (value['os_code'] is not None and
                (type(value['os_code']) is not int or not -(2**63) <= value['os_code'] < 2**63))):
        raise Refused('first_failure_shape')
    return dict(schema=1, code=value['code'], error=labels[str(value['code'])], os_code=value['os_code'],
                scope='First persisted worker failure since diagnostic candidate; may precede current process.')


def collect(root, reader=read_log, process_reader=processes):
    brain = read_json(root / 'identity/brain_identity.json')
    local = read_json(root / 'identity/local_device.json')
    if brain.get('brain_id') != BRAIN or local.get('brain_id') != BRAIN or local.get('device_id') != LOCAL:
        raise Refused('wrong_vm_profile')
    return dict(schema=1, purpose='cryptex-vm-transport-failure-diagnostic', outcome='observed',
                profile_writes=False, processes_changed=False, credentials_read=False,
                canonical_bodies_read=False, atomic_snapshot=False,
                native_log=observation(lambda: reader(root / 'managed-transport' /
                    '00000000000000000000000000000001/native/native.log')),
                process_counts_not_identity_proof=observation(process_reader),
                first_worker_failure=observation(lambda: first_failure(root / "managed-transport" /
                    "00000000000000000000000000000001/worker-first-failure-v1.json")))


def summary(report):
    if report['outcome'] != 'observed':
        return 'TANI ' + report.get('error', 'unavailable')
    stored = report['first_worker_failure']
    worker = stored.get('value', {})
    counts = report['process_counts_not_identity_proof'].get('value', {})
    markers = report['native_log'].get('value', {}).get('marker_counts', {})
    return ('TANI worker=' + str(worker.get('error', stored.get('error', 'unavailable')))
            + ' os=' + str(worker.get('os_code')) + ' IP=' + str(counts.get('expected_vm_address', '?'))
            + ' listen=' + str(counts.get('lan', '?')) + ' bind_hatasi=' + str(markers.get('listen_failure', '?')))


def main():
    report = dict(schema=1, purpose='cryptex-vm-transport-failure-diagnostic', outcome='refused')
    try:
        if os.name != 'nt' or ctypes.windll.shell32.IsUserAnAdmin():
            raise Refused('standard_windows_user_required')
        report = collect(Path(os.environ['LOCALAPPDATA']) / 'Cryptex-Dev/runtime/p0-2')
    except Refused as error:
        report['error'] = str(error)
    except Exception:
        report['error'] = 'collector_unavailable'
    report['collector_sha256'] = hashlib.sha256(__loader__.get_data(__file__)).hexdigest()
    output = Path(sys.argv[0]).absolute().parent / ('VM-SUREC-TANI-SONUC-' + uuid.uuid4().hex[:12] + '.zip')
    with zipfile.ZipFile(output, 'x', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json', json.dumps(report, indent=2) + '\n')
    print('Profil ve surecler degistirilmedi. Gonderilecek sonuc: ' + str(output))
    print(summary(report), flush=True)


if __name__ == '__main__':
    main()
