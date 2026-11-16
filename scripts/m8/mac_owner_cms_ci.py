"""Actual CMS signing/self-verification in a disposable hosted CI Keychain.
Only the exact test helper can access this synthetic private key. No owner keys.
"""
import hashlib
import ctypes
from pathlib import Path
import re
import sys
import tempfile

from mac_ci_credential_keychain import CredentialKeychain, require_hosted_mac, image_hash
from mac_identity_preflight import PreflightError, run_bounded, checked


def main():
    require_hosted_mac()
    repository = Path(__file__).resolve().parents[2]
    def run(args, stage, timeout=60):
        outcome = run_bounded([str(a) for a in args], timeout)
        return checked(lambda *_: outcome, [], stage, timeout)
    with tempfile.TemporaryDirectory(prefix='cx-owner-cms-ci-') as scratch:
        root = Path(scratch).resolve(strict=True)
        helper = root/'owner-cms'
        run(['/usr/bin/clang', '-Wall', '-Wextra', '-Werror', repository/'scripts/m8/mac_owner_cms.c',
             repository/'services/cryptexd/src/m8_owner_channel/native/inventory.c',
             '-framework', 'Security', '-framework', 'CoreFoundation', '-o', helper], 'compile_cms_helper')
        verifier = root/'inventory.dylib'
        run(['/usr/bin/clang', '-Wall', '-Wextra', '-Werror', '-dynamiclib',
             repository/'services/cryptexd/src/m8_owner_channel/native/inventory.c',
             '-framework', 'Security', '-framework', 'CoreFoundation', '-o', verifier], 'compile_unchanged_verifier')
        library = ctypes.CDLL(str(verifier))
        verify = library.cx_verify_inventory
        verify.argtypes = [ctypes.c_void_p, ctypes.c_size_t] * 3
        verify.restype = ctypes.c_int
        def verify_bytes(cms, content, cert):
            return verify(cms, len(cms), content, len(content), cert, len(cert))
        run(['/usr/bin/codesign', '--force', '--sign', '-', '--identifier', 'com.cryptex.ci.cms-helper', helper],
            'sign_cms_test_helper')
        helper_hash = image_hash(run(['/usr/bin/codesign', '-d', '--verbose=4', helper], 'inspect_test_helper'))
        config = root/'certificate.cnf'
        config.write_text('[req]\ndistinguished_name=dn\n[dn]\n[code_signing]\n'
                          'basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\n'
                          'extendedKeyUsage=codeSigning\n')
        run(['/usr/bin/openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1', '-sha256',
             '-subj', '/CN=Cryptex temporary CMS signer', '-config', config, '-extensions', 'code_signing',
             '-keyout', root/'key.pem', '-out', root/'cert.pem'], 'generate_test_identity')
        run(['/usr/bin/openssl', 'x509', '-in', root/'cert.pem', '-outform', 'DER', '-out', root/'cert.der'],
            'encode_test_certificate')
        digest = hashlib.sha256((root/'cert.der').read_bytes()).hexdigest()
        with CredentialKeychain(root, run_bounded) as scope:
            scope.run(['/usr/bin/openssl', 'pkcs12', '-export', '-in', root/'cert.pem', '-inkey', root/'key.pem',
                       '-out', root/'test.p12', '-passout', 'pass:'+scope.password,
                       '-keypbe', 'PBE-SHA1-3DES', '-certpbe', 'PBE-SHA1-3DES', '-macalg', 'sha1'], 'package_test_identity')
            scope.run(['/usr/bin/security', 'import', root/'test.p12', '-k', scope.path,
                       '-P', scope.password, '-T', helper], 'import_exact_helper_identity')
            scope.run(['/usr/bin/security', 'set-key-partition-list', '-S', 'cdhash:'+helper_hash,
                       '-s', '-k', scope.password, scope.path], 'authorize_exact_helper')
            manifest = root/'inventory.json'
            manifest.write_bytes(b'{"test":"synthetic-public-CMS-content"}\n')
            output = root/'inventory.cms'
            outcome = scope.fence.outcome([helper, digest, manifest, output], 60)
            # Diagnostic has only a closed stage token and integer status codes.
            for line in outcome.output.splitlines():
                if re.fullmatch(r'owner_cms_signing_refused stage=[a-z_]+ status=-?\d+ verify=\d+', line):
                    print(line, flush=True)
            checked(lambda *_: outcome, [], 'actual_cms_sign_and_verify', 60)
            if not output.is_file() or not 0 < output.stat().st_size <= 2097152:
                raise PreflightError('cms_output_missing')
            before = output.read_bytes()
            cert = (root/'cert.der').read_bytes()
            content = manifest.read_bytes()
            if verify_bytes(before, content, cert) != 0:
                raise PreflightError('cms_independent_self_verify_failed')
            damaged = before[:-1] + bytes([before[-1] ^ 1])
            for cms_bytes, content_bytes, cert_bytes in (
                    (before, content+b'changed', cert), (damaged, content, cert),
                    (before+b'\0', content, cert), (before, content, cert[:-1]+bytes([cert[-1] ^ 1]))):
                if verify_bytes(cms_bytes, content_bytes, cert_bytes) == 0:
                    raise PreflightError('cms_modified_input_not_refused')
            replay = scope.fence.outcome([helper, digest, manifest, output], 60)
            if replay.code != 2 or replay.timed_out or replay.limited or replay.pipe_failed or output.read_bytes() != before:
                raise PreflightError('cms_existing_output_not_preserved')
            wrong = scope.fence.outcome([helper, '0'*64, manifest, root/'wrong.cms'], 30)
            if wrong.code != 2 or wrong.timed_out or wrong.limited or wrong.pipe_failed or (root/'wrong.cms').exists():
                raise PreflightError('cms_wrong_certificate_not_refused')
            manifest.write_bytes(b'x' * 1048576)
            large = root/'large.cms'
            scope.run([helper, digest, manifest, large], 'cms_maximum_content', 60)
            if verify_bytes(large.read_bytes(), manifest.read_bytes(), cert) != 0:
                raise PreflightError('cms_maximum_content_not_verified')
        print('actual_keychain_cms_self_verify_and_refusals=passed', flush=True)


if __name__ == '__main__':
    try:
        main()
    except PreflightError as error:
        print(str(error), file=sys.stderr)
        sys.exit(2)
    except Exception:
        print('cms_ci_internal_failure', file=sys.stderr)
        sys.exit(2)
