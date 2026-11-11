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
from mac_local_startup_ci import run_real_startup_checks, check_session_journal


class StartupRunnerTests(unittest.TestCase):
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
