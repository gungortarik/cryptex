#!/usr/bin/python3
"""Read-only libproc diagnostic; never launches Cryptex or opens a Keychain."""
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import tempfile
import uuid
import zipfile

from mac_identity_preflight import run_bounded, PreflightError, unique_object


def summary(result):
    report = {'exit_code': result.code, 'timed_out': result.timed_out,
              'output_limited': result.limited, 'pipe_failed': result.pipe_failed}
    if result.code == 2 and not (result.timed_out or result.limited or result.pipe_failed):
        match = re.fullmatch(r'inventory_error (arguments|list|info|uid|path|recheck|changed) ([0-9]{1,3}) (-?[0-9]{1,7})\n', result.output)
        if match and int(match[2]) <= 255:
            report.update(stage=match[1], os_error=int(match[2]), returned_size=int(match[3]))
        else:
            report['diagnostic'] = 'unrecognized_probe_failure'
    elif result.code == 0 and not (result.timed_out or result.limited or result.pipe_failed):
        # Don't export per-process rows, including PID/start/parent/group.
        rows = result.output.splitlines()
        if 0 < len(rows) <= 4096 and all(re.fullmatch(r'[0-9]+ [0-9]+ [0-9]+ [0-9]+ [0-9]+ [0-3]', row) for row in rows):
            report['observed_live_process_count'] = len(rows)
        else:
            report['diagnostic'] = 'unrecognized_probe_output'
    return report


def collect(package, runner=run_bounded):
    if sys.platform != 'darwin':
        raise PreflightError('macos_required')
    with tempfile.TemporaryDirectory(prefix='cryptex-inventory-diagnostic-') as scratch:
        root = Path(scratch).resolve()
        probe = root / 'probe'
        result = runner(['/usr/bin/xcrun', 'clang', '-Wall', '-Wextra', '-Werror',
                         package / 'mac_process_inventory.c', '-o', probe], 60)
        report = {'compile': {k: v for k, v in summary(result).items() if k in (
            'exit_code', 'timed_out', 'output_limited', 'pipe_failed')}, 'samples': []}
        if result.code or result.timed_out or result.limited or result.pipe_failed:
            report['outcome'] = 'probe_compile_failed'
            return report
        # Nonexistent paths in our own fresh scratch directory: no app needed.
        args = [probe, root / 'unused-desktop', root / 'unused-daemon', root / 'unused-engine']
        for _ in range(3):
            report['samples'].append(summary(runner(args, 5)))
        report['outcome'] = 'diagnostic_collected'
        return report


def main():
    os.umask(0o077)
    package = Path(__file__).resolve().parent
    report = {'schema': 1, 'scope': 'read-only libproc diagnosis; no application/profile/Keychain operations',
              'outcome': 'incomplete'}
    print('Kısa süreç sorgusu tanısı başladı. Cryptex açılmaz; anahtarlara ve profillere dokunulmaz.', flush=True)
    try:
        meta = json.loads((package / 'diagnostic.json').read_text(encoding='utf-8'), object_pairs_hook=unique_object)
        expected = {'MAC-KAPANIS-TANI.command', 'mac_process_inventory.c', 'mac_identity_preflight.py'}
        if set(meta['files']) != expected or not re.fullmatch('[0-9a-f]{40}', meta['source']):
            raise PreflightError('package_metadata_invalid')
        for name, digest in meta['files'].items():
            if hashlib.sha256((package / name).read_bytes()).hexdigest() != digest:
                raise PreflightError('package_changed')
        report['source'] = meta['source']
        report.update(collect(package))
    except PreflightError as error:
        report.update(outcome='failed', diagnostic=str(error))
    except (Exception, KeyboardInterrupt):
        report['outcome'] = 'collector_incomplete'
    output = package / ('MAC-KAPANIS-TANI-SONUC-' + uuid.uuid4().hex[:8] + '.zip')
    with zipfile.ZipFile(output, 'x', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json', json.dumps(report, indent=2)+'\n')
    print('Gönderilecek dosya: ' + str(output), flush=True)
    return 0 if report['outcome'] == 'diagnostic_collected' else 1


if __name__ == '__main__':
    sys.exit(main())
