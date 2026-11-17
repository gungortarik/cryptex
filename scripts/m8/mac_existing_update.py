#!/usr/bin/python3
"""Build beside the old Mac app; never enroll, remove or rewrite its profile."""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shlex
import subprocess
import sys
import uuid
import zipfile

from lan_transfer_diagnostic import BRAIN, MAC, Refused, directory, read_metadata
from mac_identity_preflight import PreflightError, checked, run_bounded, unique_object
from mac_lan_setup import build, build_diagnostic, cms_diagnostic, extract_source, tool
from mac_live_service_diagnostic import processes
from mac_normal_profile import DefaultProfile


def existing_identity(root):
    brain = read_metadata(root / 'identity/brain_identity.json')
    device = read_metadata(root / 'identity/local_device.json')
    if (brain.get('brain_id') != BRAIN or device.get('brain_id') != BRAIN
            or device.get('device_id') != MAC):
        raise Refused('wrong_mac_profile')
    return brain, device


def require_closed(observer=processes):
    counts = observer()
    # Names are a conservative refusal gate, never permission to signal/adopt.
    if counts['desktop'] or counts['daemon']:
        raise Refused('quit_cryptex_before_update')


def prepare(package, home, plan, report, builder=build, observer=processes):
    root = home / 'Library/Application Support/Cryptex-Dev/runtime/p0-2'
    before = existing_identity(root)
    require_closed(observer)
    cache = DefaultProfile(home)
    cache.path = home / 'Library/Caches/Cryptex/existing-update' / uuid.uuid4().hex
    with directory(root) as root_fd:
        original = os.fstat(root_fd)
        try:
            work = cache.acquire()
            source = work / 'source'
            extract_source(package / 'source.zip', source, plan['source_sha256'])
            node, cargo, pnpm = tool('node'), tool('cargo'), tool('pnpm')
            tool_path = os.pathsep.join(dict.fromkeys([
                str(node.parent), str(cargo.parent), str(pnpm.parent),
                '/usr/bin', '/bin', '/usr/sbin', '/sbin']))

            def run(args, label, timeout=60, *, input_data=None):
                report['stage'] = label
                cache.check()
                print('\nCryptex: ' + label, flush=True)
                outcome = run_bounded([str(a) for a in args], timeout,
                                      input_data=input_data, tool_path=tool_path)
                if outcome.code or outcome.timed_out or outcome.limited or outcome.pipe_failed:
                    for key, diagnostic in (('build_failure', build_diagnostic(label, outcome)),
                                            ('cms_failure', cms_diagnostic(label, outcome))):
                        if diagnostic is not None:
                            report[key] = diagnostic
                return checked(lambda *_: outcome, args, label, timeout)

            # This function only builds/signs in a fresh cache. In particular,
            # mac_lan_setup.setup and its enrollment commands are never called.
            bundle, _unused_creator = builder(source, work, plan['source_commit'], run, report)
            cache.check()
            require_closed(observer)
            with directory(root) as current_fd:
                current = os.fstat(current_fd)
                if (current.st_dev, current.st_ino) != (original.st_dev, original.st_ino):
                    raise Refused('existing_profile_replaced')
            if existing_identity(root) != before:
                raise Refused('existing_identity_changed')
            desktop = bundle / 'Contents/MacOS/cryptex-desktop'
            launcher = package / ('CRYPTEX-AC-' + plan['source_commit'][:8] + '.command')
            with launcher.open('x', encoding='utf-8', newline='\n') as stream:
                stream.write('#!/bin/zsh\nset -eu\nexec ' + shlex.quote(str(desktop)) + '\n')
            launcher.chmod(0o700)
            report.update(outcome='built_beside_existing_app',
                          public_identity_unchanged=True,
                          release_inventory_sha256=hashlib.sha256(
                              (bundle / 'Contents/Resources/m8-startup.json').read_bytes()).hexdigest())
            return desktop
        finally:
            cache.close()


def main():
    os.umask(0o077)
    package = Path(__file__).resolve().parent
    report = dict(schema=1, purpose='cryptex-existing-mac-update', outcome='refused',
                  updater_profile_writes=False, enrollment_requested=False,
                  old_app_changed=False, network_delivery_verified=False)
    desktop = None
    try:
        if (sys.platform != 'darwin' or os.geteuid() == 0
                or int(platform.mac_ver()[0].split('.')[0]) < 26):
            raise Refused('standard_macos_26_user_required')
        plan = json.loads((package / 'update.json').read_text(), object_pairs_hook=unique_object)
        if (set(plan) != {'schema', 'purpose', 'source_commit', 'source_sha256'}
                or plan['schema'] != 1 or plan['purpose'] != report['purpose']
                or not re.fullmatch('[a-f0-9]{40}', plan['source_commit'])
                or not re.fullmatch('[a-f0-9]{64}', plan['source_sha256'])):
            raise Refused('invalid_update_plan')
        report['source_commit'] = plan['source_commit']
        print('Mevcut profil ve eşleştirme korunarak yeni uygulama hazırlanıyor.\n'
              'macOS izin isterse şifreni yalnızca macOS penceresine yaz.', flush=True)
        desktop = prepare(package, Path.home(), plan, report)
    except (Refused, PreflightError) as error:
        code = str(error)
        report['error'] = code if re.fullmatch('[a-z_]{1,100}', code) else 'update_refused'
    except (OSError, ValueError, KeyError, TypeError, zipfile.BadZipFile):
        report['error'] = 'update_unavailable'
    report['updater_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    output = package / ('MAC-GUNCELLEME-SONUC-' + uuid.uuid4().hex[:12] + '.zip')
    with zipfile.ZipFile(output, 'x', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('result.json', json.dumps(report, indent=2) + '\n')
    print('\nGönderilecek sonuç: ' + str(output), flush=True)
    if desktop is None:
        print('Güncelleme tamamlanamadı. Profili silme; sonucu gönder.', flush=True)
        return 2
    print('Yeni Cryptex açılıyor. Open local memory seç. Terminali açık bırak.\n'
          'Sonuç ZIP’i hazır; uygulama açıkken gönderebilirsin.', flush=True)
    # Normal startup owns all admission, Keychain and runtime writes. Wait for
    # natural user quit; never enforce a timeout or kill an application process.
    return subprocess.call([str(desktop)])


if __name__ == '__main__':
    raise SystemExit(main())
