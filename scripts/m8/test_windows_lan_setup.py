import hashlib
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from windows_lan_setup import setup,SESSION
from windows_setup_native import Refused

@unittest.skipUnless(os.name=='nt','Windows pins')
class SetupTests(unittest.TestCase):
    def fixture(self,temp):
        root=Path(temp);base=root/'appdata';base.mkdir();payload=root/'payload';payload.mkdir()
        (payload/'transport').mkdir()
        (payload/'cryptexd.exe').write_bytes(b'fixture')
        return payload,base,{'cryptexd.exe':hashlib.sha256(b'fixture').hexdigest()}
    def test_existing_profile_refuses_without_touching_it_or_running_commands(self):
        with tempfile.TemporaryDirectory() as temp:
            payload,base,hashes=self.fixture(temp)
            root=base/'Cryptex-Dev/runtime/p0-2';root.mkdir(parents=True)
            (root/'existing').write_bytes(b'keep')
            with patch('windows_lan_setup.HASHES',hashes):
                with self.assertRaises(Refused): setup(payload,base,SESSION,{},runner=lambda *a,**kw:self.fail('executed'))
            self.assertEqual(list(root.iterdir()),[root/'existing'])
            self.assertEqual((root/'existing').read_bytes(),b'keep')
            self.assertFalse((base/'Cryptex-Dev/lan-apps').exists())
    def test_failed_enrollment_preserves_partial_profile_and_stops(self):
        with tempfile.TemporaryDirectory() as temp:
            payload,base,hashes=self.fixture(temp);calls=[];report={}
            def runner(args,*a,**kw):
                calls.append(args[1])
                if args[1]=='pilot-fixture': return 'lan_profile_fixture=ready\n'
                raise Refused('injected_key_refusal')
            with patch('windows_lan_setup.HASHES',hashes):
                with self.assertRaisesRegex(Refused,'injected_key_refusal'): setup(payload,base,SESSION,report,runner)
            self.assertEqual(calls,['pilot-fixture','identity'])
            self.assertTrue(report['fresh_profile_created'])
            self.assertNotIn('first_note_committed',report)
            self.assertTrue((base/'Cryptex-Dev/runtime/p0-2/pilot-runtime.marker').exists())
    def test_changed_binary_refuses_before_profile_creation(self):
        with tempfile.TemporaryDirectory() as temp:
            payload,base,hashes=self.fixture(temp)
            (payload/'cryptexd.exe').write_bytes(b'changed')
            with patch('windows_lan_setup.HASHES',hashes):
                with self.assertRaisesRegex(Refused,'payload_hash_mismatch'): setup(payload,base,SESSION,{})
            self.assertFalse((base/'Cryptex-Dev').exists())

if __name__=='__main__': unittest.main()
