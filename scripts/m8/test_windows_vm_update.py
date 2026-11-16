import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest

from windows_vm_update import FILES, install
from windows_pairing_diagnostic import BRAIN,LOCAL
from windows_setup_native import Refused


@unittest.skipUnless(os.name=='nt','Windows namespace pins')
class UpdateTests(unittest.TestCase):
    def fixture(self,temp):
        base=Path(temp)/'appdata';package=Path(temp)/'packet'
        root=base/'Cryptex-Dev/runtime/p0-2';(root/'identity').mkdir(parents=True)
        (root/'identity/brain_identity.json').write_text(json.dumps({'brain_id':BRAIN}))
        (root/'identity/local_device.json').write_text(json.dumps({'brain_id':BRAIN,'device_id':LOCAL}))
        (root/'canonical').mkdir();(root/'canonical/immutable').write_bytes(b'preserve canonical bytes')
        old=base/'Cryptex-Dev/lan-apps/old';old.mkdir(parents=True);(old/'old.exe').write_bytes(b'old')
        (package/'payload/transport').mkdir(parents=True)
        hashes={}
        for name in FILES:
            data=name.encode();(package/'payload'/name).write_bytes(data)
            hashes[name]=hashlib.sha256(data).hexdigest()
        for name in ('windows_vm_update.py','windows_setup_native.py','windows_pairing_diagnostic.py'):
            (package/name).write_bytes(b'test helper')
        return base,package,root,dict(hashes=hashes,product_source='a'*40,tool_source='b'*40)

    def test_install_keeps_profile_and_old_app_and_does_not_start_daemon(self):
        with tempfile.TemporaryDirectory() as temp:
            base,package,root,manifest=self.fixture(temp)
            before={p.relative_to(root):p.read_bytes() for p in root.rglob('*') if p.is_file()}
            calls=[]
            app=install(package,base,manifest,lambda args,*_:calls.append(args))
            self.assertEqual(len(calls),1)
            self.assertEqual(Path(calls[0][0]).name,'powershell.exe')
            self.assertEqual(before,{p.relative_to(root):p.read_bytes() for p in root.rglob('*') if p.is_file()})
            self.assertEqual((base/'Cryptex-Dev/lan-apps/old/old.exe').read_bytes(),b'old')
            self.assertEqual((app/'cryptexd.exe').read_bytes(),b'cryptexd.exe')

    def test_live_process_refusal_installs_nothing(self):
        with tempfile.TemporaryDirectory() as temp:
            base,package,root,manifest=self.fixture(temp)
            def running(*_): raise Refused('command_failed')
            with self.assertRaises(Refused): install(package,base,manifest,running)
            self.assertEqual([p.name for p in (base/'Cryptex-Dev/lan-apps').iterdir()],['old'])

    def test_changed_payload_or_host_identity_never_executes(self):
        for change_identity in (False,True):
            with self.subTest(change_identity=change_identity), tempfile.TemporaryDirectory() as temp:
                base,package,root,manifest=self.fixture(temp)
                if change_identity:
                    (root/'identity/local_device.json').write_text(json.dumps({'brain_id':BRAIN,'device_id':'host'}))
                else: (package/'payload/cryptexd.exe').write_bytes(b'tampered')
                with self.assertRaises(Refused):
                    install(package,base,manifest,lambda *_:self.fail('executed'))
                self.assertEqual([p.name for p in (base/'Cryptex-Dev/lan-apps').iterdir()],['old'])


if __name__=='__main__': unittest.main()
