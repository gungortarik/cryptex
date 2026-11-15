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
from mac_transport_check import check_native_pairing
from mac_transport_owner import main, make_runner, summarize_output, summarize_validation


class OwnerDiagnosticsTests(unittest.TestCase):
    def fixture_probe(self, directory):
        root = Path(directory) / "pairing"
        metadata = root / "initiator/identity/local_device.json"
        metadata.parent.mkdir(parents=True)
        metadata.write_text(json.dumps({"device_id": "24376f10-1d44-4673-960a-cb46ff26ab65"}), encoding="utf-8")
        binary = Path(directory) / "positive.app/Contents/MacOS/cryptex-desktop"
        return root, binary

    def test_validation_reason_filter_never_exports_raw_error_values(self):
        secret = "private-secret-body-path"
        output = ("import staging rejected: retrieval safety gate is held: "
                  "Some(MaterializationStateMissingOrStale) " + secret +
                  " (os error 13) (os error 999) (os error 1000)")
        self.assertEqual(summarize_validation(output), {
            "reason_codes": ["materialization_missing_or_stale", "retrieval_safety_held", "staging_invalid"],
            "os_error_numbers": [13]})
        self.assertNotIn(secret, json.dumps(summarize_validation(output)))

    def test_rejected_fixture_gets_one_validation_before_original_failure_and_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            root, binary = self.fixture_probe(directory)
            for probe_result in (CommandResult(0, '{"outcome":"validated_only","published":false}'),
                                 CommandResult(1, "import staging rejected: event brain_id does not match the selected Brain")):
                report = {}
                run = make_runner(report, root)
                with self.subTest(probe=probe_result), patch("mac_transport_owner.run_bounded", side_effect=(
                        CommandResult(2, "existing_profile_incoming_Rejected"), probe_result, CommandResult(0, ""))) as bounded:
                    with self.assertRaisesRegex(PreflightError, "real_native_managed_pairing_failed"):
                        check_native_pairing(binary, root, run)
                self.assertEqual(bounded.call_count, 3)
                self.assertEqual(bounded.call_args_list[1].args[0], [
                    str(binary.with_name("cryptexd")), "replication", "validate-incoming", "--runtime-root",
                    str(root / "responder"), "--environment", "dev",
                    "24376f10-1d44-4673-960a-cb46ff26ab65".encode().hex() + "/00000000000000000000.seg"])
                self.assertLessEqual(bounded.call_args_list[1].args[1], 45)
                self.assertEqual(bounded.call_args_list[2].args[0][0:2], ["/usr/bin/security", "delete-generic-password"])
                self.assertEqual(report["post_rejection_validation"]["exit_code"], probe_result.code)
                self.assertTrue(report["post_rejection_validation"]["validation_only"])
                self.assertNotIn("native_pairing_and_reopen_verified", report)

    def test_unresolved_validation_writer_blocks_fixture_key_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            root, binary = self.fixture_probe(directory)
            report = {}
            with patch("mac_transport_owner.run_bounded", side_effect=(
                    CommandResult(2, "existing_profile_incoming_Rejected"), TimeoutError("unresolved-private"))) as bounded:
                with self.assertRaisesRegex(PreflightError, "native_pairing_cleanup_writer_unresolved"):
                    check_native_pairing(binary, root, make_runner(report, root))
            self.assertEqual(bounded.call_count, 2)
            self.assertNotIn("unresolved-private", json.dumps(report))
            self.assertEqual(report["post_rejection_validation"]["outcome"], "writer_exit_unresolved")

    def test_validation_only_for_exact_rejection_and_current_fixture(self):
        with tempfile.TemporaryDirectory() as directory:
            root, binary = self.fixture_probe(directory)
            for result, selector, selected in (
                    (CommandResult(2, "existing_profile_incoming_Rejected", timed_out=True), "--cryptex-m8-local-content-transport-check", root),
                    (CommandResult(2, "existing_profile_incoming_Rejected", limited=True), "--cryptex-m8-local-content-transport-check", root),
                    (CommandResult(2, "existing_profile_incoming_Rejected", pipe_failed=True), "--cryptex-m8-local-content-transport-check", root),
                    (CommandResult(2, "existing_profile_incoming_Held"), "--cryptex-m8-local-content-transport-check", root),
                    (CommandResult(2, "existing_profile_incoming_Rejected"), "other", root),
                    (CommandResult(2, "existing_profile_incoming_Rejected"), "--cryptex-m8-local-content-transport-check", root.parent / "older")):
                with self.subTest(result=result, selector=selector, selected=selected), \
                        patch("mac_transport_owner.run_bounded", return_value=result) as bounded:
                    report = {}
                    with self.assertRaises(PreflightError):
                        make_runner(report, root)([binary, selector, selected], "real_native_managed_pairing", 240)
                    self.assertEqual(bounded.call_count, 1)
                    self.assertNotIn("post_rejection_validation", report)

    def test_validation_does_not_extend_original_pairing_budget(self):
        with tempfile.TemporaryDirectory() as directory:
            root, binary = self.fixture_probe(directory)
            report = {}
            with patch("mac_transport_owner.run_bounded", return_value=CommandResult(2, "existing_profile_incoming_Rejected")) as bounded, \
                    patch("mac_transport_owner.time.monotonic", side_effect=(0, 240, 241)):
                with self.assertRaises(PreflightError):
                    make_runner(report, root)([binary, "--cryptex-m8-local-content-transport-check", root], "real_native_managed_pairing", 240)
            bounded.assert_called_once()
            self.assertEqual(report["post_rejection_validation"]["outcome"], "original_budget_exhausted")

    def test_validation_probe_is_not_retried(self):
        with tempfile.TemporaryDirectory() as directory:
            root, binary = self.fixture_probe(directory)
            report = {}
            run = make_runner(report, root)
            with patch("mac_transport_owner.run_bounded", side_effect=(
                    CommandResult(2, "existing_profile_incoming_Rejected"), CommandResult(0, ""),
                    CommandResult(2, "existing_profile_incoming_Rejected"))) as bounded:
                for _ in range(2):
                    with self.assertRaises(PreflightError):
                        run([binary, "--cryptex-m8-local-content-transport-check", root], "real_native_managed_pairing", 240)
            self.assertEqual(bounded.call_count, 3)

    def test_unsafe_fixture_metadata_does_not_launch_validation(self):
        with tempfile.TemporaryDirectory() as directory:
            root, binary = self.fixture_probe(directory)
            report = {}
            with patch("mac_transport_owner._metadata", side_effect=PreflightError("unsafe metadata")), \
                    patch("mac_transport_owner.run_bounded", return_value=CommandResult(2, "existing_profile_incoming_Rejected")) as bounded:
                with self.assertRaisesRegex(PreflightError, "real_native_managed_pairing_failed"):
                    make_runner(report, root)([binary, "--cryptex-m8-local-content-transport-check", root], "real_native_managed_pairing", 240)
            bounded.assert_called_once()
            self.assertEqual(report["post_rejection_validation"]["outcome"], "fixture_metadata_refused")

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
