from pathlib import Path
import sys
import unittest
from unittest.mock import patch

if sys.platform == "win32":
    import windows_console_login as login


@unittest.skipUnless(sys.platform == "win32", "Windows process identity")
class StartupCoverageTests(unittest.TestCase):
    desktop = Path("fixture") / "cryptex-desktop.exe"

    def test_old_snapshot_cannot_establish_or_invalidate_coverage(self):
        for old in [(self.desktop, "previous"), (self.desktop.with_name("other.exe"), "new")]:
            with self.subTest(old=old), patch.object(login, "identity", return_value=old), \
                    patch.object(login, "creation_time") as timestamp:
                self.assertFalse(login.current_startup((40, 41), self.desktop, "new", 100))
                timestamp.assert_not_called()

    def test_current_desktop_must_start_after_coverage(self):
        with patch.object(login, "identity", return_value=(self.desktop, "new")), \
                patch.object(login, "creation_time", return_value=99):
            with self.assertRaisesRegex(RuntimeError, "verified current-logon Desktop"):
                login.current_startup((40, 41), self.desktop, "new", 100)

    def test_requires_installed_daemon_in_same_logon(self):
        daemon = self.desktop.with_name("cryptexd.exe")
        for logon in ("new", "previous"):
            with self.subTest(logon=logon), \
                    patch.object(login, "identity", side_effect=[(self.desktop, "new"), (daemon, logon)]), \
                    patch.object(login, "creation_time", return_value=101):
                self.assertEqual(login.current_startup((40, 41), self.desktop, "new", 100), logon == "new")
