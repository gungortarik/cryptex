#!/usr/bin/python3
"""Owner-local persistent LAN preparation. No peer trust or network acceptance."""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import shlex
import ssl
import stat
import sys
import uuid
import zipfile

sys.dont_write_bytecode = True
from mac_identity_preflight import PreflightError, checked, run_bounded, unique_object
from mac_normal_profile import DefaultProfile
from mac_transport_package import prepare as prepare_transport
from mac_xpc_probe import select_identity


def public_certificate(pem_inventory, fingerprint):
    if len(pem_inventory) > 1024 * 1024 or not re.fullmatch(r'[A-F0-9]{40}', fingerprint):
        raise PreflightError('owner_certificate_inventory_invalid')
    candidates = set()
    for pem in re.findall(r'-----BEGIN CERTIFICATE-----\s+.*?-----END CERTIFICATE-----', pem_inventory, re.S):
        try:
            der = ssl.PEM_cert_to_DER_cert(pem)
        except (ValueError, ssl.SSLError):
            raise PreflightError('owner_certificate_invalid') from None
        if hashlib.sha1(der).hexdigest().upper() == fingerprint:
            candidates.add(der)
    if len(candidates) != 1:
        raise PreflightError('owner_certificate_selection_mismatch')
    return candidates.pop()


def source_members(archive):
    members = archive.infolist()
    if len(members) > 16000 or sum(i.file_size for i in members) > 128 * 1024 * 1024:
        raise PreflightError('source_archive_bound')
    seen = set()
    for item in members:
        name = item.filename
        path = PurePosixPath(name)
        if (item.orig_filename != name or path.is_absolute() or '..' in path.parts or '\\' in name or ':' in name
                or name.rstrip('/') != path.as_posix()
                or not path.parts or any(p.startswith('.') and p in ('.', '..') for p in name.split('/'))
                or name.casefold() in seen or item.flag_bits & 1
                or stat.S_IFMT(item.external_attr >> 16) not in (0, stat.S_IFREG, stat.S_IFDIR)):
            raise PreflightError('source_archive_member')
        seen.add(name.casefold())
    return members


def extract_source(archive_path, destination, expected):
    if archive_path.stat().st_size > 128 * 1024 * 1024:
        raise PreflightError('source_archive_bound')
    if hashlib.sha256(archive_path.read_bytes()).hexdigest() != expected:
        raise PreflightError('source_archive_changed')
    with zipfile.ZipFile(archive_path) as archive:
        members = source_members(archive)  # Complete validation before any writes.
        destination.mkdir(mode=0o700)
        for item in members:
            target = destination / item.filename
            if item.is_dir():
                target.mkdir(mode=0o700, parents=True, exist_ok=True)
            else:
                target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                with target.open('xb') as stream:
                    stream.write(archive.read(item))
                target.chmod(0o700 if item.external_attr >> 16 & 0o111 else 0o600)


def tool(name):
    search = os.pathsep.join([os.environ.get('PATH', ''), str(Path.home()/'.cargo/bin'),
                              '/opt/homebrew/bin', '/usr/local/bin', '/usr/bin', '/bin'])
    found = shutil.which(name, path=search)
    if not found:
        raise PreflightError('build_tool_missing_' + name)
    # Preserve rustup proxy's argv[0] rather than resolving it to `rustup`.
    return Path(found).absolute()


BUILD_LABELS = {'node_version', 'pnpm_version', 'frontend_dependencies', 'frontend_build'}
BUILD_CODES = {'ERR_PNPM_UNSUPPORTED_ENGINE', 'ERR_PNPM_BAD_PM_VERSION',
               'ERR_PNPM_OUTDATED_LOCKFILE', 'ERR_PNPM_LOCKFILE_CONFIG_MISMATCH',
               'ERR_PNPM_FETCH_401', 'ERR_PNPM_FETCH_403', 'ERR_PNPM_FETCH_404',
               'ERR_PNPM_META_FETCH_FAIL', 'ENOTFOUND', 'ECONNRESET', 'ETIMEDOUT',
               'EACCES', 'ENOSPC', 'ERR_MODULE_NOT_FOUND', 'MODULE_NOT_FOUND'}


def build_diagnostic(label, outcome):
    if label not in BUILD_LABELS:
        return None
    # Closed tokens only: never return raw stderr, URLs, paths or registry credentials.
    tokens = set(re.findall(r'\b[A-Z][A-Z0-9_]+\b', outcome.output))
    return dict(stage=label, exit_code=outcome.code, timed_out=outcome.timed_out,
                output_limited=outcome.limited, pipe_failed=outcome.pipe_failed,
                codes=sorted(tokens & BUILD_CODES))


