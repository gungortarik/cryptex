from contextlib import redirect_stdout
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import uuid
import hashlib
import zipfile

from mac_identity_preflight import CommandResult, PreflightError
from mac_normal_profile import DefaultProfile
from mac_normal_owner import exercise
from mac_window_process import WindowProcess
from test_mac_window import clean
from mac_development_diagnostic_package import export_failed
from mac_development_diagnostic_owner import verify_package
from package_mac_normal import build


class NormalArgumentsTests(unittest.TestCase):
    def test_normal_package_preserves_signed_images_and_records_distinct_scope(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bundle = root / 'app'
            files = []
            for name in ('Contents/MacOS/cryptex-desktop', 'Contents/MacOS/cryptexd',
                         'Contents/Resources/frontend/index.html'):
                path = bundle / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b'signed fixture bytes')
                files.append({'path': name, 'size': path.stat().st_size,
                              'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
            for name in ('m8-startup.json', 'm8-startup.cms'):
                (bundle / 'Contents/Resources' / name).write_bytes(b'public')
            creator = root / 'creator'
            creator.write_bytes(b'fixture image')
            source, output = root / 'source.zip', root / 'normal.zip'
            export_failed(bundle, files, creator, source, 'a' * 40, 'arm64', 'notAfter=Nov 20 00:00:00 2099 GMT')
            build(source, output, hashlib.sha256(source.read_bytes()).hexdigest(), 'b' * 40)
            with zipfile.ZipFile(output) as archive:
                self.assertIsNone(archive.testzip())
                meta = json.loads(archive.read('Cryptex-Mac-Normal/diagnostic.json'))
                self.assertEqual(meta['normal_test'], 'absent_default_root_v1')
                self.assertNotIn('window_test', meta)
                self.assertNotIn('Cryptex-Mac-Normal/MAC-PENCERE.command', archive.namelist())
                self.assertEqual(archive.getinfo('Cryptex-Mac-Normal/MAC-NORMAL.command').external_attr >> 16, 0o100700)
                self.assertEqual(archive.read('Cryptex-Mac-Normal/development.app/Contents/MacOS/cryptexd'), b'signed fixture bytes')
                archive.extractall(root / 'extracted')
            verify_package(root / 'extracted/Cryptex-Mac-Normal', meta)

    def test_no_argument_launch_retains_real_home(self):
        with patch.dict(os.environ, {'HOME': '/owner'}):
            process = WindowProcess(Path('/app/desktop'), None)
            self.assertEqual(process.arguments(), [str(Path('/app/desktop'))])
            self.assertEqual(process.env['HOME'], '/owner')
            explicit = WindowProcess(Path('/app/desktop'), Path('/test'))
            self.assertEqual(explicit.arguments(), [str(Path('/app/desktop')), '--cryptex-development-window-check', str(Path('/test'))])


@unittest.skipUnless(sys.platform == 'darwin', 'actual Darwin directory descriptors required')
class DefaultDirectoryTests(unittest.TestCase):
    def test_existing_directory_file_and_dangling_link_are_preserved(self):
        for kind in ('directory', 'file', 'link'):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as root:
                lease = DefaultProfile(Path(root).resolve())
                lease.path.parent.mkdir(parents=True)
                if kind == 'directory':
                    lease.path.mkdir()
                elif kind == 'file':
                    lease.path.write_bytes(b'existing')
                else:
                    lease.path.symlink_to('absent-target')
                before = lease.path.lstat()
                try:
                    with self.assertRaisesRegex(PreflightError, 'normal_profile_already_exists'):
                        lease.acquire()
                    self.assertEqual(lease.path.lstat().st_ino, before.st_ino)
                    self.assertFalse(lease.created)
                finally:
                    lease.close()

    def test_cleanup_is_confined_to_owned_leaf_and_refuses_replacement(self):
        with tempfile.TemporaryDirectory() as root:
            lease = DefaultProfile(Path(root).resolve())
            try:
                path = lease.acquire()
                (path / 'events').mkdir()
                (path / 'events/record').write_bytes(b'synthetic')
                parent = path.parent
                lease.remove()
                self.assertFalse(path.exists())
                self.assertTrue(parent.exists())
            finally:
                lease.close()
        with tempfile.TemporaryDirectory() as root:
            lease = DefaultProfile(Path(root).resolve())
            try:
                path = lease.acquire()
                path.rename(path.with_name('original'))
                path.mkdir(mode=0o700)
                with self.assertRaisesRegex(PreflightError, 'normal_profile_path_changed'):
                    lease.remove()
                self.assertTrue(path.exists())
                self.assertTrue(path.with_name('original').exists())
            finally:
                lease.close()

    def test_symlink_inside_fixture_prevents_all_deletion(self):
        with tempfile.TemporaryDirectory() as root:
            lease = DefaultProfile(Path(root).resolve())
            try:
                path = lease.acquire()
                (path / 'record').write_bytes(b'synthetic')
                (path / 'link').symlink_to('/tmp')
                with self.assertRaisesRegex(PreflightError, 'normal_cleanup_unsafe_object'):
                    lease.remove()
                self.assertTrue((path / 'record').exists())
            finally:
                lease.close()


class NormalFlowTests(unittest.TestCase):
    def run_flow(self, root, *, mutate=False, exit_code=0, answers=True):
        path = Path(root) / 'runtime'
        calls = []
        class Lease:
            created = False
            removed = False
            def acquire(self):
                path.mkdir()
                self.created = True
                return path
            def check(self):
                pass
            def remove(self):
                self.removed = True
                self.created = False
        lease = Lease()
        lease.path = path
        class Process:
            def __init__(self, binary, profile):
                self.profile = profile
            def start(self):
                self.started = True
                if mutate:
                    (path / 'canonical/events/record').write_bytes(b'changed')
            def daemon_pin(self):
                return {123}
            def finish(self, **kwargs):
                return dict(clean(), exit_code=exit_code)
        def runner(args, timeout):
            args = [str(a) for a in args]
            calls.append(args)
            if args[0] == '/usr/bin/pgrep':
                return CommandResult(1, '')
            if 'signed-transfer-v1' in args:
                (path / 'identity').mkdir()
                (path / 'identity/local_device.json').write_text(json.dumps({'device_id': str(uuid.uuid4())}))
                (path / 'canonical/events').mkdir(parents=True)
                (path / 'canonical/events/record').write_bytes(b'synthetic')
            return CommandResult(0, '')
        report = {}
        error = None
        with redirect_stdout(io.StringIO()):
            try:
                exercise(Path('/package'), lease, report, ask=lambda _: answers, runner=runner, process_type=Process)
            except PreflightError as caught:
                error = caught
        return report, calls, lease, error

    def test_two_clean_launches_cleanup_only_exact_generated_key(self):
        with tempfile.TemporaryDirectory() as root:
            report, calls, lease, error = self.run_flow(root)
            self.assertIsNone(error)
            self.assertTrue(lease.removed)
            deletes = [x for x in calls if 'delete-generic-password' in x]
            self.assertEqual(len(deletes), 1)
            self.assertTrue(deletes[0][-1].startswith('device-event-signing-seed:'))
            self.assertEqual(set(report['launches']), {'normal_open', 'normal_restart'})
            self.assertEqual(report['outcome'], 'normal_default_locator_open_restart_passed')

    def test_changed_data_failed_exit_or_owner_denial_retains_key_and_profile(self):
        for options in ({'mutate': True}, {'exit_code': 2}, {'answers': False}):
            with self.subTest(options=options), tempfile.TemporaryDirectory() as root:
                report, calls, lease, error = self.run_flow(root, **options)
                self.assertIsNotNone(error)
                self.assertFalse(lease.removed)
                self.assertTrue(report['test_profile_retained'])
                self.assertFalse(any('delete-generic-password' in x for x in calls))


if __name__ == '__main__':
    unittest.main()
