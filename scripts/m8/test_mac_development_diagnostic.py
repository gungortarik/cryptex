import hashlib
import io
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from contextlib import redirect_stdout
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mac_identity_preflight import PreflightError
from mac_development_diagnostic_owner import exercise, progress
from mac_development_diagnostic_package import export_failed


def outcome(code=0, output='', **extra):
    return SimpleNamespace(code=code, output=output, timed_out=False, limited=False,
                           pipe_failed=False, **extra)


class OwnerTests(unittest.TestCase):
    def test_cold_failure_is_not_relabelled_as_cold_success_after_owner_access(self):
        for mode in ('cold_pass', 'cold_wait', 'access_denied', 'changed', 'cleanup_failed'):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                calls = []
                report = {}
                marker = 'development_existing_profile_startup_and_cleanup=passed'

                def runner(args, timeout):
                    calls.append(args)
                    if len(calls) == 1:
                        records = root / 'runtime/canonical/events/device'
                        records.mkdir(parents=True)
                        (records / 'one.seg').write_bytes(b'signed synthetic bytes')
                        identity = root / 'runtime/identity'
                        identity.mkdir()
                        (identity / 'local_device.json').write_text(
                            json.dumps({'device_id': '00000000-0000-4000-8000-000000000001'}))
                    if args[0] == '/usr/bin/security':
                        self.assertEqual(args[-1], 'device-event-signing-seed:00000000-0000-4000-8000-000000000001')
                        return outcome(code=1 if mode == 'cleanup_failed' else 0)
                    if len(calls) == 2 and mode in ('cold_wait', 'access_denied'):
                        return outcome(2, '{"m8ProfileStage":"key_store","phase":"begin"}\nlocal_signed_startup_Deadline')
                    if len(args) > 1 and args[1] == 'identity':
                        self.assertEqual(timeout, 120)
                        return outcome(2 if mode == 'access_denied' else 0)
                    if len(calls) == 2 and mode == 'changed':
                        (root / 'runtime/canonical/events/device/one.seg').write_bytes(b'changed')
                    return outcome(output=marker)

                with redirect_stdout(io.StringIO()):
                    if mode in ('access_denied', 'changed', 'cleanup_failed'):
                        with self.assertRaises(PreflightError):
                            exercise(root / 'package', root, report, runner)
                    else:
                        exercise(root / 'package', root, report, runner)
                        self.assertEqual(report['cold_start_passed'], mode == 'cold_pass')
                        self.assertEqual(report['outcome'], 'cold_start_and_restart_passed' if mode == 'cold_pass'
                                         else 'restart_passed_after_explicit_identity_access')
                self.assertEqual(calls[-1][0], '/usr/bin/security')
                self.assertEqual(report['credential_cleanup_verified'], mode != 'cleanup_failed')

    def test_unresolved_process_does_not_delete_key_or_profile(self):
        with tempfile.TemporaryDirectory() as temp:
            calls = []
            def runner(args, timeout):
                calls.append(args)
                raise OSError('unresolved')
            report = {}
            with self.assertRaisesRegex(PreflightError, 'writer_unresolved'):
                exercise(Path(temp), Path(temp), report, runner)
            self.assertEqual(len(calls), 1)
            self.assertFalse(report['captured_group_reaped'])
            self.assertTrue((Path(temp) / 'runtime').exists())

    def test_missing_identity_after_successful_restarts_prevents_cleanup_success(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            calls = []
            def runner(args, timeout):
                calls.append(args)
                if len(calls) == 1:
                    records = root / 'runtime/canonical/events/device'
                    records.mkdir(parents=True)
                    (records / 'one.seg').write_bytes(b'synthetic')
                    identity = root / 'runtime/identity'
                    identity.mkdir()
                    (identity / 'local_device.json').write_text(
                        json.dumps({'device_id': '00000000-0000-4000-8000-000000000001'}))
                if len(calls) == 4:
                    (root / 'runtime/identity/local_device.json').unlink()
                return outcome(output='development_existing_profile_startup_and_cleanup=passed')
            report = {}
            with self.assertRaisesRegex(PreflightError, 'cleanup_identity_metadata_missing'):
                exercise(root, root, report, runner)
            self.assertEqual(len(calls), 4)
            self.assertFalse(report['credential_cleanup_verified'])
            self.assertFalse(any(args[0] == '/usr/bin/security' for args in calls))

    def test_progress_drops_private_and_unknown_data(self):
        output = '\n'.join(['private', '{"m8ProfileStage":"key_store","phase":"begin","key":"private"}',
                            '{"m8ProfileStage":"private","phase":"begin"}', 'local_signed_startup_Deadline'])
        self.assertNotIn('private', json.dumps(progress(output)))
        self.assertEqual(len(progress(output)), 2)


class ExportTests(unittest.TestCase):
    def test_failed_status_and_exact_files_are_exported_and_mutation_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            bundle = root / 'app'
            files = []
            for name in ('Contents/MacOS/cryptex-desktop', 'Contents/MacOS/cryptexd',
                         'Contents/Resources/frontend/index.html'):
                file = bundle / name
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(b'public synthetic fixture')
                files.append({'path': name, 'size': file.stat().st_size,
                              'sha256': hashlib.sha256(file.read_bytes()).hexdigest()})
            for name in ('m8-startup.json', 'm8-startup.cms'):
                (bundle / 'Contents/Resources' / name).write_bytes(b'public inventory')
            creator = root / 'creator'
            creator.write_bytes(b'signed fixture creator')
            output = root / 'out.zip'
            export_failed(bundle, files, creator, output, 'a' * 40, 'arm64',
                          'notAfter=Nov 20 00:00:00 2026 GMT')
            with zipfile.ZipFile(output) as archive:
                meta = json.loads(archive.read('Cryptex-Mac-Baslangic-Tani/diagnostic.json'))
                self.assertIs(meta['ci_startup_passed'], False)
                self.assertIs(meta['diagnostic_only'], True)
                self.assertFalse(any('key.pem' in name for name in archive.namelist()))
                mode = archive.getinfo('Cryptex-Mac-Baslangic-Tani/MAC-BASLANGIC-TANI.command').external_attr >> 16
                self.assertEqual(mode, 0o100700)
            (bundle / files[0]['path']).write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError, 'inventory_changed'):
                export_failed(bundle, files, creator, root / 'changed.zip', 'a' * 40, 'arm64',
                              'notAfter=Nov 20 00:00:00 2026 GMT')


if __name__ == '__main__':
    unittest.main()