def build_frontend(source, node, pnpm, run, report):
    expected = json.loads((source/'package.json').read_text())['engines']
    versions = report.setdefault('build_tools', {})
    for name, executable in (('node', node), ('pnpm', pnpm)):
        # pnpm may be a shell/Corepack launcher or a native executable. Let the
        # OS execute it; passing its bytes to Node as JavaScript is invalid.
        version = run([executable, '--version'], name + '_version', 60).strip()
        if not re.fullmatch(r'v?\d{1,4}\.\d{1,4}\.\d{1,4}', version):
            raise PreflightError(name + '_version_unrecognized')
        versions[name] = version.removeprefix('v')
        if versions[name] != expected[name]:
            raise PreflightError(name + '_version_mismatch')
    run([pnpm, 'install', '--frozen-lockfile'], 'frontend_dependencies', 900)
    run([pnpm, '--filter', '@cryptex/desktop', 'build'], 'frontend_build', 600)


def inventory(bundle, architecture, release, run):
    files, roles = [], {}
    names = ['Contents/MacOS/cryptex-desktop', 'Contents/MacOS/cryptexd']
    names += [p.relative_to(bundle).as_posix() for p in sorted((bundle/'Contents/Resources').rglob('*')) if p.is_file()]
    for name in names:
        path = bundle/name
        info = path.lstat()
        if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
            raise PreflightError('owner_inventory_unsafe')
        raw = path.read_bytes()
        files.append({'path': name, 'size': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()})
    for name, role in (('cryptex-desktop', 'desktop'), ('cryptexd', 'daemon')):
        relative = 'Contents/MacOS/' + name
        output = run(['/usr/bin/codesign', '-d', '--verbose=4', bundle/relative], 'image_hash', 30)
        hashes = re.findall(r'^CDHash=([a-f0-9]{40})$', output, re.M)
        if len(hashes) != 1:
            raise PreflightError('owner_image_hash_missing')
        roles[role] = {'path': relative, 'cdhash': hashes[0]}
    return {'schema': 1, 'purpose': 'cryptex-m8-release-inventory', 'release_id': release,
            'architecture': architecture, **roles, 'files': files}


def build(source, work, commit, run, report):
    fingerprint = select_identity(run(['/usr/bin/security', 'find-identity', '-v', '-p', 'codesigning'], 'select_owner_identity', 30))
    der = public_certificate(run(['/usr/bin/security', 'find-certificate', '-a', '-p'], 'read_public_certificates', 30), fingerprint)
    certificate = work/'owner-public.der'
    certificate.write_bytes(der)
    report['signer_certificate_sha256'] = hashlib.sha256(der).hexdigest()
    cargo, node, pnpm = tool('cargo'), tool('node'), tool('pnpm')
    release = 'cryptex-lan-' + commit[:12]
    previous = Path.cwd()
    saved = {k: os.environ.get(k) for k in ('CRYPTEX_M8_DEVELOPMENT_CERT_DER', 'CRYPTEX_M8_DEVELOPMENT_RELEASE_ID',
             'CARGO_PROFILE_DEV_DEBUG', 'CARGO_TARGET_DIR')}
    try:
        os.chdir(source)
        os.environ.update(CRYPTEX_M8_DEVELOPMENT_CERT_DER=str(certificate),
                          CRYPTEX_M8_DEVELOPMENT_RELEASE_ID=release, CARGO_PROFILE_DEV_DEBUG='0',
                          CARGO_TARGET_DIR=str(work/'target'))
        build_frontend(source, node, pnpm, run, report)
        # Distinct explicit public-metadata creator. It never owns OS keys.
        run([cargo, 'build', '--quiet', '--locked', '-p', 'cryptexd'], 'public_creator_build', 1800)
        creator = work/'public-profile-creator'
        shutil.copyfile(work/'target/debug/cryptexd', creator)
        creator.chmod(0o700)
        from mac_local_startup_ci import fixture_rust_command
        run(fixture_rust_command(str(cargo), 'build', '--quiet', '--locked', '-p', 'cryptexd', '-p', 'cryptex-desktop',
            '--features', 'cryptex-desktop/m8-development-product'), 'product_build', 2400)
    finally:
        os.chdir(previous)
        for key, value in saved.items():
            if value is None: os.environ.pop(key, None)
            else: os.environ[key] = value
    bundle = work/'Cryptex-LAN.app'
    binary_dir, resources = bundle/'Contents/MacOS', bundle/'Contents/Resources'
    binary_dir.mkdir(mode=0o700, parents=True)
    resources.mkdir(mode=0o700)
    shutil.copytree(source/'apps/desktop/dist', resources/'frontend')
    prepare_transport(resources/'transport', run)
    for name, role in (('cryptex-desktop', 'desktop'), ('cryptexd', 'daemon')):
        target = binary_dir/name
        shutil.copyfile(work/'target/debug'/name, target)
        target.chmod(0o700)
        run(['/usr/bin/codesign', '--force', '--timestamp=none', '--sign', fingerprint,
             '--identifier', 'com.cryptex.development.' + role, target], 'sign_product_' + role, 120)
        run(['/usr/bin/codesign', '--verify', '--strict', '-R', '=certificate leaf = H"' + fingerprint + '"', target],
            'verify_product_' + role, 30)
    cms = work/'owner-cms'
    run(['/usr/bin/clang', '-Wall', '-Wextra', '-Werror', source/'scripts/m8/mac_owner_cms.c',
         source/'services/cryptexd/src/m8_owner_channel/native/inventory.c',
         '-framework', 'Security', '-framework', 'CoreFoundation', '-o', cms], 'compile_owner_cms', 60)
    run(['/usr/bin/codesign', '--force', '--timestamp=none', '--sign', fingerprint,
         '--identifier', 'com.cryptex.development.inventory-signer', cms], 'sign_inventory_helper', 120)
    manifest = resources/'m8-startup.json'
    manifest.write_text(json.dumps(inventory(bundle, platform.machine(), release, run), separators=(',', ':')), encoding='utf-8')
    run([cms, report['signer_certificate_sha256'], manifest, resources/'m8-startup.cms'], 'sign_inventory_keychain', 120)
    report['product_built_and_signed'] = True
    return bundle, creator


