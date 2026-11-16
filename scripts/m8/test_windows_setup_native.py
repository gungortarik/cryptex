import os
from pathlib import Path
import sys
import tempfile
import time
import unittest
from concurrent.futures import ThreadPoolExecutor
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
    def test_detached_pipes_descendant_is_signaled_before_return(self):
        with tempfile.TemporaryDirectory() as temp, ThreadPoolExecutor(max_workers=1) as pool:
            root=Path(temp);pid_file=root/'child.pid';release=root/'release'
            leader=('import subprocess,sys,time;from pathlib import Path;'
                    'p=subprocess.Popen([sys.executable,"-c","import time;time.sleep(30)"],'
                    'stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);'
                    'Path('+repr(str(pid_file))+').write_text(str(p.pid));'
                    '\nwhile not Path('+repr(str(release))+').exists():time.sleep(.01)')
            future=pool.submit(run,[sys.executable,'-c',leader],10)
            deadline=time.monotonic()+5
            while not pid_file.exists() and time.monotonic()<deadline: time.sleep(.01)
            self.assertTrue(pid_file.exists())
            open_process=native.api('OpenProcess',[native.w.DWORD,native.w.BOOL,native.w.DWORD],native.w.HANDLE)
            handle=open_process(0x100000,False,int(pid_file.read_text()))
            self.assertTrue(handle)
            try:
                self.assertEqual(native.wait(handle,0),258)
                release.touch();future.result(timeout=10)
                self.assertEqual(native.wait(handle,0),0)
            finally: native.close(handle)
    def test_unknown_job_cleanup_cannot_report_success(self):
        with patch.object(native,'query_job',return_value=False):
            with self.assertRaisesRegex(Refused,'job_members_unknown'):
                run([sys.executable,'-c','print("success")'])
    def test_reader_start_fault_closes_owned_handles(self):
        current=native.api('GetCurrentProcess',[],native.w.HANDLE)()
        count=native.api('GetProcessHandleCount',[native.w.HANDLE,native.c.POINTER(native.w.DWORD)])
        before=native.w.DWORD();after=native.w.DWORD()
        self.assertTrue(count(current,native.c.byref(before)))
        with patch.object(native.threading.Thread,'start',side_effect=RuntimeError('injected')):
            with self.assertRaisesRegex(RuntimeError,'injected'):
                run([sys.executable,'-c','print("never resumes")'])
        self.assertTrue(count(current,native.c.byref(after)))
        self.assertEqual(before.value,after.value)
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
