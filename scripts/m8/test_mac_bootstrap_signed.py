import copy
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace
import zipfile

import mac_bootstrap_mechanism as mechanism
import mac_bootstrap_signed as probe
from mac_identity_preflight import PreflightError


class SignedBootstrapTests(unittest.TestCase):
    @staticmethod
    def rows(eligible=True):
        rows = [{"capability": "certified_two_binary_bundle", "eligible": eligible,
                 "diagnostic": "verified_self_organization_two_binary_bundle" if eligible else "apple_anchor_or_fixed_self_identifier_refused"}]
        if eligible:
            rows += [{"case": name, "observed": "code_requirement_refused", "stage": "static_code", "code": 55, "pass": True}
                     for name in sorted(probe.NEGATIVES)]
            rows += [{"case": name, "observed": observed, "stage": stage, "code": codes[0], "pass": True}
                     for name, (observed, stage, codes) in mechanism.EXPECTED.items()]
        return rows

    @staticmethod
    def parse(rows):
        return probe.parse_signed("\n".join(json.dumps(row) for row in rows))

    def test_full_signed_candidate_needs_both_signature_controls_and_all_mechanism_cases(self):
        native = self.parse(self.rows())
        self.assertTrue(probe.complete(native, 0, False))
        self.assertEqual(len(native["signature_cases"]) + len(native["cases"]), 19)
        self.assertFalse(probe.complete(native, 0, True))
        self.assertFalse(probe.complete(native, 2, False))
        self.assertFalse(probe.complete(self.parse(self.rows(False)), 0, False))
        self.assertFalse(probe.complete(self.parse(self.rows()[:-1]), 0, False))

    def test_certified_capability_is_literal_and_never_carries_identity_details(self):
        for key, value in (("eligible", 1), ("team_id", "forbidden"), ("diagnostic", "unknown"), ("diagnostic", [])):
            rows = self.rows(False)
            rows[0][key] = value
            with self.assertRaises(PreflightError):
                self.parse(rows)
        rows = self.rows(False) + [self.rows()[1]]
        with self.assertRaises(PreflightError):
            self.parse(rows)

    def test_duplicate_signature_control_and_setup_failure_cannot_pass(self):
        rows = self.rows()
        rows[2] = copy.deepcopy(rows[1])
        with self.assertRaises(PreflightError):
            self.parse(rows)
        for code, outcome in ((62, "code_requirement_refused"), (55, "unexpected")):
            rows = self.rows()
            rows[1].update(code=code, observed=outcome)
            with self.assertRaises(PreflightError):
                self.parse(rows)

    def test_false_signature_observation_is_preserved_and_never_complete(self):
        rows = self.rows()
        rows[1].update(code=62, observed="unexpected", **{"pass": False})
        native = self.parse(rows)
        self.assertFalse(native["signature_cases"][0]["pass"])
        self.assertFalse(probe.complete(native, 0, False))

    def test_truncated_native_prefix_is_preserved_but_never_complete(self):
        for count in (1, 2, 3):
            native = self.parse(self.rows()[:count])
            self.assertTrue(native["bundle"]["eligible"])
            self.assertEqual(len(native["signature_cases"]), count - 1)
            self.assertEqual(native["cases"], [])
            self.assertFalse(probe.complete(native, 0, False))
            report = self.collect(rows=self.rows()[:count], native_code=59)
            self.assertEqual(report["native"], native)
            self.assertFalse(report["candidate_complete"])

    def test_missing_duplicate_json_or_extra_privacy_fields_refuse(self):
        with self.assertRaises(PreflightError):
            probe.parse_signed('{"eligible":false,"eligible":true}')
        with self.assertRaises(PreflightError):
            probe.parse_signed("x" * 65537)
        rows = self.rows()
        rows[1]["certificate"] = "forbidden"
        with self.assertRaises(PreflightError):
            self.parse(rows)
        for key in ("case", "observed", "stage"):
            rows = self.rows()
            rows[1][key] = []
            with self.assertRaises(PreflightError):
                self.parse(rows)

    def collect(self, ci=False, rows=None, native_code=0, forced_code=-9, cleanup_code=0):
        report = {"candidate_complete": False, "production_changed": False, "m8_complete": False}
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            def run(args, _timeout=20):
                output, code = "", 0
                if "-productVersion" in args or "--show-sdk-version" in args:
                    output = "27.2"
                elif "--show-sdk-path" in args:
                    output = "/public/sdk"
                elif args[0] == "/usr/bin/uname":
                    output = "arm64"
                elif "clang" in args:
                    self.assertIn("-DCPX_BUNDLE_IDENTITY=1", args)
                    self.assertIn("-Werror", args)
                    Path(args[-1]).write_bytes(b"disposable signed fixture")
                elif "find-identity" in args:
                    if ci:
                        self.fail("CI must never query owner's signing inventory")
                    output = '  1) ' + 'A' * 40 + ' "Synthetic identity"\n  1 valid identities found\n'
                elif len(args) > 1 and args[1] == "probe":
                    output = "\n".join(json.dumps(row) for row in (self.rows(not ci) if rows is None else rows))
                    code = native_code
                elif len(args) > 1 and args[1] == "forced-exit":
                    code = forced_code
                elif len(args) > 1 and args[1] == "lookup-missing":
                    code = cleanup_code
                elif len(args) > 1 and args[1] in {"reaper-controls", "cleanup-control", "send-timeout-copy", "send-error-once-header"}:
                    code = {"reaper-controls": 0, "cleanup-control": 68, "send-timeout-copy": 67, "send-error-once-header": 59}[args[1]]
                return SimpleNamespace(code=code, output=output, limited=False, timed_out=False, pipe_failed=False)
            with patch.object(probe.signal, "SIGKILL", 9, create=True):
                try:
                    probe.collect(report, Path(__file__).parent, work, ci, run)
                except PreflightError as error:
                    report["diagnostic"] = str(error)
            return report

    def test_ci_only_refusal_never_becomes_owner_positive_acceptance(self):
        report = self.collect(ci=True)
        self.assertFalse(report["candidate_complete"])
        self.assertEqual(report["outcome"], "ci_adhoc_self_refusal_observed_owner_positive_cases_not_run")
        self.assertFalse(report["production_changed"])
        report = self.collect(ci=True, rows=self.rows(True))
        self.assertFalse(report["candidate_complete"])
        self.assertEqual(report["diagnostic"], "ci_adhoc_self_was_not_explicitly_refused")

    def test_owner_candidate_requires_actual_cleanup_and_zero_exit(self):
        report = self.collect()
        self.assertTrue(report["candidate_complete"])
        self.assertFalse(report["production_changed"])
        self.assertFalse(report["m8_complete"])
        for kwargs in ({"native_code": 2}, {"forced_code": 73}, {"cleanup_code": 66}, {"rows": self.rows(False)}):
            self.assertFalse(self.collect(**kwargs)["candidate_complete"])

    def test_exact_archive_includes_policy_header_and_rejects_tampering(self):
        spec = importlib.util.spec_from_file_location("bootstrap_signed_packager", Path(__file__).with_name("package-macos-bootstrap-signed.py"))
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        files = {name: Path(__file__).with_name(name).read_bytes() for name in probe.FILES}
        payload = module.packager.package_bytes("a" * 40, files, probe.FILES, probe.SCOPE)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with zipfile.ZipFile(io.BytesIO(payload)) as archive:
                self.assertEqual(archive.namelist(), list(probe.FILES) + ["MANIFEST.json"])
                self.assertEqual(archive.getinfo("MAC-MESAJ.command").external_attr >> 16, 0o100755)
                archive.extractall(root)
            self.assertEqual(probe.verify_package(root, probe.FILES, probe.SCOPE), "a" * 40)
            (root / "mac_bootstrap_bundle.h").write_bytes(b"tampered policy\n")
            with self.assertRaises(PreflightError):
                probe.verify_package(root, probe.FILES, probe.SCOPE)


if __name__ == "__main__":
    unittest.main()
