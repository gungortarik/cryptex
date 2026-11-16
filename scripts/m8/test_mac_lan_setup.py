import hashlib
import io
import json
import os
from pathlib import Path
import ssl
import sys
import tempfile
import unittest
import zipfile
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).parent))
from mac_lan_setup import public_certificate, source_members, setup, build_frontend, build_diagnostic, tool, compile_products, product_release
from mac_identity_preflight import PreflightError, CommandResult, run_bounded, checked

class LanSetupTests(unittest.TestCase):
    def test_failed_product_compile_restores_environment_and_never_reaches_setup(self):
        with tempfile.TemporaryDirectory() as temp:
            work = Path(temp).resolve()
            source = work/'source'; source.mkdir()
            (work/'target/debug').mkdir(parents=True)
            calls, previous = [], Path.cwd()
            def run(args, stage, timeout):
                calls.append(stage)
                self.assertEqual(os.environ['CRYPTEX_M8_DEVELOPMENT_RELEASE_ID'], product_release('a'*40))
                self.assertEqual(os.environ['CRYPTEX_M8_DEVELOPMENT_CERT_DER'], str(work/'public.der'))
                if stage == 'public_creator_build':
                    (work/'target/debug/cryptexd').write_bytes(b'fixture creator')
                else:
                    raise PreflightError('injected_product_build_failure')
            with patch('mac_lan_setup.tool', side_effect=lambda name: Path('/tools')/name), \
                 patch('mac_lan_setup.build_frontend'), \
                 patch.dict(os.environ, CRYPTEX_M8_DEVELOPMENT_RELEASE_ID='previous', CARGO_TARGET_DIR='previous-target'):
                with self.assertRaisesRegex(PreflightError, 'injected_product_build_failure'):
                    compile_products(source, work, 'a'*40, work/'public.der', run, {})
                self.assertEqual(os.environ['CRYPTEX_M8_DEVELOPMENT_RELEASE_ID'], 'previous')
                self.assertEqual(os.environ['CARGO_TARGET_DIR'], 'previous-target')
            self.assertEqual(Path.cwd(), previous)
            self.assertEqual(calls, ['public_creator_build', 'product_build'])
            self.assertFalse((work/'Cryptex-LAN.app').exists())
        with self.assertRaisesRegex(PreflightError, 'product_source_invalid'):
            product_release('invalid/source')

    def test_tool_lookup_prefers_explicit_path_and_ignores_current_directory(self):
        selected = str(Path.home()/'selected-tools')
        with patch.dict(os.environ, PATH=os.pathsep.join(['', '.', selected])), \
             patch('mac_lan_setup.shutil.which', return_value=None) as which:
            with self.assertRaisesRegex(PreflightError, 'build_tool_missing_node'):
                tool('node')
        search = which.call_args.kwargs['path'].split(os.pathsep)
        self.assertEqual(search[0], selected)
        self.assertNotIn('', search)
        self.assertNotIn('.', search)

    def test_tool_version_mismatch_stops_before_dependency_install(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root/'package.json').write_text(json.dumps({'engines': {'node': '24.19.0', 'pnpm': '11.28.0'}}))
            calls, report = [], {}
            def run(args, label, timeout):
                calls.append(label)
                return 'v24.19.0' if label == 'node_version' else '10.0.0'
            with self.assertRaisesRegex(PreflightError, '^pnpm_version_mismatch$'):
                build_frontend(root, Path('/node'), Path('/pnpm'), run, report)
            self.assertEqual(calls, ['node_version', 'pnpm_version'])
            self.assertEqual(report['build_tools'], {'node': '24.19.0', 'pnpm': '10.0.0'})

    @unittest.skipUnless(os.name == 'posix', 'actual executable/shebang test runs on Mac CI')
    def test_frontend_executes_shell_launcher_instead_of_parsing_it_as_javascript(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root/'package.json').write_text(json.dumps({'engines': {'node': '24.19.0', 'pnpm': '11.28.0'}}))
            node, pnpm = root/'node', root/'pnpm'
            # Node can only report its version. Feeding it another program fails.
            node.write_text('#!/bin/sh\n[ "$1" = "--version" ] || exit 71\nprintf "v24.19.0\\n"\n')
            pnpm.write_text('#!/bin/sh\ncase "$1" in\n--version) printf "11.28.0\\n";;\n'
                            'install) [ "$2" = "--frozen-lockfile" ] || exit 72;;\n'
                            '--filter) [ "$2" = "@cryptex/desktop" ] && [ "$3" = "build" ] || exit 73;;\n'
                            '*) exit 74;;\nesac\n')
            node.chmod(0o700); pnpm.chmod(0o700)
            def run(args, label, timeout):
                return checked(run_bounded, [str(a) for a in args], label, timeout)
            self.assertEqual(run_bounded([str(node), str(pnpm), 'install'], 5).code, 71)
            report = {}
            build_frontend(root, node, pnpm, run, report)
            self.assertEqual(report['build_tools'], {'node': '24.19.0', 'pnpm': '11.28.0'})

    def test_build_failure_never_exports_raw_output_or_unknown_codes(self):
        value = build_diagnostic('frontend_dependencies', CommandResult(1,
            'ERR_PNPM_UNSUPPORTED_ENGINE private-token https://example.invalid/secret ENOSPC UNKNOWN_SECRET'))
        self.assertEqual(value['codes'], ['ENOSPC', 'ERR_PNPM_UNSUPPORTED_ENGINE'])
        self.assertNotIn('secret', json.dumps(value))
        self.assertIsNone(build_diagnostic('sign_inventory_keychain', CommandResult(1, 'ENOSPC secret')))

    def test_public_certificate_is_selected_by_exact_existing_identity(self):
        der = b'public certificate fixture'
        pem = ssl.DER_cert_to_PEM_cert(der)
        fingerprint = hashlib.sha1(der).hexdigest().upper()
        self.assertEqual(public_certificate(pem, fingerprint), der)
        with self.assertRaises(PreflightError): public_certificate(pem, '0'*40)

    def test_archive_refuses_alias_traversal_and_links_before_writes(self):
        for name, mode in [('../outside', 0o100600), ('a//b', 0o100600), ('a\\b', 0o100600),
                           ('/outside', 0o100600), ('link', 0o120777)]:
            raw = io.BytesIO()
            with zipfile.ZipFile(raw, 'w') as archive:
                entry = zipfile.ZipInfo(name)
                entry.filename = name  # Preserve adversarial backslashes on Windows too.
                entry.external_attr = mode << 16
                archive.writestr(entry, b'x')
            with zipfile.ZipFile(raw) as archive:
                with self.assertRaises(PreflightError): source_members(archive)

    def test_setup_order_product_owns_keys_and_note_and_never_cleans_profile(self):
        calls = []
        class Profile:
            def __init__(self, home): self.path = home/'runtime'
            def acquire(self): self.path.mkdir(); return self.path
            def check(self): pass
            def close(self): calls.append('close')
            def remove(self): raise AssertionError('persistent profile removed')
        def run(args, stage, timeout, **kwargs):
            calls.append((stage, args, kwargs))
            return {'first_note_capture': 'first_note=committed_public',
                    'product_transport_enrollment': 'transport_enrollment=local_only',
                    'initialized_startup_restart': 'development_existing_profile_startup_and_cleanup=passed'}.get(stage, '')
        with tempfile.TemporaryDirectory() as temp, patch('mac_lan_setup.DefaultProfile', Profile):
            report = {}
            setup(Path('/bundle'), Path('/creator'), Path(temp), {'run_id': 'run', 'brain_id': 'brain'}, run, lambda _: True, report)
            self.assertTrue(report['persistent_profile_preserved'])
            self.assertEqual([c[0] for c in calls[:-1]], ['public_brain_create', 'product_key_enrollment',
                'first_note_capture', 'product_transport_enrollment', 'initialized_startup_restart', 'initialized_startup_restart'])
            self.assertEqual(calls[0][1][0], Path('/creator'))
            self.assertEqual(calls[1][1][0], Path('/bundle/Contents/MacOS/cryptexd'))
            self.assertIn('input_data', calls[2][2])
            self.assertTrue((Path(temp)/'runtime/pilot-runtime.marker').exists())

    def test_declining_setup_creates_nothing(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaises(PreflightError):
                setup(Path('/bundle'), Path('/creator'), Path(temp), {'run_id': 'run'},
                      lambda *a: self.fail('command ran'), lambda _: False, {})
            self.assertEqual(list(Path(temp).iterdir()), [])

if __name__ == '__main__': unittest.main()
