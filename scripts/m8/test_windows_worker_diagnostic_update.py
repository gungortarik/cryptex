import os
from pathlib import Path
import unittest
if os.name=='nt':
    import windows_worker_diagnostic_update as d

@unittest.skipUnless(os.name=='nt','Windows helper imports')
class WaitTests(unittest.TestCase):
    def test_first_valid_failure_finishes_without_sleep(self):
        self.assertTrue(d.wait_for_failure(Path('fixture'),reader=lambda _: {},clock=lambda:0,
            sleep=lambda _:self.fail('unexpected sleep')))

    def test_absence_is_bounded_and_never_reported_as_success(self):
        now=[0]
        def missing(_): raise FileNotFoundError()
        def sleep(n): now[0]+=n
        self.assertFalse(d.wait_for_failure(Path('fixture'),seconds=5,reader=missing,
            clock=lambda:now[0],sleep=sleep))
        self.assertEqual(now[0],5)

if __name__=='__main__': unittest.main()
