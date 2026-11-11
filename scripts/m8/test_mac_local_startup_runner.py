"""Regression for explicit Rust plugin resolution inside the bounded runner."""
import os
from pathlib import Path
import sys
import unittest
from types import SimpleNamespace
from unittest.mock import patch

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from mac_identity_preflight import PreflightError
from mac_local_startup_ci import run_real_startup_checks


class StartupRunnerTests(unittest.TestCase):
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
