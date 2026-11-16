import hashlib
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from windows_lan_recovery import recover,APP,MARKER
from windows_setup_native import Refused

@unittest.skipUnless(os.name=='nt','Windows namespace pins')
class RecoveryTests(unittest.TestCase):
    def fixture(self,temp):
        base=Path(temp);root=base/'Cryptex-Dev/runtime/p0-2';root.mkdir(parents=True)
        (root/'pilot-runtime.marker').write_bytes(MARKER)
        app=base/'Cryptex-Dev/lan-apps'/APP;app.mkdir(parents=True);(app/'transport').mkdir()
        for name in ('cryptexd.exe','cryptex-desktop.exe'):(app/name).write_bytes(b'fixture')
        return base,root,app,{n:hashlib.sha256(b'fixture').hexdigest() for n in ('cryptexd.exe','cryptex-desktop.exe')}
    def test_launch_refusal_preserves_profile_and_records_exact_error(self):
        with tempfile.TemporaryDirectory() as temp:
            base,root,app,hashes=self.fixture(temp);report={}
            def runner(args,*a,**kw):
                self.assertEqual(args,[app/'cryptexd.exe','--help'])
                raise Refused('process_create_failed',winerror=4551)
            with patch('windows_lan_setup.HASHES',hashes):
                with self.assertRaises(Refused) as error:recover(base,report,lambda:self.fail('asked resume'),runner)
            self.assertEqual(error.exception.winerror,4551)
            self.assertEqual([p.name for p in root.iterdir()],['pilot-runtime.marker'])
            self.assertEqual((root/'pilot-runtime.marker').read_bytes(),MARKER)
            self.assertNotIn('resume_authorized',report)
    def test_nonempty_profile_never_launches_or_mutates(self):
        with tempfile.TemporaryDirectory() as temp:
            base,root,app,hashes=self.fixture(temp);(root/'identity').mkdir();(root/'identity/state').write_bytes(b'keep')
            with patch('windows_lan_setup.HASHES',hashes):
                with self.assertRaisesRegex(Refused,'profile_not_marker_only'):
                    recover(base,{},lambda:True,lambda *a,**k:self.fail('launched'))
            self.assertEqual((root/'identity/state').read_bytes(),b'keep')
    def test_owner_decline_keeps_probe_read_only(self):
        with tempfile.TemporaryDirectory() as temp:
            base,root,app,hashes=self.fixture(temp);calls=[];report={}
            def runner(args,*a,**kw):calls.append(args);return 'cryptexd help'
            with patch('windows_lan_setup.HASHES',hashes):
                with self.assertRaisesRegex(Refused,'owner_declined_resume'):recover(base,report,lambda:False,runner)
            self.assertEqual(len(calls),1)
            self.assertTrue(report['profile_preserved_after_probe'])
            self.assertEqual([p.name for p in root.iterdir()],['pilot-runtime.marker'])
    def test_state_change_during_owner_review_refuses_resume(self):
        with tempfile.TemporaryDirectory() as temp:
            base,root,app,hashes=self.fixture(temp);calls=[]
            def ask():(root/'new-state').write_bytes(b'keep');return True
            def runner(args,*a,**kw):calls.append(args);return 'cryptexd help'
            with patch('windows_lan_setup.HASHES',hashes):
                with self.assertRaisesRegex(Refused,'profile_not_marker_only'):recover(base,{},ask,runner)
            self.assertEqual(len(calls),1)
            self.assertEqual((root/'new-state').read_bytes(),b'keep')
if __name__=='__main__':unittest.main()
