import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

if sys.platform == "win32":
    import windows_login_handoff as handoff


@unittest.skipUnless(sys.platform == "win32", "Windows sign-in measurement")
class HandoffTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        (self.root / "package.json").write_text(json.dumps({"source": "build"}))
        self.state = self.root / "state"
        for name, value in (("HERE", self.root), ("STATE", self.state), ("RESULT", self.root / "result.json")):
            self.enterContext(patch.object(handoff, name, value))
        self.enterContext(patch.dict(handoff.os.environ, {"USERNAME": "cryptexm14", "CRYPTEXD_BIN": ""}))
        self.enterContext(patch.object(handoff.sys, "argv", ["windows_login_handoff.py"]))
        self.enterContext(patch.object(handoff, "registration", return_value=None))
        self.enterContext(patch.object(handoff, "installed"))
        self.enterContext(patch.object(handoff, "identity", return_value=("python", "current")))

    def test_product_is_armed_after_successful_task_preflight(self):
        calls = []

        def task(op, token):
            calls.append(op)
            if op == "preflight":
                (self.state / (token + ".ready")).touch()

        with patch.object(handoff, "task", side_effect=task), \
                patch.object(handoff, "add_new", side_effect=lambda *_: calls.append("product")):
            handoff.main()
        self.assertEqual(calls, ["create", "preflight", "product"])
        self.assertTrue((self.state / "armed.json").is_file())

    def test_failed_preflight_removes_observer_without_arming_product(self):
        with patch.object(handoff, "task", side_effect=[None, RuntimeError("preflight"), None]) as task, \
                patch.object(handoff, "add_new") as add:
            with self.assertRaisesRegex(RuntimeError, "preflight"):
                handoff.main()
        self.assertEqual([c.args[0] for c in task.call_args_list], ["create", "preflight", "remove"])
        add.assert_not_called()
        self.assertFalse((self.state / "armed.json").exists())

    def test_late_real_login_is_reported_even_when_cleanup_succeeds(self):
        token = "a" * 32
        self.state.mkdir()
        (self.state / "armed.json").write_text(json.dumps({
            "source": "build", "folder": str(self.root), "session": "old", "token": token, "phase": "armed"}))
        with patch.object(handoff.sys, "argv", ["handoff", "observe", token]), \
                patch.object(handoff, "check_entry"), patch.object(handoff, "remove_owned"), \
                patch.object(handoff, "task") as task, \
                patch.object(handoff, "observe", side_effect=RuntimeError("late")):
            handoff.main()
        report = json.loads(handoff.RESULT.read_bytes())
        self.assertEqual((report["passed"], report["distinct_logon"], report["error"]), (False, True, "late"))
        self.assertTrue(report["startup_entry_removed"])
        task.assert_called_once_with("remove", token)

    def test_calibration_error_reaches_parent_even_with_different_scheduler_token(self):
        def task(operation, token):
            if operation == "preflight":
                with patch.object(handoff.sys, "argv", ["handoff", "observe", token]), \
                        patch.object(handoff, "identity", return_value=("pythonw", "different")), \
                        patch.object(handoff.ConsoleObserver, "calibrate", side_effect=RuntimeError("missing PID")):
                    with self.assertRaisesRegex(RuntimeError, "missing PID"):
                        handoff.main()

        with patch.object(handoff, "task", side_effect=task), patch.object(handoff, "add_new") as add:
            with self.assertRaisesRegex(RuntimeError, "missing PID"):
                handoff.main()
        add.assert_not_called()
        self.assertFalse(handoff.RESULT.exists())
