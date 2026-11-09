import importlib.util
from contextlib import redirect_stdout
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.dont_write_bytecode = True
import mac_xpc_probe as probe
from mac_identity_preflight import CommandResult, PreflightError

spec = importlib.util.spec_from_file_location("xpc_packager", Path(__file__).with_name("package-macos-xpc.py"))
packager = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packager)
ci_spec = importlib.util.spec_from_file_location("xpc_ci_smoke", Path(__file__).with_name("mac_xpc_ci_smoke.py"))
ci = importlib.util.module_from_spec(ci_spec)
ci_spec.loader.exec_module(ci)


class SyntheticXpcTests(unittest.TestCase):
    def setUp(self):
        # Unit tests exercise exact target ownership on either authoring OS.
        uid = patch.object(probe.os, 'getuid', return_value=501, create=True)
        uid.start()
        self.addCleanup(uid.stop)

    def test_identity_selection_requires_one_complete_unambiguous_inventory(self):
        first = ' 1) ' + 'A' * 40 + ' "PRIVATE NAME (ABCDEFGHIJ)"\n'
        self.assertEqual(probe.select_identity(first + ' 1 valid identities found\n'), 'A' * 40)
        for inventory in ('0 valid identities found\n', first + '0 valid identities found\n',
                          first + first.replace('A' * 40, 'B' * 40) + '2 valid identities found\n'):
            with self.assertRaises(PreflightError):
                probe.select_identity(inventory)

    def test_requirement_is_exact_certificate_and_fixed_identifier(self):
        text = probe.requirement(probe.CLIENT, 'A' * 40)
        self.assertIn('identifier "' + probe.CLIENT + '"', text)
        self.assertIn('certificate leaf = H"' + 'A' * 40 + '"', text)
        for identifier, certificate in [('arbitrary', 'A' * 40), (probe.CLIENT, 'bad" or true')]:
            with self.assertRaises(PreflightError):
                probe.requirement(identifier, certificate)

    def test_transport_failures_are_never_authorization_rejections(self):
        for result in (CommandResult(-9, ''), CommandResult(0, '{"result":"peer_rejected"}', timed_out=True),
                       CommandResult(0, '{"result":"peer_rejected"}', limited=True),
                       CommandResult(0, '{"result":"peer_rejected"}', pipe_failed=True),
                       CommandResult(0, '{"result":"echo","result":"peer_rejected"}')):
            self.assertNotEqual(probe.client_result(result), 'peer_rejected')
        self.assertEqual(probe.client_result(CommandResult(0, '{"result":"peer_rejected"}\n')), 'peer_rejected')

    def test_later_failure_or_interruption_cannot_erase_attempted_private_key_use(self):
        for error in (PreflightError('fixture_verify_failed'), KeyboardInterrupt()):
            with tempfile.TemporaryDirectory() as directory:
                work = Path(directory)
                (work / 'native').write_bytes(b'synthetic compiler output')
                (work / 'mac_xpc_probe.c').write_bytes(b'synthetic source')
                report = {'private_key_use': 'not_requested'}

                def checked(_run, args, diagnostic, timeout=20):
                    if diagnostic == 'identity_inventory':
                        return '1) ' + 'A' * 40 + ' "PRIVATE NAME"\n1 valid identities found\n'
                    if diagnostic == 'sdk_path':
                        return '/SDK'
                    if diagnostic == 'fixture_verify':
                        raise error
                    return ''

                with patch.object(probe, 'scalar', return_value='27.2'), patch.object(probe, 'checked', checked), self.assertRaises(type(error)):
                    probe.collect(report, work, work)
                self.assertEqual(report['private_key_use'], 'requested_for_synthetic_fixture_signing')

    def test_cases_need_matching_delivery_lane_and_explicit_refusal(self):
        nonce = 'a' * 32

        def conclude(name, result, events):
            controls = []
            baseline = []
            for control, token, lane in [('expected_guard', 'b', 'guard'), ('expected_after_negatives', 'c', 'guard'),
                                         ('expected_observer', 'd', 'observe')]:
                if control != name:
                    controls.append({'name': control, 'nonce': token * 32, 'client_result': 'echo'})
                    baseline.append({'nonce': token * 32, 'lane': lane, 'op': 'echo', 'valid': True})
            if name.endswith('_guard') and not name.startswith('expected'):
                controls.append({'name': name[:-6] + '_observer', 'nonce': 'e' * 32, 'client_result': 'message_rejected'})
                baseline.append({'nonce': 'e' * 32, 'lane': 'observe', 'op': 'echo', 'valid': False})
            return probe.conclude(controls + [{'name': name, 'nonce': nonce, 'client_result': result}], baseline + events)[-1]['verdict']

        guard = {'nonce': nonce, 'lane': 'guard', 'op': 'echo', 'valid': True}
        observe = dict(guard, lane='observe', valid=False)
        self.assertEqual(conclude('expected_guard', 'echo', [guard]), 'pass')
        self.assertNotEqual(conclude('expected_guard', 'echo', [dict(guard, lane='observe')]), 'pass')
        self.assertEqual(conclude('wrong_id_observer', 'message_rejected', [observe]), 'pass')
        self.assertNotEqual(conclude('wrong_id_observer', 'message_rejected', []), 'pass')
        for status in ('connection_error', 'client_launch_or_exit_failure', 'runner_bound_failure'):
            self.assertNotEqual(conclude('wrong_id_guard', status, []), 'pass')
        self.assertEqual(conclude('wrong_id_guard', 'peer_rejected', []), 'pass')
        self.assertNotEqual(conclude('wrong_id_guard', 'peer_rejected', [guard]), 'pass')
        self.assertEqual(conclude('forwarded_endpoint_after_origin_exit', 'echo', [guard]), 'signer_only_insufficient')

    def test_rejecting_everyone_or_missing_paired_observer_cannot_pass_negatives(self):
        cases = [{'name': name, 'nonce': token * 32, 'client_result': 'peer_rejected'}
                 for name, token in [('expected_guard', 'a'), ('expected_after_negatives', 'b'),
                                     ('expected_observer', 'c'), ('wrong_id_guard', 'd')]]
        self.assertTrue(all(c['verdict'] != 'pass' for c in probe.conclude(cases, [])))
        cases = [{'name': name, 'nonce': token * 32, 'client_result': result}
                 for name, token, result in [('expected_guard', 'a', 'echo'), ('expected_after_negatives', 'b', 'echo'),
                                             ('expected_observer', 'c', 'echo'), ('wrong_id_guard', 'd', 'peer_rejected')]]
        events = [{'nonce': token * 32, 'lane': lane, 'op': 'echo', 'valid': True}
                  for token, lane in [('a', 'guard'), ('b', 'guard'), ('c', 'observe')]]
        self.assertNotEqual(probe.conclude(cases, events)[-1]['verdict'], 'pass')

    def test_cleanup_only_uses_exact_owned_current_user_target(self):
        with tempfile.TemporaryDirectory(prefix='cryptex-mac-imza.') as directory:
            work = Path(directory)
            domain, label = probe.launch_target(work)
            marker = work / 'job-owned.json'
            marker.write_text(json.dumps({'domain': domain, 'label': label}))
            calls = []

            def run(args, timeout):
                calls.append(args)
                return CommandResult(0 if args[1] == 'bootout' else 113, '')

            self.assertEqual(probe.cleanup(work, run), 'registration_removed')
            self.assertEqual(calls, [['/bin/launchctl', 'bootout', domain + '/' + label],
                                     ['/bin/launchctl', 'print', domain + '/' + label]])
            self.assertFalse(marker.exists())
            marker.write_text(json.dumps({'domain': 'system', 'label': label}))
            calls.clear()
            self.assertEqual(probe.cleanup(work, run), 'cleanup_marker_invalid')
            self.assertEqual(calls, [])

    def test_cleanup_failure_keeps_ownership_marker_and_cannot_claim_removed(self):
        with tempfile.TemporaryDirectory(prefix='cryptex-mac-imza.') as directory:
            work = Path(directory)
            domain, label = probe.launch_target(work)
            marker = work / 'job-owned.json'
            marker.write_text(json.dumps({'domain': domain, 'label': label}))
            self.assertEqual(probe.cleanup(work, lambda *_: CommandResult(5, 'private')), 'cleanup_unconfirmed')
            self.assertTrue(marker.exists())

    def test_ci_probe_failure_still_emits_cleanup_and_unproven_process_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            output = io.StringIO()

            def checked(_run, _args, diagnostic, timeout=20):
                return 'CDHash=' + 'a' * 40 if diagnostic == 'adhoc_detail' else '/SDK'

            with (patch.object(ci.sys, 'platform', 'darwin'),
                  patch.dict(ci.os.environ, {'GITHUB_ACTIONS': 'true'}),
                  patch.object(ci.tempfile, 'mkdtemp', return_value=directory),
                  patch.object(ci, 'validate_work'), patch.object(ci, 'checked', checked),
                  patch.object(ci, 'exercise', side_effect=RuntimeError('injected')),
                  patch.object(ci, 'cleanup', return_value='registration_removed'),
                  redirect_stdout(output), self.assertRaisesRegex(RuntimeError, 'injected')):
                ci.main()
            receipt = json.loads(output.getvalue())
            self.assertEqual(receipt['scope'], 'disposable_ci_adhoc_smoke_only')
            self.assertEqual(receipt['temporary_job_cleanup'], 'registration_removed')
            self.assertEqual(receipt['server_process_exit'], 'not_independently_observed')

    def test_package_hashes_modes_source_binding_and_tampering(self):
        files = {name: Path(__file__).with_name(name).read_bytes() for name in probe.FILES}
        payload = packager.package_bytes('a' * 40, files)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with zipfile.ZipFile(io.BytesIO(payload)) as archive:
                self.assertEqual(set(archive.namelist()), set(probe.FILES) | {'MANIFEST.json'})
                self.assertEqual(archive.getinfo('MAC-IMZA.command').external_attr >> 16, 0o100755)
                archive.extractall(root)
            self.assertEqual(probe.verify_package(root), 'a' * 40)
            (root / 'mac_xpc_probe.c').write_text('tampered\n')
            with self.assertRaises(PreflightError):
                probe.verify_package(root)

    def test_invalid_or_unbounded_server_logs_refuse(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'server.jsonl'
            for value in ('{"private":"body"}', '{"lane":"guard","nonce":"invalid","valid":true,"op":"echo"}', 'x' * 65537):
                path.write_text(value)
                with self.assertRaises((PreflightError, ValueError)):
                    probe.read_events(path)


if __name__ == '__main__':
    unittest.main()
