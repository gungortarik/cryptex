import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from mac_combined_loss_check import (guardian_identity, live_engine_snapshot,
    profile_snapshot, verify_exclusion)
from mac_identity_preflight import PreflightError


class CombinedLossTests(unittest.TestCase):
    def test_requires_dead_helper_live_guardian_and_distinct_live_engine_processes(self):
        before = b"20 10 20 Z\n21 1 20 S\n22 1 20 S\n23 22 20 S\n"
        engines = live_engine_snapshot(before, 20, 21, driver=10)
        self.assertEqual(engines, {22, 23})
        after = b"21 1 20 S\n22 1 20 S\n23 22 20 S\n"
        self.assertEqual(live_engine_snapshot(after, 20, 21, engines=engines), engines)
        for broken in (before.replace(b"20 10", b"20 11"),
                       before.replace(b"20 Z", b"20 S"),
                       before.replace(b"21 1 20 S", b"21 1 20 Z"),
                       before.replace(b"23 22 20 S", b"23 22 20 Z"),
                       before.replace(b"23 22 20", b"23 22 30"),
                       before + b"23 22 20 S\n", b"bad", b"\xff"):
            with self.subTest(broken=broken), self.assertRaises(PreflightError):
                live_engine_snapshot(broken, 20, 21, driver=10)
        with self.assertRaisesRegex(PreflightError, "engines_not_live"):
            live_engine_snapshot(after.replace(b"23 22", b"24 22"), 20, 21, engines=engines)

    def fixture(self, root):
        names = ["transport.active", "transport.marker", "ports.json"]
        names += ["00000000000000000000000000000001/" + name for name in (
            "profile.json", "api-binding.json", "native-ready.json", "native-init.json",
            "native-generation-started", "native-generation-complete", "api-ready",
            "native/config.xml", "native/cert.pem", "native/key.pem")]
        for name in names:
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"a" * 32 if name == "transport.active" else b"unchanged")

    def test_repeated_refusal_checks_live_engines_and_existing_key_without_mutating_files(self):
        with tempfile.TemporaryDirectory() as directory, patch('os.geteuid', return_value=Path(directory).stat().st_uid, create=True):
            root = Path(directory)
            self.fixture(root)
            calls, live = [], []
            def run(args, stage, timeout):
                calls.append(args[1])
                return ("mac_transport_combined_credential_continuity=passed" if "integrity" in args[1]
                        else "mac_transport_unresolved_restart_refused=passed")
            before = profile_snapshot(root)
            self.assertEqual(verify_exclusion(Path("daemon"), root, run, lambda: live.append(True), before), before)
            self.assertEqual(calls.count('--m8-local-transport-blocked-check'), 2)
            self.assertEqual(calls.count('--m8-local-transport-combined-integrity-check'), 3)
            self.assertEqual(len(live), 4)
            self.assertEqual(profile_snapshot(root), before)

    def test_refusal_message_does_not_hide_profile_or_marker_mutation_or_missing_key(self):
        for fault in ('marker', 'profile', 'key'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory() as directory, patch('os.geteuid', return_value=Path(directory).stat().st_uid, create=True):
                root = Path(directory)
                self.fixture(root)
                def run(args, stage, timeout):
                    if 'integrity' in args[1]:
                        if fault == 'key': return ''
                        return 'mac_transport_combined_credential_continuity=passed'
                    leaf = 'transport.active' if fault == 'marker' else '00000000000000000000000000000001/api-binding.json'
                    (root / leaf).write_bytes(b'changed')
                    return 'mac_transport_unresolved_restart_refused=passed'
                before = profile_snapshot(root)
                with self.assertRaises(PreflightError):
                    verify_exclusion(Path('daemon'), root, run, lambda: None, before)

    def test_mutation_during_loss_cannot_become_the_accepted_baseline(self):
        with tempfile.TemporaryDirectory() as directory, patch('os.geteuid', return_value=Path(directory).stat().st_uid, create=True):
            root = Path(directory)
            self.fixture(root)
            before = profile_snapshot(root)
            (root / 'transport.active').write_bytes(b'b' * 32)
            with self.assertRaisesRegex(PreflightError, 'transition_mutated_profile'):
                verify_exclusion(Path('daemon'), root,
                    lambda *_: self.fail('no native command before checking preloss continuity'),
                    lambda: None, before)

    def test_guardian_metadata_is_bounded_and_distinct(self):
        with tempfile.TemporaryDirectory() as directory, patch('os.geteuid', return_value=Path(directory).stat().st_uid, create=True):
            root = Path(directory)
            for data in (b'20 21', b'20 20', b'1 21', b'20 21\n', b'wrong', b'9'*40):
                (root/'guardian.ready').write_bytes(data)
                if data == b'20 21':
                    self.assertEqual(guardian_identity(root), (20,21))
                else:
                    with self.assertRaises(PreflightError): guardian_identity(root)


if __name__ == '__main__':
    unittest.main()
