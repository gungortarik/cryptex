import json
from pathlib import Path
import tempfile
import unittest

from mac_identity_preflight import PreflightError
from mac_transport_check import IDENTITY_MARKERS, MARKERS, check_identity, check_native_pairing, loss_group_snapshot


class IdentityCleanupTests(unittest.TestCase):
    def test_unresolved_driver_wait_does_not_start_any_cleanup_writer(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            metadata = root / "00000000000000000000000000000001/profile.json"
            metadata.parent.mkdir()
            metadata.write_text(json.dumps({"credential_id": "d" * 32}))
            calls = []
            def run(args, stage, timeout):
                calls.append(args)
                raise TimeoutError("driver exit unverified")
            with self.assertRaisesRegex(PreflightError, "cleanup_failed"):
                check_identity(Path("driver"), root, run)
            self.assertEqual(len(calls), 1)
    def test_timeout_or_crash_after_key_creation_still_deletes_and_verifies_exact_key(self):
        for diagnostic in ("bounded_timeout", "driver_crash"):
            with self.subTest(diagnostic=diagnostic), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                metadata = root / "00000000000000000000000000000001/profile.json"
                calls = []
                def run(args, stage, timeout):
                    calls.append(args)
                    if args[1] == "--m8-local-transport-driver":
                        metadata.parent.mkdir()
                        metadata.write_text(json.dumps({"credential_id": "a" * 32}))
                        raise PreflightError(diagnostic)
                    return "mac_transport_fixture_key_cleanup=passed"
                with self.assertRaisesRegex(PreflightError, diagnostic):
                    check_identity(Path("signed-driver"), root, run)
                self.assertEqual([call[1] for call in calls], ["--m8-local-transport-driver", "--m8-local-transport-fixture-key-cleanup"])
                self.assertTrue(all(call[0] == Path("signed-driver") and call[-1] == root for call in calls))
                self.assertTrue(metadata.exists())  # failure evidence is retained

    def test_already_deleted_key_is_verified_and_unsafe_label_never_reaches_security(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            metadata = root / "00000000000000000000000000000001/profile.json"
            metadata.parent.mkdir()
            metadata.write_text(json.dumps({"credential_id": "b" * 32}))
            calls = []
            def run(args, stage, timeout):
                calls.append(args)
                return "mac_transport_fixture_key_cleanup=passed" if "cleanup" in args[1] else "\n".join(IDENTITY_MARKERS)
            check_identity(Path("driver"), root, run)
            self.assertEqual(len(calls), 2)
            metadata.write_text(json.dumps({"credential_id": "unrelated-account"}))
            calls.clear()
            with self.assertRaisesRegex(PreflightError, "cleanup_failed"):
                check_identity(Path("driver"), root, run)
            self.assertEqual(len(calls), 1)
            self.assertEqual(calls[0][1], "--m8-local-transport-driver")

    def test_unresolved_writer_fence_never_falls_back_to_raw_security_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            metadata = root / "00000000000000000000000000000001/profile.json"
            metadata.parent.mkdir()
            metadata.write_text(json.dumps({"credential_id": "c" * 32}))
            calls = []
            def run(args, stage, timeout):
                calls.append(args)
                if "cleanup" in args[1]:
                    raise PreflightError("writer_lock_timeout")
                return "\n".join(IDENTITY_MARKERS)
            with self.assertRaisesRegex(PreflightError, "cleanup_failed"):
                check_identity(Path("driver"), root, run)
            self.assertEqual(len(calls), 2)
            self.assertTrue(all(call[0] == Path("driver") for call in calls))


class PairingCleanupTests(unittest.TestCase):
    def test_loss_evidence_requires_the_exact_driver_and_private_group(self):
        valid = b"20 10 20 S\n21 20 20 S\n22 21 20 S\n"
        loss_group_snapshot(valid, 10, 20)
        for data in (valid.replace(b"20 10", b"20 11"), valid.replace(b"22 21 20", b"22 21 30"),
                     b"20 10 20 S\n", b"20 10 20 Z\n21 20 20 S\n22 21 20 S\n"):
            with self.assertRaises(PreflightError):
                loss_group_snapshot(data, 10, 20)
    def fixture(self, root):
        for index, role in enumerate(("initiator", "responder"), 1):
            identity = root / role / "identity/local_device.json"
            identity.parent.mkdir(parents=True)
            identity.write_text(json.dumps({"device_id": f"00000000-0000-4000-8000-{index:012d}"}), encoding="utf-8")
            metadata = root / role / "managed-transport/00000000000000000000000000000001/profile.json"
            metadata.parent.mkdir(parents=True)
            metadata.write_text(json.dumps({"credential_id": str(index) * 32}), encoding="utf-8")

    def test_success_cleans_only_both_exact_fixture_keys(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            calls = []

            def run(args, stage, timeout=30):
                calls.append(args)
                return "\n".join(MARKERS)

            check_native_pairing(Path("signed-desktop"), root, run)
            self.assertEqual(len(calls), 5)
            self.assertEqual([call[-1] for call in calls[1:]], [
                "device-event-signing-seed:00000000-0000-4000-8000-000000000001", "api:" + "1" * 32,
                "device-event-signing-seed:00000000-0000-4000-8000-000000000002", "api:" + "2" * 32])

    def test_missing_marker_still_cleans_all_created_keys(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            calls = []

            def run(args, stage, timeout=30):
                calls.append(args)
                return ""

            with self.assertRaisesRegex(PreflightError, "marker_missing"):
                check_native_pairing(Path("signed-desktop"), root, run)
            self.assertEqual(len(calls), 5)

    def test_untrusted_cleanup_label_never_reaches_keychain(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            metadata = root / "initiator/managed-transport/00000000000000000000000000000001/profile.json"
            metadata.write_text(json.dumps({"credential_id": "other-account"}), encoding="utf-8")
            calls = []

            def run(args, stage, timeout=30):
                calls.append(args)
                return "\n".join(MARKERS)

            with self.assertRaisesRegex(PreflightError, "cleanup_failed"):
                check_native_pairing(Path("signed-desktop"), root, run)
            self.assertFalse(any("api:other-account" in call for call in calls))
            self.assertTrue(any("api:" + "2" * 32 in call for call in calls))


if __name__ == "__main__":
    unittest.main()
