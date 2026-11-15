import hashlib
import io
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch, Mock
from contextlib import redirect_stdout
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mac_identity_preflight import PreflightError
from mac_window_process import WindowProcess, natural_quit_ok, parse_group, window_progress
from mac_window_owner import exercise
from package_mac_window import build
from mac_development_diagnostic_package import export_failed
from mac_development_diagnostic_owner import verify_package


def clean(daemons=1):
    return dict(exit_code=0, natural_exit=True, same_group_live_descendants_before_cleanup=0,
                group_reaped=True, timed_out=False, output_limited=False, observer_failed=False,
                sampled_daemon_count=daemons, progress=[])


class ProcessTests(unittest.TestCase):
    def test_startup_diagnostics_keep_only_known_codes(self):
        output = '\n'.join([
            'development_startup: existing_profile_missing',
            'development_startup: local_signed_startup_Deadline',
            'development_startup: private memory body',
            '{"developmentStage":"start","private":"secret"}',
            '{"developmentStage":"private secret"}',
            '{"developmentStage":[]}', 'private path /Users/owner',
            '{broken json', '{"developmentStage":"bind"}'])
        self.assertEqual(window_progress(output), [
            {'startup_error': 'existing_profile_missing'},
            {'startup_error': 'local_signed_startup_Deadline'},
            {'development_stage': 'start'}, {'development_stage': 'bind'}])

    def test_forced_cleanup_or_surviving_child_cannot_pass_as_quit(self):
        self.assertTrue(natural_quit_ok(clean()))
        for key, value in [('natural_exit', False), ('same_group_live_descendants_before_cleanup', 1),
                           ('group_reaped', False), ('timed_out', True), ('exit_code', 2),
                           ('observer_failed', True), ('output_limited', True)]:
            value_set = clean()
            value_set[key] = value
            self.assertFalse(natural_quit_ok(value_set), key)
        self.assertFalse(natural_quit_ok({}))

    def test_group_snapshot_refuses_foreign_duplicate_and_unbounded_rows(self):
        self.assertEqual(parse_group('10 1 10 S /a path/cryptex-desktop\n11 10 10 Z cryptexd', 10)[11][2], 'cryptexd')
        for raw in ('11 10 12 S cryptexd', 'invalid', '10 1 10 S a\n10 1 10 S b', 'x' * 65537):
            with self.assertRaises(PreflightError):
                parse_group(raw, 10)

    def test_either_observer_thread_start_failure_still_reaps_registered_child(self):
        for which in (0, 1):
            with self.subTest(which=which):
                child = Mock(pid=123, stdout=io.BytesIO(b''), returncode=-9)
                threads = [Mock(ident=None), Mock(ident=None)]
                threads[which].start.side_effect = RuntimeError('cannot start')
                instance = WindowProcess(Path('/fixture/desktop'), Path('/private/home'))
                owned = [instance]  # Register before start, as the collector does.
                with patch('mac_window_process.subprocess.Popen', return_value=child), \
                        patch('mac_window_process.threading.Thread', side_effect=threads), \
                        patch.object(instance, 'snapshot', return_value={123: (1, 'S', 'desktop')}), \
                        patch('mac_window_process.os.killpg', create=True) as kill, \
                        patch('mac_window_process.signal.SIGKILL', 9, create=True), \
                        patch('mac_window_process.darwin_group_is_dead', return_value=True):
                    with self.assertRaises(RuntimeError):
                        instance.start()
                    result = owned[0].finish()
                    kill.assert_called_once()
                    child.wait.assert_called_once()
                    self.assertTrue(result['group_reaped'])
                    self.assertFalse(natural_quit_ok(result))