def setup(bundle, creator, home, plan, run, ask, report):
    note = 'Cryptex LAN acceptance ' + plan['run_id'] + ': Mac first public note.'
    if not ask('Yeni test profili kalıcı tutulacak. İlk public not:\n' + note + '\nKurulum başlasın mı?'):
        raise PreflightError('owner_declined_setup')
    profile = DefaultProfile(home)
    try:
        root = profile.acquire()  # Recheck absence at mutation time; no adoption.
        report['fresh_profile_created'] = True
        (root/'pilot-runtime.marker').write_bytes(b'm0-native-desktop-runtime-v1\n')
        run([creator, 'pilot-fixture', 'lan-profile-v1', '--runtime-root', root, '--brain-id', plan['brain_id'],
             '--confirm-isolated-pilot'], 'public_brain_create', 45)
        profile.check()
        daemon = bundle/'Contents/MacOS/cryptexd'
        run([daemon, 'identity', 'enroll-local', '--runtime-root', root, '--environment', 'dev'], 'product_key_enrollment', 120)
        profile.check()
        output = run([daemon, 'identity', 'first-note-local-dev', '--runtime-root', root,
                      '--confirm-public-first-note-stdin'], 'first_note_capture', 60, input_data=note.encode())
        if 'first_note=committed_public' not in output.splitlines():
            raise PreflightError('first_note_receipt_missing')
        report['first_note_committed'] = True
        report['first_note_sha256'] = hashlib.sha256(note.encode()).hexdigest()
        profile.check()
        output = run([daemon, 'transport', 'enroll-local-dev', '--runtime-root', root, '--confirm-local-only'],
                     'product_transport_enrollment', 180)
        if 'transport_enrollment=local_only' not in output.splitlines():
            raise PreflightError('transport_receipt_missing')
        report['transport_enrolled_local_only'] = True
        desktop = bundle/'Contents/MacOS/cryptex-desktop'
        for _ in range(2):
            profile.check()
            output = run([desktop, '--cryptex-development-check', root], 'initialized_startup_restart', 180)
            if 'development_existing_profile_startup_and_cleanup=passed' not in output.splitlines():
                raise PreflightError('initialized_startup_missing')
        report['initialized_startup_restart'] = True
        # Keep the actual enrolled app/keys/profile for pairing. No fixture cleanup.
        report['persistent_profile_preserved'] = True
    finally:
        profile.close()


