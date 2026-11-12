import json
from pathlib import Path
import tempfile
import unittest

from mac_identity_preflight import PreflightError
from mac_transport_check import MARKERS, check_native_pairing, loss_group_snapshot


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
