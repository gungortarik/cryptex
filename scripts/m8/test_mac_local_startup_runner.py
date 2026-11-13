"""Regression for explicit Rust plugin resolution inside the bounded runner."""
import os
import json
from pathlib import Path
import sys
import unittest
import tempfile
from types import SimpleNamespace
from unittest.mock import patch

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from mac_identity_preflight import PreflightError
from mac_local_startup_ci import run_real_startup_checks, check_session_journal, sign_fixture_image


class StartupRunnerTests(unittest.TestCase):
    def test_each_certificate_signature_unlocks_only_its_fixture_immediately_before_use(self):
        calls = []
        def run(args, stage):
            calls.append((args, stage))
        for stage in ("sign_real_daemon", "resign_wrong_identifier"):
            sign_fixture_image(run, "/fixture/daemon", "A" * 40, "fixture.identifier",
                               "/fixture/test.keychain-db", "fixture-password", stage)
        self.assertEqual(len(calls), 4)
        for unlock, signing in (calls[:2], calls[2:]):
            self.assertEqual(unlock[0], ["/usr/bin/security", "unlock-keychain", "-p",
                                        "fixture-password", "/fixture/test.keychain-db"])
            self.assertEqual(unlock[1], "unlock_ci_keychain_before_" + signing[1])
            self.assertEqual(signing[0][-3:], ["--keychain", "/fixture/test.keychain-db", "/fixture/daemon"])
            self.assertNotIn("fixture-password", signing[0])
            self.assertNotIn("set-keychain-settings", unlock[0])

    def test_unlock_failure_never_attempts_certificate_signing(self):
        calls = []
        def run(args, _stage):
            calls.append(args)
            raise PreflightError("fixture_unlock_failed")
        with self.assertRaisesRegex(PreflightError, "fixture_unlock_failed"):
            sign_fixture_image(run, "/fixture/daemon", "A" * 40, "fixture.identifier",
                               "/fixture/test.keychain-db", "fixture-password", "sign_fixture")
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][1], "unlock-keychain")

    def test_adhoc_negative_fixture_never_uses_a_keychain(self):
        calls = []
        sign_fixture_image(lambda args, stage: calls.append((args, stage)),
                           "/fixture/daemon", "-", "fixture.identifier",
                           "/fixture/test.keychain-db", "fixture-password", "resign_missing_leaf")
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][0][0], "/usr/bin/codesign")
        self.assertNotIn("--keychain", calls[0][0])
        self.assertNotIn("fixture-password", calls[0][0])

    def test_session_outcomes_are_order_independent_and_exact(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "pending.json"
            states = ("anchored", "cancelled", "owner_pending")
            def write(values):
                path.write_text(json.dumps({"transactions": [
                    {"state": state, "append_attempts": [1] if state == "anchored" else []}
                    for state in values]}), encoding="utf-8")
            for order in (states, tuple(reversed(states))):
                write(order)
                check_session_journal(path, states)
            for invalid in (states + ("anchored",), ("cancelled", "owner_pending")):
                write(invalid)
                with self.assertRaisesRegex(PreflightError, "journal_outcome_mismatch"):
                    check_session_journal(path, states)
            write(states)
            journal = json.loads(path.read_text(encoding="utf-8"))
            journal["transactions"][0]["append_attempts"].append(2)
            path.write_text(json.dumps(journal), encoding="utf-8")
            with self.assertRaisesRegex(PreflightError, "journal_outcome_mismatch"):
                check_session_journal(path, states)

    def test_explicit_clippy_and_cargo_paths_and_environment_restoration(self):
        calls = []
        before = {"CARGO": "original-cargo", "CRYPTEX_M8_LOCAL_STARTUP_CERT_DER": "original-pin",
                  "CARGO_PROFILE_DEV_DEBUG": "original-debug"}
        def bounded(args, _timeout):
            calls.append(args)
            self.assertEqual(args[:2], ["/selected/cargo-clippy", "clippy"])
            self.assertEqual(args[-3:], ["--", "-D", "warnings"])
            self.assertEqual(os.environ["CARGO"], "/selected/cargo")
            return SimpleNamespace(code=0, output="", timed_out=False, limited=False, pipe_failed=False)
        def checked(*_args):
            raise PreflightError("stop_after_first_command")
        with patch.dict(os.environ, before), patch("shutil.which", return_value="/selected/cargo-clippy"):
            with self.assertRaisesRegex(PreflightError, "stop_after_first_command"):
                run_real_startup_checks(Path("/fixture"), checked, bounded, "/selected/cargo",
                    "/selected/openssl", "/fixture/keychain", "disposable-password", "A" * 40)
            self.assertEqual({name: os.environ[name] for name in before}, before)
        self.assertEqual(len(calls), 1)


if __name__ == "__main__":
    unittest.main()
