import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import mac_existing_update as update


@unittest.skipIf(os.name == 'nt', 'requires pinned POSIX directory handles')
class ExistingUpdateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name).resolve()
        self.package = self.home / 'packet'
        self.package.mkdir()
        self.root = self.home / 'Library/Application Support/Cryptex-Dev/runtime/p0-2'
        self.identity = self.root / 'identity'
        self.identity.mkdir(parents=True)
        (self.identity / 'brain_identity.json').write_text(json.dumps({'brain_id': update.BRAIN}))
        (self.identity / 'local_device.json').write_text(json.dumps(
            {'brain_id': update.BRAIN, 'device_id': update.MAC}))
        self.event = self.root / 'canonical/events/untouched.seg'
        self.event.parent.mkdir(parents=True)
        self.event.write_bytes(b'existing canonical fixture - do not rewrite')
        self.old_app = self.home / 'old-app'
        self.old_app.write_bytes(b'old signed app fixture')
        with zipfile.ZipFile(self.package / 'source.zip', 'w') as archive:
            archive.writestr('fixture.txt', 'source fixture')
        self.plan = dict(source_commit='a'*40, source_sha256=hashlib.sha256(
            (self.package / 'source.zip').read_bytes()).hexdigest())
        self.observer = lambda: dict(desktop=0, daemon=0, engine=2)
        self.tools = patch('mac_existing_update.tool', side_effect=lambda name: Path('/tools') / name)
        self.tools.start()
        self.addCleanup(self.tools.stop)

    def builder(self, source, work, commit, run, report):
        self.assertEqual(commit, self.plan['source_commit'])
        bundle = work / 'app'
        (bundle / 'Contents/MacOS').mkdir(parents=True)
        (bundle / 'Contents/Resources').mkdir()
        (bundle / 'Contents/MacOS/cryptex-desktop').write_bytes(b'new app fixture')
        (bundle / 'Contents/Resources/m8-startup.json').write_text('{}')
        return bundle, work / 'creator-never-executed'

    def test_build_preserves_identity_events_and_old_app_without_enrollment(self):
        before = {p: (p.stat().st_ino, p.read_bytes()) for p in
                  [*self.identity.iterdir(), self.event, self.old_app]}
        report = {}
        with patch('mac_lan_setup.setup', side_effect=AssertionError('must never enroll')), \
             patch('mac_existing_update.subprocess.call', side_effect=AssertionError('prepare must not launch')):
            desktop = update.prepare(self.package, self.home, self.plan, report,
                                     self.builder, self.observer)
        self.assertTrue(desktop.exists())
        self.assertEqual(report['outcome'], 'built_beside_existing_app')
        for path, state in before.items():
            self.assertEqual((path.stat().st_ino, path.read_bytes()), state)
        launcher = self.package / 'CRYPTEX-AC-aaaaaaaa.command'
        self.assertIn('exec ', launcher.read_text())
        self.assertEqual(launcher.stat().st_mode & 0o777, 0o700)

    def test_wrong_profile_and_active_app_refuse_before_build(self):
        builder = lambda *_: self.fail('builder must not run')
        with self.assertRaisesRegex(update.Refused, 'quit_cryptex_before_update'):
            update.prepare(self.package, self.home, self.plan, {}, builder,
                           lambda: dict(desktop=1, daemon=0))
        (self.identity / 'local_device.json').write_text('{}')
        with self.assertRaisesRegex(update.Refused, 'wrong_mac_profile'):
            update.prepare(self.package, self.home, self.plan, {}, builder, self.observer)

    def test_verification_build_can_repeat_without_creating_or_replacing_a_launcher(self):
        launcher = self.package / 'CRYPTEX-AC-aaaaaaaa.command'
        launcher.write_text('existing launcher must remain unchanged')
        before = launcher.read_bytes()
        for _ in range(2):
            desktop = update.prepare(self.package, self.home, self.plan, {},
                                     self.builder, self.observer, create_launcher=False)
            self.assertTrue(desktop.exists())
            self.assertEqual(launcher.read_bytes(), before)
        self.assertEqual(list(self.package.glob('*.command')), [launcher])

    def test_identity_change_during_build_refuses_launcher(self):
        def builder(*args):
            result = self.builder(*args)
            (self.identity / 'local_device.json').write_text('{}')
            return result
        with self.assertRaisesRegex(update.Refused, 'wrong_mac_profile'):
            update.prepare(self.package, self.home, self.plan, {}, builder, self.observer)
        self.assertFalse(list(self.package.glob('*.command')))

    def test_replaced_root_refuses_launcher(self):
        def replace(*args):
            result = self.builder(*args)
            self.root.rename(self.root.with_name('preserved-original'))
            self.root.mkdir()
            return result
        with self.assertRaisesRegex(update.Refused, 'existing_profile_replaced'):
            update.prepare(self.package, self.home, self.plan, {}, replace, self.observer)
        self.assertFalse(list(self.package.glob('*.command')))

    def test_app_started_during_build_refuses_launcher_without_signalling(self):
        observations = iter([dict(desktop=0, daemon=0), dict(desktop=0, daemon=1)])
        with self.assertRaisesRegex(update.Refused, 'quit_cryptex_before_update'):
            update.prepare(self.package, self.home, self.plan, {}, self.builder,
                           lambda: next(observations))
        self.assertFalse(list(self.package.glob('*.command')))

    def test_build_failure_preserves_profile_and_creates_no_launcher(self):
        def failed(*_):
            raise update.PreflightError('injected_build_failure')
        before = self.event.read_bytes()
        with self.assertRaisesRegex(update.PreflightError, 'injected_build_failure'):
            update.prepare(self.package, self.home, self.plan, {}, failed, self.observer)
        self.assertEqual(self.event.read_bytes(), before)
        self.assertFalse(list(self.package.glob('*.command')))


if __name__ == '__main__':
    unittest.main()
