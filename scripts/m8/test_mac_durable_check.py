import json
from pathlib import Path
import tempfile
import unittest

from mac_durable_check import MARKERS, check_durable_profile
from mac_identity_preflight import CommandResult, PreflightError


class DurableCollectorTests(unittest.TestCase):
    def exercise(self, fault=None):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "runtime"
            deleted = []

            def runner(args, timeout):
                if args[0] == "/usr/bin/security":
                    self.assertEqual(args[1:5], ["delete-generic-password", "-s",
                                                "Cryptex-Dev-DeviceCredentials", "-a"])
                    self.assertEqual(args[5], "device-event-signing-seed:ddfa4e7c-d69d-4b53-89f6-2aa790588b5d")
                    deleted.append(True)
                    code = 44 if len(deleted) == 2 else 0
                    if fault == "cleanup" or (fault == "recreated" and len(deleted) == 2):
                        code = 1 if fault == "cleanup" else 0
                    return CommandResult(code, "")
                if args[1].endswith("durable-check"):
                    for folder in ("identity", "canonical", "bootstrap"):
                        (root / folder).mkdir(parents=True)
                    (root / "identity/local_device.json").write_text(
                        '{"device_id":"ddfa4e7c-d69d-4b53-89f6-2aa790588b5d"}')
                    (root / "canonical/synthetic").write_bytes(b"original")
                    states = ["owner_pending", "cancelled", "owner_pending", "anchored"]
                    rows = [{"state": state, "append_attempts": [1] if state == "anchored" else []}
                            for state in states]
                    if fault == "duplicate":
                        rows[-1]["append_attempts"].append(2)
                    (root / "bootstrap/pending.json").write_text(json.dumps({"transactions": rows}))
                    (root / "ui-receipt.json").write_text(
                        json.dumps({"eventCount": 2, "prefixPreserved": fault != "prefix"}))
                    markers = MARKERS[:-1] if fault == "marker" else MARKERS
                    return CommandResult(0, "\n".join(markers))
                if fault == "mutation":
                    (root / "canonical/synthetic").write_bytes(b"changed")
                output = "native_keychain_seed_missing"
                if fault == "early":
                    output = "cryptexd: peer approval recovery conflict: canonical history verification failed: secure_store_missing_item"
                if fault == "wrong_error":
                    output = "local_signed_startup_Instance"
                return CommandResult(2, output, timed_out=fault == "timeout")

            report = {}
            if fault in (None, "early"):
                check_durable_profile(Path("/fixture/desktop"), root, report, runner)
                self.assertEqual(report, {"credential_cleanup_verified": True,
                                          "restart_and_confirmation_verified": True,
                                          "missing_key_refusal_verified": True})
            else:
                with self.assertRaises(PreflightError):
                    check_durable_profile(Path("/fixture/desktop"), root, report, runner)
                self.assertNotIn("missing_key_refusal_verified", report)
            self.assertTrue(deleted)

    def test_success_and_earlier_recovery_refusal(self):
        for fault in (None, "early"):
            with self.subTest(fault=fault):
                self.exercise(fault)

    def test_failures_are_retained_and_cleanup_is_attempted(self):
        for fault in ("cleanup", "recreated", "duplicate", "prefix", "marker",
                      "mutation", "wrong_error", "timeout"):
            with self.subTest(fault=fault):
                self.exercise(fault)
