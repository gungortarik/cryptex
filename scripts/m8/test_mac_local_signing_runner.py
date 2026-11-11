"""Failure output must not reveal disposable signing credentials."""
import contextlib
import io
import os
from pathlib import Path
import runpy
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import mac_identity_preflight


class LocalSigningRunnerTests(unittest.TestCase):
    def test_trust_removal_failure_still_cleans_certificate_keychain_and_directory(self):
        stages = []
        with tempfile.TemporaryDirectory(prefix="cx-runner-test-") as parent:
            real_directory = tempfile.TemporaryDirectory
            def directory(*args, **kwargs):
                return real_directory(*args, dir=parent, **kwargs)
            def checked(_run, args, stage, *_timeout):
                stages.append(stage)
                if stage == "identify_ci_certificate":
                    return "SHA1 Fingerprint=" + "AA:" * 19 + "AA"
                if stage == "capture_ci_search_list":
                    return '"/original/login.keychain-db"'
                if stage == "verify_ci_signing_identity":
                    return "A" * 40
                if stage == "create_ci_keychain":
                    Path(args[-1]).touch()
                if stage == "remove_disposable_ci_trust":
                    raise mac_identity_preflight.PreflightError("remove_disposable_ci_trust_failed")
                if stage == "delete_ci_keychain":
                    Path(args[-1]).unlink()
                return "1 passed; 0 failed"
            outcome = SimpleNamespace(code=0, output="1 passed; 0 failed",
                                      timed_out=False, limited=False, pipe_failed=False)
            stderr, stdout = io.StringIO(), io.StringIO()
            with (patch.object(sys, "platform", "darwin"),
                  patch.dict(os.environ, {"GITHUB_ACTIONS": "true",
                      "RUNNER_ENVIRONMENT": "github-hosted", "RUNNER_OS": "macOS"}),
                  patch("shutil.which", return_value=sys.executable),
                  patch("tempfile.TemporaryDirectory", side_effect=directory),
                  patch.object(mac_identity_preflight, "checked", side_effect=checked),
                  patch.object(mac_identity_preflight, "run_bounded", return_value=outcome),
                  contextlib.redirect_stderr(stderr), contextlib.redirect_stdout(stdout)):
                with self.assertRaises(SystemExit) as status:
                    runpy.run_path(str(Path(__file__).with_name("test_mac_local_signing_ci.py")),
                                   run_name="__main__")
            self.assertEqual(status.exception.code, 2)
            self.assertLess(stages.index("remove_disposable_ci_trust"),
                            stages.index("remove_disposable_ci_public_certificate"))
            self.assertLess(stages.index("remove_disposable_ci_public_certificate"),
                            stages.index("restore_ci_search_list"))
            self.assertLess(stages.index("restore_ci_search_list"),
                            stages.index("delete_ci_keychain"))
            self.assertNotIn("local_certificate_fixture_cleanup=passed", stdout.getvalue())
            self.assertEqual(list(Path(parent).iterdir()), [])

    def test_unexpected_provisioning_exception_is_redacted_and_directory_removed(self):
        with tempfile.TemporaryDirectory(prefix="cx-runner-test-") as parent:
            real_directory = tempfile.TemporaryDirectory
            def directory(*args, **kwargs):
                return real_directory(*args, dir=parent, **kwargs)
            stderr = io.StringIO()
            stdout = io.StringIO()
            with (patch.object(sys, "platform", "darwin"),
                  patch.dict(os.environ, {"GITHUB_ACTIONS": "true",
                      "RUNNER_ENVIRONMENT": "github-hosted", "RUNNER_OS": "macOS"}),
                  patch("shutil.which", return_value=sys.executable),
                  patch("tempfile.TemporaryDirectory", side_effect=directory),
                  patch.object(mac_identity_preflight, "checked",
                      side_effect=RuntimeError("secret-fixture-password")),
                  contextlib.redirect_stderr(stderr), contextlib.redirect_stdout(stdout)):
                with self.assertRaises(SystemExit) as exit_status:
                    runpy.run_path(str(Path(__file__).with_name("test_mac_local_signing_ci.py")),
                                   run_name="__main__")
            self.assertEqual(exit_status.exception.code, 2)
            self.assertEqual(stderr.getvalue(), "local_certificate_ci_internal_failure\n")
            self.assertNotIn("secret-fixture-password", stdout.getvalue())
            self.assertEqual(list(Path(parent).iterdir()), [])


if __name__ == "__main__":
    unittest.main()
