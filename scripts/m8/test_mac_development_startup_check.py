import sys
from pathlib import Path
from types import SimpleNamespace
import unittest
import tempfile
import io
from contextlib import redirect_stdout
import json

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from mac_development_startup_check import require_refusal, canonical_hashes, exercise_profile, FencedRun
from mac_identity_preflight import PreflightError


class RefusalTests(unittest.TestCase):
    def test_only_clean_exact_refusal_is_accepted(self):
        base = dict(code=2, timed_out=False, limited=False, pipe_failed=False,
                    output="local_signed_startup_Inventory\n")
        require_refusal(SimpleNamespace(**base), "local_signed_startup_Inventory")
        for change in (dict(code=0), dict(code=1), dict(timed_out=True), dict(limited=True),
                       dict(pipe_failed=True), dict(output="prefix local_signed_startup_Inventory\n"),
                       dict(output="some_other_error\n")):
            with self.subTest(change=change), self.assertRaises(PreflightError):
                require_refusal(SimpleNamespace(**(base | change)), "local_signed_startup_Inventory")


class DiagnosticTests(unittest.TestCase):
    def test_failure_output_only_contains_allowlisted_stages(self):
        output = "\n".join([
            "private note and runtime path", "local_signed_startup_Channel", "local_signed_startup_private",
            json.dumps({"developmentStage": "private"}),
            json.dumps({"m8ProfileStage": "prepare", "kind": "private"}),
            json.dumps({"m8LocalStage": "content_bind", "error": "local_signed_startup_Instance", "secret": "private"}),
            json.dumps({"m8ProfileStage": "prepare", "kind": "Other", "path": "private"}),
            json.dumps({"developmentStage": "unix_health", "path": "private"}),
            json.dumps({"m8LocalStage": "private path", "error": "private key"})])
        result = SimpleNamespace(code=2, timed_out=False, limited=False, pipe_failed=False, output=output)
        fence = FencedRun(lambda *_: result)
        captured = io.StringIO()
        with redirect_stdout(captured), self.assertRaises(PreflightError):
            fence.checked(["desktop"], "development_check", 10)
        diagnostic = captured.getvalue()
        self.assertNotIn("private", diagnostic)
        self.assertIn("unix_health", diagnostic)
        self.assertIn("prepare", diagnostic)
        self.assertIn("local_signed_startup_Instance", diagnostic)
        self.assertTrue(fence.stopped)


class ProfileTests(unittest.TestCase):
    def test_hashes_include_active_tails_and_new_records(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            records = root / "canonical/events/device"
            records.mkdir(parents=True)
            (records / "one.seg").write_bytes(b"sealed")
            active = records / "two.open"
            active.write_bytes(b"active")
            before = canonical_hashes(root)
            self.assertEqual(len(before), 2)
            active.write_bytes(b"changed")
            self.assertNotEqual(before, canonical_hashes(root))
            active.write_bytes(b"active")
            (records / "extra.seg").write_bytes(b"new")
            self.assertNotEqual(before, canonical_hashes(root))

    def test_failure_cleanup_requires_reaped_writer_and_keeps_diagnostics(self):
        for failure in ("exit", "timeout", "marker", "raw", "active", "success"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                manifest = root / "inventory.json"
                manifest.write_bytes(b"{}")
                calls = []
                device = "11111111-1111-4111-8111-111111111111"
                def bounded(args, timeout):
                    profile = root / "development-pilot-runtime"
                    result = dict(code=0, timed_out=False, limited=False, pipe_failed=False,
                                  output="development_existing_profile_startup_and_cleanup=passed\n")
                    if args[0] == "fixture":
                        (profile / "identity").mkdir()
                        (profile / "identity/local_device.json").write_text(json.dumps({"device_id": device}))
                        records = profile / "canonical/events/device"
                        records.mkdir(parents=True)
                        (records / "one.seg").write_bytes(b"sealed")
                        (records / "two.open").write_bytes(b"active")
                    elif manifest.read_bytes() != b"{}":
                        result.update(code=2, output="local_signed_startup_Inventory\n")
                    elif failure == "raw":
                        raise OSError("unresolved group")
                    elif failure == "exit":
                        result["code"] = 2
                    elif failure == "timeout":
                        result["timed_out"] = True
                    elif failure == "marker":
                        result["output"] = ""
                    elif failure == "active":
                        (profile / "canonical/events/device/two.open").write_bytes(b"changed")
                    return SimpleNamespace(**result)
                def cleanup(args, label, timeout):
                    calls.append(args)
                if failure == "success":
                    exercise_profile(root, "desktop", "fixture", manifest, bounded, cleanup)
                else:
                    with self.assertRaises(PreflightError):
                        exercise_profile(root, "desktop", "fixture", manifest, bounded, cleanup)
                self.assertEqual(len(calls), 0 if failure == "raw" else 1)
                if calls:
                    self.assertEqual(calls[0][-1], "device-event-signing-seed:" + device)
                self.assertEqual((root / "development-pilot-runtime").exists(), failure != "success")
                self.assertEqual(manifest.read_bytes(), b"{}")


if __name__ == "__main__":
    unittest.main()
