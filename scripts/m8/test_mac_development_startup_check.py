import sys
from pathlib import Path
from types import SimpleNamespace
import unittest

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from mac_development_startup_check import require_refusal
from mac_identity_preflight import PreflightError


class RefusalTests(unittest.TestCase):
    def test_only_clean_exact_refusal_is_accepted(self):
        base = dict(code=2, timed_out=False, limited=False, pipe_failed=False,
                    output="local_signed_startup_Inventory\n")
        require_refusal(SimpleNamespace(**base), "local_signed_startup_Inventory")
        for change in (dict(code=0), dict(code=1), dict(timed_out=True), dict(limited=True),
                       dict(pipe_failed=True), dict(output="prefix local_signed_startup_Inventory\n"),
                       dict(output="some_other_error\n")):
            with self.subTest(change=change), self.assertRaises(PreflightError):
                require_refusal(SimpleNamespace(**(base | change)), "local_signed_startup_Inventory")


if __name__ == "__main__":
    unittest.main()
