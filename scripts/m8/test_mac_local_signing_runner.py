"""Failure output must not reveal disposable signing credentials."""
import contextlib
import io
import os
from pathlib import Path
import runpy
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import mac_identity_preflight


class LocalSigningRunnerTests(unittest.TestCase):
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
