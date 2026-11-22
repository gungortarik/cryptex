from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import login


class LoginReceiptTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        state = patch.object(login, "STATE", self.root)
        state.start()
        self.addCleanup(state.stop)
        for leaf, content in {"armed-session": "prior", "ready-session": "new",
                              "launch-one": "new", "disabled": "ok"}.items():
            (self.root / leaf).write_text(content)

    @patch.object(login, "registration", return_value=None)
    def test_new_login(self, _):
        login.verify(True, current_session="new")
        self.assertTrue((self.root / "result.json").exists())

    @patch.object(login, "registration", return_value=None)
    def test_same_session(self, _):
        (self.root / "armed-session").write_text("new")
        with self.assertRaisesRegex(RuntimeError, "fresh login"):
            login.verify(True, current_session="new")

    @patch.object(login, "registration", return_value=None)
    def test_duplicate_before_single_instance(self, _):
        (self.root / "launch-two").write_text("new")
        with self.assertRaisesRegex(RuntimeError, "saw 2"):
            login.verify(True, current_session="new")

    @patch.object(login, "registration", return_value=None)
    def test_other_session_attempt_is_reported_separately(self, _):
        (self.root / "launch-earlier").write_text("prior")
        result = login.verify(True, current_session="new")
        self.assertEqual((result["launches"], result["other_session_launches"]), (1, 1))

    @patch.object(login, "registration", return_value=None)
    def test_stale_ready_and_empty_attempt_are_refused(self, _):
        with self.assertRaisesRegex(RuntimeError, "another login session"):
            login.verify(True, current_session="later")
        (self.root / "launch-incomplete").touch()
        with self.assertRaisesRegex(RuntimeError, "Incomplete"):
            login.verify(True, current_session="new")

    @patch.object(login, "registration", return_value="still registered")
    def test_incomplete_removal(self, _):
        with self.assertRaisesRegex(RuntimeError, "still exists"):
            login.verify(True, current_session="new")
