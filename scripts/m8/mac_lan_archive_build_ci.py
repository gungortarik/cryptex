"""Compile the exact owner packet source path on hosted Mac CI; no Keychain.

The temporary certificate is only a build input for this disposable CI check.
It is never exported as an owner publisher or delivered in a setup packet.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

from mac_identity_preflight import checked, run_bounded
from mac_lan_setup import compile_products, extract_source, product_release


def main():
    if (sys.platform != 'darwin' or os.environ.get('GITHUB_ACTIONS') != 'true'
            or os.environ.get('RUNNER_ENVIRONMENT') != 'github-hosted'):
        raise RuntimeError('hosted_mac_ci_required')
    repository = Path(__file__).resolve().parents[2]
    commit = subprocess.check_output(['git', '-C', str(repository), 'rev-parse', 'HEAD'], text=True).strip()
    def run(args, stage, timeout=60):
        print('archive_build_stage=' + stage, flush=True)
        outcome = run_bounded([str(arg) for arg in args], timeout, tool_path=os.environ['PATH'])
        return checked(lambda *_: outcome, [], stage, timeout)
    with tempfile.TemporaryDirectory(prefix='cx-lan-archive-ci-') as scratch:
        work = Path(scratch).resolve(strict=True)
        archive = work/'source.zip'
        subprocess.run(['git', '-C', str(repository), 'archive', '--format=zip', '--output='+str(archive), commit], check=True)
        source = work/'source'
        extract_source(archive, source, hashlib.sha256(archive.read_bytes()).hexdigest())
        # Execute the unchanged authoritative Rust selector against the packet's
        # generated ID, and prove the previous packet ID is rejected.
        selector = source/'services/cryptexd/build_support/development_policy.rs'
        probe = work/'policy.rs'
        probe.write_text('#[path=' + json.dumps(selector.as_posix()) + '] mod policy;\n'
                         'fn main() { let id=std::env::args().nth(1).unwrap();\n'
                         'let p=policy::select(true,Some(&id)).unwrap(); assert_eq!(p.release_id,id);\n'
                         'assert!(policy::select(true,Some("cryptex-lan-aaaaaaaaaaaa")).is_err()); }\n')
        run(['rustc', '--edition=2024', probe, '-o', work/'policy'], 'compile_release_policy_probe')
        run([work/'policy', product_release(commit)], 'verify_packet_release_policy')
        print('packet_release_matches_native_policy=passed', flush=True)
        run(['/usr/bin/openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1',
             '-subj', '/CN=Cryptex disposable archive build', '-keyout', work/'ci.key',
             '-out', work/'ci.pem'], 'create_disposable_build_certificate')
        certificate = work/'ci.der'
        run(['/usr/bin/openssl', 'x509', '-in', work/'ci.pem', '-outform', 'DER', '-out', certificate],
            'encode_disposable_public_pin')
        creator, release = compile_products(source, work, commit, certificate, run, {})
        assert creator.is_file() and release == product_release(commit)
        for name in ('cryptexd', 'cryptex-desktop'):
            assert (work/'target/debug'/name).stat().st_size > 0
        print('owner_packet_archive_product_compile=passed', flush=True)


if __name__ == '__main__': main()
