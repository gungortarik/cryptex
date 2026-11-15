"""Darwin observer tests only; no Cryptex profile, keychain or UI is touched."""
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mac_window_process import WindowProcess, natural_quit_ok


@unittest.skipUnless(sys.platform == 'darwin', 'Darwin process-group observation')
class NativeProcessTests(unittest.TestCase):
    def run_script(self, body, timeout=10):
        with tempfile.TemporaryDirectory(prefix='cx-window-observer-') as temp:
            home = Path(temp)
            script = home / 'observer-fixture'
            script.write_text('#!/bin/sh\n' + body, encoding='utf-8')
            script.chmod(0o700)
            process = WindowProcess(script, home, timeout=timeout)
            try:
                process.start()
                return process.finish(wait=15)
            finally:
                process.finish()

    def test_clean_leader_exit_is_observed_before_reap(self):
        result = self.run_script('exit 0\n')
        self.assertTrue(natural_quit_ok(result), result)

    def test_lingering_group_child_refuses_clean_quit(self):
        result = self.run_script('/bin/sleep 30 &\nexit 0\n')
        self.assertTrue(result['natural_exit'], result)
        self.assertGreater(result['same_group_live_descendants_before_cleanup'], 0, result)
        self.assertTrue(result['group_reaped'], result)
        self.assertFalse(natural_quit_ok(result), result)

    def test_timeout_is_not_natural_shutdown(self):
        result = self.run_script('exec /bin/sleep 30\n', timeout=0.5)
        self.assertTrue(result['timed_out'], result)
        self.assertTrue(result['group_reaped'], result)
        self.assertFalse(natural_quit_ok(result), result)


if __name__ == '__main__':
    unittest.main()
