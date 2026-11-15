"""Read-only keychain discovery comparison; never read items or change OS state."""
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile


def compare_contexts(original_home, temporary_home, runner=subprocess.run):
    env = os.environ.copy()
    env['PATH'] = '/usr/bin:/bin:/usr/sbin:/sbin'
    for key in list(env):
        if key.startswith('DYLD_'):
            env.pop(key)

    def read(home):
        values = []
        for operation in ('default-keychain', 'list-keychains'):
            result = runner(['/usr/bin/security', operation, '-d', 'user'],
                            env={**env, 'HOME': str(home)}, capture_output=True, timeout=5)
            # Keep paths in memory only. Never export stderr or keychain names.
            if len(result.stdout) > 65536 or len(result.stderr) > 65536:
                raise ValueError('discovery_output_limit')
            paths = tuple(shlex.split(result.stdout.decode('utf-8', 'strict'))) if result.returncode == 0 else ()
            values.append((result.returncode, paths))
        return values

    before, temporary, after = read(original_home), read(temporary_home), read(original_home)
    return {
        'scope': 'read-only keychain discovery; no secret access or product startup',
        'original_context_stable': before == after,
        'default_discovery_equal': before[0] == temporary[0],
        'search_list_discovery_equal': before[1] == temporary[1],
        'original_exit_codes': [value[0] for value in before],
        'temporary_exit_codes': [value[0] for value in temporary],
        'original_search_list_count': len(before[1][1]),
        'temporary_search_list_count': len(temporary[1][1]),
    }


def main():
    if sys.platform != 'darwin':
        raise SystemExit('macos_required')
    original = Path(os.environ['HOME'])
    with tempfile.TemporaryDirectory(prefix='cryptex-keychain-discovery-') as temporary:
        result = compare_contexts(original, Path(temporary))
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
