"""Owner diagnostics preserve failures without exporting process output."""
import json
import hashlib
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from mac_identity_preflight import CommandResult, PreflightError
from mac_transport_owner import main, make_runner, summarize_output


class OwnerDiagnosticsTests(unittest.TestCase):
    def test_only_closed_codes_markers_and_startup_fields_leave_collector(self):
        secret = "private-body-and-credential-123"
        output = "\n".join((
            secret, "/Users/private/" + secret,
            "existing_profile_incoming_Rejected",
            "native_existing_profile_signed_import=passed",
            json.dumps({"m8LocalStage": "native_accept", "error": "local_signed_startup_Channel",
                        "elapsedMs": 17, "credential": secret}),
            json.dumps({"m8LocalStage": secret, "error": "local_signed_startup_Image", "elapsedMs": 12}),
            json.dumps({"m8LocalStage": "native_accept", "error": secret, "elapsedMs": 12}),
            json.dumps({"m8LocalStage": "native_accept", "error": "local_signed_startup_Image", "elapsedMs": True}),
        ))
        receipt = summarize_output(output)
        self.assertNotIn(secret, json.dumps(receipt))
        self.assertEqual(receipt["markers"], ["native_existing_profile_signed_import=passed"])
        self.assertEqual(receipt["startup_failures"], [
            {"stage": "native_accept", "error": "local_signed_startup_Channel", "elapsed_ms": 17}])
        self.assertIn("existing_profile_incoming_Rejected", receipt["codes"])

    def test_malformed_nested_and_non_scalar_json_is_bounded(self):
        lines = ["{", "{" + "[" * 2000, "[1]", json.dumps({
            "m8LocalStage": [], "error": {}, "elapsedMs": 0})]
        valid = json.dumps({"m8LocalStage": "runtime_ready", "error": "local_signed_startup_Readiness",
                            "elapsedMs": 1})
        receipt = summarize_output("\n".join(lines + [valid] * 100))
        self.assertEqual(len(receipt["startup_failures"]), 16)
        self.assertEqual(summarize_output("x" * 65536 + "\nexisting_profile_live_pairing")["codes"], [])

    def test_each_failure_flag_retains_exact_cause_and_original_budget(self):
        for result in (CommandResult(2, "existing_profile_live_pairing"),
                       CommandResult(-9, "", timed_out=True),
                       CommandResult(-9, "", limited=True),
                       CommandResult(0, "", pipe_failed=True)):
            with self.subTest(result=result), patch("mac_transport_owner.run_bounded", return_value=result) as bounded:
                report = {}
                run = make_runner(report)
                with self.assertRaisesRegex(PreflightError, "real_native_managed_pairing_failed"):
                    run(["signed-binary", "private-path"], "real_native_managed_pairing", 240)
                bounded.assert_called_once_with(["signed-binary", "private-path"], 240)
                receipt = report["command_diagnostics"][0]
                self.assertEqual(receipt["exit_code"], result.code)
                self.assertEqual(receipt["timed_out"], result.timed_out)
                self.assertEqual(receipt["output_limited"], result.limited)
                self.assertEqual(receipt["pipe_failed"], result.pipe_failed)
                self.assertEqual(receipt["timeout_seconds"], 240)
                self.assertNotIn("private-path", json.dumps(report))

    def test_successful_cleanup_does_not_overwrite_prior_failure(self):
        report = {}
        run = make_runner(report)
        with patch("mac_transport_owner.run_bounded", side_effect=(
                CommandResult(2, "existing_profile_incoming_Held"), CommandResult(0, "private-secret"))):
            with self.assertRaises(PreflightError):
                run(["binary"], "real_native_managed_pairing", 240)
            self.assertEqual(run(["cleanup"], "cleanup_native_pairing_device_key"), "private-secret")
        self.assertEqual([row["exit_code"] for row in report["command_diagnostics"]], [2, 0])
        self.assertEqual(report["command_diagnostics"][0]["codes"], ["existing_profile_incoming_Held"])
        self.assertNotIn("private-secret", json.dumps(report))

    def test_unresolved_process_error_remains_untyped_for_cleanup_fence(self):
        report = {}
        with patch("mac_transport_owner.run_bounded", side_effect=TimeoutError("unresolved private path")):
            with self.assertRaises(TimeoutError):
                make_runner(report)(["binary"], "real_native_managed_pairing", 240)
        self.assertEqual(report["command_diagnostics"], [])
        self.assertNotIn("unresolved private path", json.dumps(report))

    def test_receipt_count_is_bounded(self):
        report = {}
        run = make_runner(report)
        with patch("mac_transport_owner.run_bounded", return_value=CommandResult(0, "")):
            for _ in range(70):
                run(["binary"], "cleanup_native_pairing_device_key")
        self.assertEqual(len(report["command_diagnostics"]), 64)
        self.assertTrue(report["command_diagnostics_truncated"])

    def test_one_click_failure_archive_has_cause_and_collector_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            package = Path(directory)
            launcher = package / "MAC-AKTARIM.command"
            launcher.write_bytes(b"collector fixture")
            (package / "fixture.json").write_text(json.dumps({
                "source": "a" * 40, "architecture": "arm64",
                "expires_utc": "2099-01-01T00:00:00+00:00"}), encoding="utf-8")
            runtime = package / "private-runtime"
            runtime.mkdir()

            def failed_pairing(binary, root, run):
                run([binary, root], "real_native_managed_pairing", 240)

            with patch("mac_transport_owner.__file__", str(launcher)), \
                    patch("mac_transport_owner.sys.stdout", new_callable=io.StringIO), \
                    patch("mac_transport_owner.os.umask"), \
                    patch("mac_transport_owner.sys.platform", "darwin"), \
                    patch("mac_transport_owner.platform.machine", return_value="arm64"), \
                    patch("mac_transport_owner.tempfile.mkdtemp", return_value=str(runtime)), \
                    patch("mac_transport_owner.check_identity"), \
                    patch("mac_transport_owner.check_native_pairing", side_effect=failed_pairing), \
                    patch("mac_transport_owner.check_parent_loss") as loss, \
                    patch("mac_transport_owner.run_bounded", return_value=CommandResult(2, "existing_profile_incoming_Rejected")):
                self.assertEqual(main(), 1)
                loss.assert_not_called()
            with zipfile.ZipFile(next(package.glob("MAC-AKTARIM-SONUC-*.zip"))) as archive:
                self.assertEqual(archive.namelist(), ["result.json"])
                report = json.loads(archive.read("result.json"))
            self.assertEqual(report["outcome"], "failed")
            self.assertTrue(report["identity_restart_and_refusals_verified"])
            self.assertNotIn("native_pairing_and_reopen_verified", report)
            self.assertEqual(report["collector_sha256"], hashlib.sha256(launcher.read_bytes()).hexdigest())
            self.assertEqual(report["command_diagnostics"][0]["codes"], ["existing_profile_incoming_Rejected"])
            self.assertTrue(runtime.is_dir())


if __name__ == "__main__":
    unittest.main()