def main():
    package = Path(__file__).resolve().parent
    report = {'schema': 1, 'purpose': 'cryptex-persistent-lan-setup', 'platform': platform.system(),
              'pairing_authorized': False, 'network_delivery_verified': False, 'convergence_verified': False}
    stage = 'validate_package'
    work = None
    try:
        if sys.platform != 'darwin' or platform.machine() not in ('arm64', 'x86_64') or int(platform.mac_ver()[0].split('.')[0]) < 26:
            raise PreflightError('macos_26_required')
        plan = json.loads((package/'setup.json').read_text(), object_pairs_hook=unique_object)
        if (set(plan) != {'schema', 'purpose', 'run_id', 'brain_id', 'source_commit', 'source_sha256'}
                or plan['schema'] != 1 or plan['purpose'] != 'cryptex-persistent-lan-setup'
                or str(uuid.UUID(plan['run_id'])) != plan['run_id'] or str(uuid.UUID(plan['brain_id'])) != plan['brain_id']
                or not re.fullmatch('[a-f0-9]{40}', plan['source_commit']) or not re.fullmatch('[a-f0-9]{64}', plan['source_sha256'])):
            raise PreflightError('setup_plan_invalid')
        report.update(run_id=plan['run_id'], source_commit=plan['source_commit'])
        home = Path.home()
        normal = home/'Library/Application Support/Cryptex-Dev/runtime/p0-2'
        if normal.exists() or normal.is_symlink():
            raise PreflightError('normal_profile_already_exists')
        # Validate/create only a private cache namespace using the same held
        # descriptor walker as profile setup; no default/legacy profile writes.
        cache = DefaultProfile(home)
        cache.path = home/'Library/Caches/Cryptex/lan-setup'/uuid.uuid4().hex
        work = cache.acquire()
        source = work/'source'
        extract_source(package/'source.zip', source, plan['source_sha256'])
        node, cargo, pnpm = tool('node'), tool('cargo'), tool('pnpm')
        tool_path = os.pathsep.join(dict.fromkeys([str(node.parent), str(cargo.parent), str(pnpm.parent), '/usr/bin', '/bin', '/usr/sbin', '/sbin']))
        def run(args, label, timeout=60, *, input_data=None):
            nonlocal stage
            stage = label
            cache.check()
            print('\nCryptex: ' + label, flush=True)
            outcome = run_bounded([str(a) for a in args], timeout, input_data=input_data, tool_path=tool_path)
            if outcome.code or outcome.timed_out or outcome.limited or outcome.pipe_failed:
                diagnostic = build_diagnostic(label, outcome)
                if diagnostic is not None:
                    report['build_failure'] = diagnostic
            return checked(lambda *_: outcome, args, label, timeout)
        bundle, creator = build(source, work, plan['source_commit'], run, report)
        from mac_window_owner import answer
        setup(bundle, creator, home, plan, run, answer, report)
        launcher = package/'CRYPTEX-AC.command'
        with launcher.open('x', encoding='utf-8', newline='\n') as stream:
            stream.write('#!/bin/zsh\nset -eu\nexec ' + shlex.quote(str(bundle/'Contents/MacOS/cryptex-desktop')) + '\n')
        launcher.chmod(0o700)
        report['release_inventory_sha256'] = hashlib.sha256((bundle/'Contents/Resources/m8-startup.json').read_bytes()).hexdigest()
        report['outcome'] = 'prepared_local_only'
        print('\nKurulum ve yeniden açılış kontrolü tamamlandı. Eşleştirme henüz yapılmadı.', flush=True)
        print('Sonraki açılış: bu klasördeki CRYPTEX-AC.command', flush=True)
    except (PreflightError, OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        report.update(outcome='refused_or_incomplete', failed_stage=stage)
        if isinstance(error, PreflightError) and re.fullmatch('[a-z_]{1,100}', str(error)):
            report['error'] = str(error)
        print('\nBu adım tamamlanamadı: ' + stage + '. Mevcut dosyalar korundu.', flush=True)
    finally:
        if 'cache' in locals(): cache.close()
        output = Path.home()/'Downloads'/('ESITLEME-KURULUM-SONUC-Darwin-' + uuid.uuid4().hex[:12] + '.zip')
        with zipfile.ZipFile(output, 'x', zipfile.ZIP_DEFLATED) as archive:
            archive.writestr('result.json', json.dumps(report, indent=2) + '\n')
        print('\nGönderilecek sonuç: ' + str(output), flush=True)
    return 0 if report.get('outcome') == 'prepared_local_only' else 2


if __name__ == '__main__':
    raise SystemExit(main())
