import os
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest.mock import patch
import windows_setup_native as native
from windows_setup_native import Pins,Refused,run

@unittest.skipUnless(os.name=='nt','Windows native handles/jobs')
class WindowsSetupNativeTests(unittest.TestCase):
    def test_output_stdin_and_exit_refusal(self):
        self.assertEqual(run([sys.executable,'-c','import sys;print(sys.stdin.read())'],input_data=b'public'), 'public\r\n')
        with self.assertRaises(Refused): run([sys.executable,'-c','raise SystemExit(2)'])
    def test_output_bound_and_deadline(self):
        with self.assertRaises(Refused): run([sys.executable,'-c','print("x"*70000)'])
        started=time.monotonic()
        with self.assertRaisesRegex(Refused,'process_deadline'): run([sys.executable,'-c','import time;time.sleep(30)'],.1)
        self.assertLess(time.monotonic()-started,6)
    def test_job_cleans_descendant_after_leader_exits(self):
        with tempfile.TemporaryDirectory() as temp:
            marker=Path(temp)/'must-not-appear'
            child='import time,pathlib;time.sleep(2);pathlib.Path('+repr(str(marker))+').write_text("leaked")'
            leader='import subprocess,sys;subprocess.Popen([sys.executable,"-c",'+repr(child)+']);print("spawned")'
            self.assertIn('spawned',run([sys.executable,'-c',leader]))
            time.sleep(2.2)
            self.assertFalse(marker.exists())
    def test_assignment_failure_never_resumes_child(self):
        with tempfile.TemporaryDirectory() as temp:
            marker=Path(temp)/'must-not-appear'
            with patch.object(native,'assign',return_value=False):
                with self.assertRaisesRegex(Refused,'job_assign_failed'):
                    run([sys.executable,'-c','from pathlib import Path;Path('+repr(str(marker))+').touch()'])
            self.assertFalse(marker.exists())
    def test_exclusive_directory_and_retained_file_pin(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            with Pins() as pins:
                pins.chain(root)
                leaf=pins.child(root,'fresh',fresh=True)
                with self.assertRaises(Refused): pins.child(root,'fresh',fresh=True)
                file=leaf/'file';file.write_bytes(b'original')
                pins.pin(file,False)
                with self.assertRaises(OSError): file.write_bytes(b'changed')
                with self.assertRaises(OSError): leaf.rename(root/'moved')
                self.assertEqual(file.read_bytes(),b'original')

if __name__=='__main__': unittest.main()
