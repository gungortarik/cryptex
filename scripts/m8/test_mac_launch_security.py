import copy
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

import mac_launch_security as probe
from mac_identity_preflight import PreflightError


class LaunchSecurityReceiptTests(unittest.TestCase):
    @staticmethod
    def rows(eligible=True, supported=True):
        rows = [{"capability": "atomic_api", "headers": supported, "runtime": supported},
                {"capability": "self_anchored_bundle", "eligible": eligible,
                 "diagnostic": "verified_same_organization_bundle" if eligible else "apple_anchor_or_fixed_self_identifier_refused"}]
        expected = dict(probe.ANCHOR_CASES) if eligible else {}
        if eligible and supported:
            expected.update(probe.CHANNEL_CASES)
        return rows + [{"case": name, "result": result, "pass": True} for name, result in expected.items()]

    @staticmethod
    def parse(rows):
        return probe.parse_native("\n".join(json.dumps(row) for row in rows))

    def test_full_inventory_requires_both_mechanisms_and_every_observation(self):
        native = self.parse(self.rows())
        self.assertTrue(probe.complete(native))
        self.assertEqual(len(native["cases"]), 10)
        self.assertEqual(probe.SCOPE, "macos_atomic_launch_same_organization_candidate_only")

    def test_ci_adhoc_and_missing_native_api_never_complete(self):
        for eligible, supported in ((False, False), (False, True), (True, False)):
            self.assertFalse(probe.complete(self.parse(self.rows(eligible, supported))))

    def test_failed_case_is_preserved_and_never_complete(self):
        for index in range(2, 12):
            rows = self.rows()
            rows[index]["pass"] = False
            native = self.parse(rows)
            self.assertFalse(probe.complete(native))
            self.assertFalse(native["cases"][index - 2]["pass"])

    def test_missing_duplicate_unknown_or_extra_privacy_fields_refuse(self):
        rows = self.rows()
        unknown = copy.deepcopy(rows)
        unknown[2]["case"] = "unknown"
        leaked = copy.deepcopy(rows)
        leaked[1]["team_id"] = "not_allowed"
        for value in (rows[:-1], rows + [rows[-1]], unknown, leaked, rows[:1], [None, None]):
            with self.assertRaises(PreflightError):
                self.parse(value)

    def test_non_boolean_and_inconsistent_capabilities_refuse(self):
        for row, key in ((0, "headers"), (0, "runtime"), (1, "eligible"), (2, "pass")):
            value = self.rows()
            value[row][key] = 1
            with self.assertRaises(PreflightError):
                self.parse(value)
        rows = self.rows()
        rows[0]["headers"] = False
        with self.assertRaises(PreflightError):
            self.parse(rows)
        rows = self.rows()
        rows[1]["diagnostic"] = "untrusted_self"
        with self.assertRaises(PreflightError):
            self.parse(rows)

    def test_duplicate_json_fields_and_unbounded_output_refuse(self):
        with self.assertRaises(PreflightError):
            probe.parse_native('{"capability":"atomic_api","headers":true,"headers":false}\n{}')
        with self.assertRaises(PreflightError):
            probe.parse_native("x" * 65537)

    def test_exact_package_scope_and_bytes_are_bound(self):
        spec = importlib.util.spec_from_file_location("launch_packager", Path(__file__).with_name("package-macos-launch-security.py"))
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        files = {name: Path(__file__).with_name(name).read_bytes() for name in probe.FILES}
        payload = module.packager.package_bytes("a" * 40, files, probe.FILES, probe.SCOPE)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with zipfile.ZipFile(io.BytesIO(payload)) as archive:
                self.assertEqual(archive.namelist(), list(probe.FILES) + ["MANIFEST.json"])
                self.assertEqual(archive.getinfo("MAC-BAGLANTI.command").external_attr >> 16, 0o100755)
                archive.extractall(root)
            self.assertEqual(probe.verify_package(root, probe.FILES, probe.SCOPE), "a" * 40)
            with self.assertRaises(PreflightError):
                probe.verify_package(root)
            (root / "mac_launch_security.c").write_bytes(b"tampered\n")
            with self.assertRaises(PreflightError):
                probe.verify_package(root, probe.FILES, probe.SCOPE)


if __name__ == "__main__":
    unittest.main()
