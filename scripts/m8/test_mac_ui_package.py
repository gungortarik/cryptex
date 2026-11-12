import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

from mac_ui_owner import summarize
from mac_ui_package import export_fixture


class InteractiveFixtureTests(unittest.TestCase):
    def test_export_contains_only_tested_files_and_public_metadata(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            files = []
            for name in ("Contents/MacOS/cryptex-desktop", "Contents/MacOS/cryptexd",
                         "Contents/Resources/startup-check.txt", "Contents/Resources/frontend/index.html"):
                path = root / name
                path.parent.mkdir(exist_ok=True, parents=True)
                path.write_bytes(b"fixture")
                files.append({"path": name, "size": 7, "sha256": hashlib.sha256(b"fixture").hexdigest()})
            for name in ("m8-startup.json", "m8-startup.cms"):
                (root / "Contents/Resources" / name).write_bytes(b"public")
            (root / "key.pem").write_bytes(b"private")
            package = root / "fixture.zip"
            args = ("0" * 40, "arm64", "notAfter=Nov 12 01:00:00 2026 GMT")
            export_fixture(root, files, package, *args)
            with zipfile.ZipFile(package) as archive:
                self.assertEqual(len(archive.namelist()), 8)
                self.assertFalse(any("key.pem" in name for name in archive.namelist()))
                command = archive.getinfo("Cryptex-Mac-UI/MAC-ARAYUZ.command")
                self.assertTrue(command.external_attr >> 16 & 0o100)
                self.assertEqual(json.loads(archive.read("Cryptex-Mac-UI/fixture.json"))["expires_utc"],
                                 "2026-11-12T01:00:00+00:00")
            (root / files[0]["path"]).write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "changed after test"):
                export_fixture(root, files, root / "changed.zip", *args)

    def test_durable_export_includes_collector_dependencies_and_executable(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            files = []
            for name in ("Contents/MacOS/cryptex-desktop", "Contents/MacOS/cryptexd",
                         "Contents/Resources/startup-check.txt", "Contents/Resources/frontend/index.html"):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"fixture")
                files.append({"path": name, "size": 7, "sha256": hashlib.sha256(b"fixture").hexdigest()})
            for name in ("m8-startup.json", "m8-startup.cms"):
                (root / "Contents/Resources" / name).write_bytes(b"public")
            package = root / "durable.zip"
            export_fixture(root, files, package, "0" * 40, "arm64",
                           "notAfter=Nov 12 01:00:00 2026 GMT", durable=True)
            with zipfile.ZipFile(package) as archive:
                prefix = "Cryptex-Mac-Kalici/"
                self.assertEqual(len(archive.namelist()), 10)
                for helper in ("mac_durable_check.py", "mac_identity_preflight.py"):
                    self.assertEqual(archive.read(prefix + helper),
                                     Path(__file__).with_name(helper).read_bytes())
                command = archive.getinfo(prefix + "MAC-KALICI.command")
                self.assertTrue(command.external_attr >> 16 & 0o100)

    def test_exact_owner_outcome_and_duplicate_or_missing_operation(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "bootstrap").mkdir()
            (root / "ui-receipt.json").write_text('{"eventCount":2,"prefixPreserved":true}')
            states = [{"state": "anchored", "append_attempts": [1]}, {"state": "cancelled", "append_attempts": []}]
            for rows, accepted in ((states, True), (states[:1], False), (states + states, False)):
                (root / "bootstrap/pending.json").write_text(json.dumps({"transactions": rows}))
                self.assertEqual(summarize(root), {"two_events_original_prefix_preserved": True,
                                                  "one_cancel_and_one_confirm": accepted})
            (root / "ui-receipt.json").unlink()
            (root / "bootstrap/pending.json").write_text(json.dumps({"transactions": states}))
            self.assertEqual(summarize(root), {"two_events_original_prefix_preserved": None,
                                              "one_cancel_and_one_confirm": True})
