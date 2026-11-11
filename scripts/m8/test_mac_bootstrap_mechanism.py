import copy
import json
from pathlib import Path
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch

import mac_bootstrap_mechanism as probe
from mac_identity_preflight import PreflightError


class BootstrapMechanismTests(unittest.TestCase):
    @staticmethod
    def rows():
        return [{"case": name, "observed": observed, "stage": stage, "code": codes[0], "pass": True}
                for name, (observed, stage, codes) in probe.EXPECTED.items()]

    @staticmethod
    def parse(rows):
        return probe.parse_native("\n".join(json.dumps(row) for row in rows))

    def test_missing_cases_or_nonzero_native_exit_never_complete(self):
        rows = self.parse(self.rows())
        self.assertTrue(probe.complete(rows, 0))
        for code in (True, 2, -9):
            self.assertFalse(probe.complete(rows, code))
        self.assertFalse(probe.complete(self.parse(self.rows()[:-1]), 0))

    def test_actual_stage_and_code_must_match_each_claimed_pass(self):
        for field, replacement in (("stage", "child_signal"), ("code", 9), ("observed", "unexpected")):
            for index in range(len(self.rows())):
                rows = self.rows()
                rows[index][field] = replacement
                with self.assertRaises(PreflightError):
                    self.parse(rows)

    def test_spawn_failure_preserved_in_partial_inventory(self):
        rows = [{"case": "two_way_after_imposter", "observed": "spawn_error", "stage": "spawn", "code": 9, "pass": False}]
        parsed = self.parse(rows)
        self.assertEqual(parsed, rows)
        self.assertFalse(probe.complete(parsed, 2))

    def test_registration_refusal_preserves_actual_bootstrap_status(self):
        rows = [{"case": "fresh_child_session", "observed": "bootstrap_error", "stage": "check_in", "code": 1100, "pass": False}]
        self.assertEqual(self.parse(rows)[0]["code"], 1100)
        self.assertFalse(probe.complete(self.parse(rows), 0))

    def test_unknown_duplicate_privacy_fields_and_non_booleans_refuse(self):
        for key, value in (("audit_token", "forbidden"), ("pid", 123), ("pass", 1), ("code", True), ("code", -9), ("case", "unknown")):
            rows = self.rows()
            rows[0][key] = value
            with self.assertRaises(PreflightError):
                self.parse(rows)
        with self.assertRaises(PreflightError):
            self.parse(self.rows() + [self.rows()[0]])
        with self.assertRaises(PreflightError):
            probe.parse_native('{"case":"one","case":"two"}')
        with self.assertRaises(PreflightError):
            probe.parse_native("x" * 65537)
        with self.assertRaises(PreflightError):
            probe.parse_native("")

    def run_collect(self, native_code=0, native_rows=None, forced_code=-9, cleanup_code=0,
                    native_timeout=False, cleanup_timeout=False, control_failure=None, cleanup_fault_code=68,
                    control_output=""):
        with tempfile.TemporaryDirectory() as directory:
            def run(args, _timeout=20):
                output, code, timeout = "", 0, False
                if "--show-sdk-path" in args:
                    output = "/public/sdk"
                elif "clang" in args:
                    self.assertIn("-Werror", args)
                    self.assertNotIn("-Wno-deprecated-declarations", args)
                    Path(args[-1]).write_bytes(b"disposable fixture")
                elif "--sign" in args:
                    self.assertEqual(args[args.index("--sign") + 1], "-")
                elif len(args) > 1 and args[1] == "probe":
                    output = "\n".join(json.dumps(row) for row in (self.rows() if native_rows is None else native_rows))
                    code, timeout = native_code, native_timeout
                elif len(args) > 1 and args[1] == "forced-exit":
                    code = forced_code
                elif len(args) > 1 and args[1] == "lookup-missing":
                    code, timeout = cleanup_code, cleanup_timeout
                elif len(args) > 1 and args[1] in {"reaper-controls", "cleanup-control", "send-timeout-copy", "send-error-once-header"}:
                    code = {"reaper-controls": 0, "cleanup-control": cleanup_fault_code, "send-timeout-copy": 67, "send-error-once-header": 59}[args[1]]
                    if args[1] == control_failure:
                        code, output = 1, control_output
                return SimpleNamespace(code=code, output=output, limited=False, timed_out=timeout, pipe_failed=False)
            # The collector only executes on Darwin; this mock runner also runs
            # on Windows, which does not define the POSIX signal constant.
            with patch.object(probe.signal, "SIGKILL", 9, create=True):
                return probe.collect(Path(directory), run)

    def test_ci_mechanism_pass_never_certifies_product_or_signed_bundle(self):
        report = self.run_collect()
        self.assertTrue(report["phase_one_complete"])
        for key in ("certified_bundle_acceptance", "production_changed", "owner_session_acceptance", "m8_complete"):
            self.assertIs(report[key], False)
        self.assertIs(report["ci_only"], True)
        self.assertEqual(report["owner_package"], "not_produced")
        self.assertTrue(report["forced_exit_namespace_removed"])

    def test_failed_native_exit_and_failures_never_retain_completion(self):
        failed = copy.deepcopy(self.rows())
        failed[0]["pass"] = False
        for kwargs in ({"native_code": 2}, {"native_rows": failed}, {"native_rows": self.rows()[:-1]},
                       {"forced_code": 73}, {"cleanup_code": 66}, {"native_timeout": True}, {"cleanup_timeout": True}):
            report = self.run_collect(**kwargs)
            self.assertFalse(report["phase_one_complete"])
            self.assertEqual(report["outcome"], "mechanism_incomplete")

    def test_partial_typed_failure_survives_collector_rejection(self):
        failure = {"case": "two_way_after_imposter", "observed": "bootstrap_error", "stage": "check_in", "code": 1100, "pass": False}
        report = self.run_collect(native_code=2, native_rows=[failure])
        self.assertEqual(report["cases"], [failure])
        self.assertFalse(report["phase_one_complete"])

    def test_each_native_cleanup_fault_control_is_required(self):
        for mode in ("reaper-controls", "cleanup-control", "send-timeout-copy", "send-error-once-header"):
            report = self.run_collect(control_failure=mode)
            self.assertFalse(report["phase_one_complete"])
            self.assertEqual(report["diagnostic"], "native_fault_control_failed")
            self.assertEqual(report["fault_controls"][mode], {"stage": "child_exit", "code": 1})

    def test_cleanup_setup_failure_cannot_impersonate_observed_wrong_right_refusal(self):
        report = self.run_collect(cleanup_fault_code=62)
        self.assertFalse(report["phase_one_complete"])
        self.assertEqual(report["diagnostic"], "native_fault_control_failed")
        self.assertEqual(report["fault_controls"]["cleanup-control"]["code"], 62)

    def test_control_actual_stage_is_preserved_without_exporting_extra_data(self):
        diagnostic = {"stage": "extract_right", "code": 17}
        report = self.run_collect(control_failure="send-error-once-header", control_output=json.dumps(diagnostic))
        self.assertFalse(report["phase_one_complete"])
        self.assertEqual(report["fault_controls"]["send-error-once-header"]["diagnostic"], diagnostic)
        for value in ({**diagnostic, "token": "forbidden"}, {"stage": "unknown", "code": 1}, {"stage": "extract_right", "code": True}):
            with self.assertRaises(PreflightError):
                probe.control_diagnostic(json.dumps(value))


if __name__ == "__main__":
    unittest.main()