class FlowTests(unittest.TestCase):
    def simulate(self, mode):
        with tempfile.TemporaryDirectory() as temp, redirect_stdout(io.StringIO()):
            home = Path(temp)
            profile = home / 'Library/Application Support/Cryptex-Dev/runtime/p0-2'
            identity = profile / 'identity/local_device.json'
            record = profile / 'canonical/events/device/1.seg'
            calls, instances = [], []
            report = {}
            device = '00000000-0000-4000-8000-000000000001'

            def runner(args, timeout):
                calls.append(args)
                if args[0] == '/usr/bin/pgrep':
                    return SimpleNamespace(code=1, output='', timed_out=False, limited=False, pipe_failed=False)
                if args[0].endswith('profile-creator'):
                    identity.parent.mkdir(parents=True)
                    identity.write_text(json.dumps({'device_id': device}), encoding='utf-8')
                    record.parent.mkdir(parents=True)
                    record.write_bytes(b'signed fixture')
                return SimpleNamespace(code=0, output='', timed_out=False, limited=False, pipe_failed=False)

            class Process:
                def __init__(self, binary, selected_home):
                    self.number = len(instances)
                    instances.append(self)
                    self.result = None
                    self.started = False
                    self.assert_home = selected_home == home
                def start(self):
                    # Every normal opening now has a fully initialized fixture.
                    if not identity.exists() or not record.exists():
                        raise AssertionError('profile must precede the first opening')
                    self.started = True
                    if mode == 'thread_start' and self.number == 0:
                        raise RuntimeError('injected')
                def daemon_pin(self):
                    return {42}
                def finish(self, wait=0):
                    if mode == 'unresolved' and self.number == 0:
                        raise OSError('unknown writer')
                    if self.result is None:
                        self.result = clean(0 if self.number == 2 else 1)
                        if self.number == 0:
                            if mode in ('forced', 'thread_start'):
                                self.result['natural_exit'] = False
                            if mode == 'survivor':
                                self.result['same_group_live_descendants_before_cleanup'] = 1
                            if mode == 'changed':
                                record.write_bytes(b'changed')
                            if mode == 'identity_changed':
                                identity.write_text(json.dumps({'device_id': '00000000-0000-4000-8000-000000000002'}), encoding='utf-8')
                        if self.number == 2:
                            if mode == 'cancel_daemon':
                                self.result['sampled_daemon_count'] = 1
                            if mode == 'cancel_changed':
                                record.write_bytes(b'changed on cancel')
                    return self.result

            if mode == 'pass':
                exercise(home / 'package', home, report, lambda _: True, runner, Process)
                self.assertEqual(report['outcome'], 'bounded_window_checks_passed_with_owner_observations')
                self.assertTrue(report['canonical_unchanged_after_restart'])
                self.assertTrue(report['cancel_canonical_and_device_id_unchanged'])
                self.assertEqual(list(report['launches']), ['window_session', 'restart', 'opening_cancel'])
                self.assertEqual(len(report['owner_observations']), 10)
            else:
                with self.assertRaises((PreflightError, RuntimeError)):
                    exercise(home / 'package', home, report, lambda _: True, runner, Process)
                self.assertNotEqual(report.get('outcome'), 'bounded_window_checks_passed_with_owner_observations')
                if mode in ('forced', 'survivor'):
                    self.assertEqual(report['primary_diagnostic'], 'natural_quit_not_verified')
            cleanups = [c for c in calls if c[0] == '/usr/bin/security']
            if mode in ('forced', 'survivor', 'unresolved', 'thread_start', 'identity_changed'):
                self.assertFalse(cleanups)
                self.assertTrue(identity.exists())
            else:
                self.assertEqual(cleanups[0][-1], 'device-event-signing-seed:' + device)
            self.assertTrue(all(i.assert_home for i in instances))
            self.assertNotIn('signed fixture', json.dumps(report))

    def test_success_preserves_canonical_and_records_owner_observations(self):
        self.simulate('pass')

    def test_failure_cannot_be_relabelled_by_final_cleanup(self):
        for mode in ('forced', 'survivor', 'unresolved', 'thread_start', 'changed', 'identity_changed',
                     'cancel_daemon', 'cancel_changed'):
            with self.subTest(mode=mode):
                self.simulate(mode)


class PackageTests(unittest.TestCase):
    def test_preserves_exact_public_images_and_executable_modes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            bundle = root / 'app'
            files = []
            for name in ('Contents/MacOS/cryptex-desktop', 'Contents/MacOS/cryptexd',
                         'Contents/Resources/frontend/index.html'):
                path = bundle / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b'synthetic signed image')
                files.append({'path': name, 'size': path.stat().st_size,
                              'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
            for name in ('m8-startup.json', 'm8-startup.cms'):
                (bundle / 'Contents/Resources' / name).write_bytes(b'public metadata')
            creator = root / 'creator'
            creator.write_bytes(b'creator')
            source = root / 'source.zip'
            export_failed(bundle, files, creator, source, 'a' * 40, 'arm64', 'notAfter=Nov 20 00:00:00 2099 GMT')
            digest = hashlib.sha256(source.read_bytes()).hexdigest()
            output = root / 'window.zip'
            build(source, output, digest, 'b' * 40)
            with zipfile.ZipFile(output) as z:
                prefix = 'Cryptex-Mac-Pencere/'
                meta = json.loads(z.read(prefix + 'diagnostic.json'))
                self.assertIs(meta['ci_startup_passed'], False)
                self.assertEqual(meta['source'], 'a' * 40)
                self.assertEqual(meta['collector_source'], 'b' * 40)
                self.assertEqual(z.read(prefix + 'development.app/Contents/MacOS/cryptex-desktop'), b'synthetic signed image')
                self.assertEqual(z.getinfo(prefix + 'MAC-PENCERE.command').external_attr >> 16, 0o100700)
                self.assertFalse(any('key.pem' in n or 'BASLANGIC-TANI.command' in n for n in z.namelist()))
                z.extractall(root / 'extracted')
            verify_package(root / 'extracted/Cryptex-Mac-Pencere', meta)
            with self.assertRaisesRegex(ValueError, 'source_archive_changed'):
                build(source, root / 'bad.zip', '0' * 64, 'b' * 40)


if __name__ == '__main__':
    unittest.main()
