import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch
from types import SimpleNamespace
import zipfile

sys.dont_write_bytecode = True
import mac_identity_preflight as probe

spec = importlib.util.spec_from_file_location("mac_identity_packager", Path(__file__).with_name("package-macos-identity.py"))
packager = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packager)


class IdentityPreflightTests(unittest.TestCase):
    def test_darwin_queue_only_needs_close_not_context_manager_support(self):
        class Queue:
            closed = False

            def control(self, changes, count, timeout):
                return [SimpleNamespace(flags=0)]

            def close(self):
                self.closed = True

        queue = Queue()
        api = SimpleNamespace(kqueue=lambda: queue, kevent=lambda *a, **k: None,
                              KQ_FILTER_PROC=1, KQ_EV_ADD=2, KQ_EV_ONESHOT=4,
                              KQ_NOTE_EXIT=8, KQ_EV_ERROR=16)
        with patch.object(probe, 'select', api), patch.object(probe.sys, 'platform', 'darwin'):
            self.assertTrue(probe.wait_without_reaping(123, 1))
        self.assertTrue(queue.closed)

    def test_inventory_exports_only_counts_and_does_not_infer_key_use(self):
        text = ('  1) ' + 'A' * 40 + ' "Apple Development: PRIVATE PERSON (ABCDEFGHIJ)"\n'
                '  2) ' + 'B' * 40 + ' "Developer ID Application: PRIVATE PERSON (ABCDEFGHIJ)"\n'
                '  2 valid identities found\n')
        data = probe.identity_counts(text)
        self.assertEqual(data, {"valid_file_keychain_entries": 2, "distinct_file_keychain_certificates": 2,
                                "distinct_apple_team_labels_observed": 1})
        for secret in ("PRIVATE", "ABCDEFGHIJ", "A" * 40, "B" * 40):
            self.assertNotIn(secret, json.dumps(data))
        self.assertEqual(probe.identity_counts(' 0 valid identities found\n')["valid_file_keychain_entries"], 0)

    def test_malformed_incomplete_and_duplicate_summaries_refuse(self):
        for text in ('', '0 valid identities found\n0 valid identities found\n', '1 valid identities found\n',
                     'private diagnostic\n0 valid identities found\n', '1) BAD "name"\n1 valid identities found\n'):
            with self.subTest(text=text), self.assertRaises(probe.PreflightError):
                probe.identity_counts(text)

    def test_deadline_output_limit_and_nonzero_exit_cannot_become_collection_success(self):
        for result in (probe.CommandResult(0, "0 valid identities found", limited=True),
                       probe.CommandResult(0, "0 valid identities found", timed_out=True), probe.CommandResult(7, "private data")):
            with self.assertRaises(probe.PreflightError) as error:
                probe.checked(lambda *_: result, ['unused'], 'identity_inventory')
            self.assertNotIn('private', str(error.exception))

    def test_complete_preflight_still_cannot_claim_native_xpc_or_minimum_os_proof(self):
        calls = []

        def run(args, timeout):
            calls.append(args)
            if args[0].endswith('sw_vers'):
                output = '27.2' if args[-1] == '-productVersion' else '26B123'
            elif args[0].endswith('uname'):
                output = 'arm64'
            elif args[0].endswith('security'):
                output = '0 valid identities found\n'
            elif args[-1] == '--show-sdk-version':
                output = '27.0'
            elif args[-1] == '--show-sdk-path':
                output = '/synthetic-sdk'
            elif args[0].endswith('api-availability'):
                output = '\n'.join(name + '=present' for name in probe.SYMBOLS)
            else:
                output = ''
            return probe.CommandResult(0, output)

        facts = probe.new_report()
        probe.collect(facts, run, Path('/package'), Path('/work'))
        self.assertEqual(facts['outcome'], 'prerequisites_collected')
        self.assertFalse(facts['signed_matrix_prerequisites_proven'])
        self.assertEqual(facts['native_xpc_matrix'], 'not_run')
        self.assertEqual(facts['product_minimum_os'], 'unconfirmed')
        self.assertFalse(facts['m8_complete'])
        self.assertFalse(facts['signing_selection_needed'])
        self.assertFalse(any('codesign' in args[0] or 'launchctl' in args[0] for args in calls))

    def test_package_is_bound_executable_and_tampering_is_refused(self):
        files = {name: Path(__file__).with_name(name).read_bytes().replace(b'\r\n', b'\n') for name in probe.FILES}
        payload = packager.package_bytes('a' * 40, files)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with zipfile.ZipFile(io.BytesIO(payload)) as archive:
                self.assertEqual(archive.namelist(), [*probe.FILES, 'MANIFEST.json'])
                self.assertEqual(archive.getinfo('MAC-KIMLIK.command').external_attr >> 16, 0o100755)
                archive.extractall(root)
            self.assertEqual(probe.verify_package(root), 'a' * 40)
            (root / 'mac_identity_api.c').write_bytes(b'tampered\n')
            with self.assertRaises(probe.PreflightError):
                probe.verify_package(root)
            (root / 'MANIFEST.json').write_text('{"schema":1,"schema":1}', encoding='utf-8')
            with self.assertRaises(probe.PreflightError):
                probe.verify_package(root)

    def test_non_mac_entry_refuses_before_creating_runtime(self):
        if sys.platform == 'darwin':
            self.skipTest('non-Mac refusal applies to the Windows authoring host')
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = subprocess.run([sys.executable, str(Path(probe.__file__)), '--package', str(root), '--work', str(root)], capture_output=True)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(list(root.iterdir()), [])

    @unittest.skipUnless(os.name == 'posix', 'native process-group deadline verification runs on Mac CI')
    def test_native_process_deadline_is_bounded(self):
        result = probe.run_bounded([sys.executable, '-c', 'import time; time.sleep(20)'], 0.1)
        self.assertTrue(result.timed_out)
        self.assertNotEqual(result.code, 0)

    @unittest.skipUnless(os.name == 'posix', 'native process-group output cap verification runs on Mac CI')
    def test_native_output_cap_is_bounded(self):
        result = probe.run_bounded([sys.executable, '-c', 'print("x" * 200000)'], 5)
        self.assertTrue(result.limited)
        self.assertLessEqual(len(result.output.encode()), probe.OUTPUT_LIMIT)

    @unittest.skipUnless(os.name == 'posix', 'native descendant cleanup runs on Mac CI')
    def test_successful_leader_cannot_leave_a_pipe_detached_descendant(self):
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / 'escaped'
            ready = Path(directory) / 'ready'
            release = Path(directory) / 'release'
            code = ('import os,time,sys\n'
                    'pid=os.fork()\n'
                    'if pid == 0:\n'
                    ' os.close(1); os.close(2)\n'
                    ' with open(sys.argv[2]+".tmp","w") as f: f.write(str(os.getpid()))\n'
                    ' os.replace(sys.argv[2]+".tmp",sys.argv[2])\n'
                    ' end=time.monotonic()+10\n'
                    ' while not os.path.exists(sys.argv[3]) and time.monotonic()<end: time.sleep(0.01)\n'
                    ' if os.path.exists(sys.argv[3]): open(sys.argv[1],"w").write("escaped")\n'
                    ' os._exit(0)\n'
                    'while not os.path.exists(sys.argv[2]): time.sleep(0.01)\n'
                    'os._exit(0)\n')
            result = probe.run_bounded([sys.executable, '-c', code, str(marker), str(ready), str(release)], 5)
            self.assertEqual(result.code, 0)
            self.assertFalse(result.timed_out)
            # The leader cannot exit until the child is ready. The child cannot
            # write the marker until the collector has returned and we release it.
            pid = int(ready.read_text())
            release.write_text('release', encoding='utf-8')
            deadline = time.monotonic() + 5
            while True:
                self.assertFalse(marker.exists(), 'owned descendant continued after collection')
                observed = subprocess.run(['/bin/ps', '-p', str(pid), '-o', 'stat='], capture_output=True, text=True, timeout=1)
                if observed.returncode == 1 or observed.stdout.strip().startswith('Z'):
                    self.assertFalse(marker.exists())
                    break  # Gone or dead-awaiting-reap, rather than merely descheduled.
                self.assertEqual(observed.returncode, 0)
                if time.monotonic() >= deadline:
                    self.fail('owned descendant remained live after collection')
                time.sleep(0.01)


if __name__ == '__main__':
    unittest.main()
