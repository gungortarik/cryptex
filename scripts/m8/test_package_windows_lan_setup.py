"""VM packaging must retain exact runtime tools and public session binding."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from package_windows_lan_setup import ROOT, build
from windows_lan_setup import PRODUCT, SESSION


class PackageTests(unittest.TestCase):
    def test_vm_changes_handoff_only_and_preserves_exact_committed_tools(self):
        commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            archives=[]
            with patch('package_windows_lan_setup.payload',return_value={'cryptexd.exe':b'fixture'}):
                for vm in (False,True):
                    output=root/('vm.zip' if vm else 'host.zip')
                    source,_=build(root,output,commit,vm)
                    self.assertEqual(source,commit)
                    with zipfile.ZipFile(output) as archive:
                        archives.append({n.split('/',1)[1]:archive.read(n) for n in archive.namelist()})
            host,vm=archives
            changed={n for n in host.keys()|vm.keys() if host.get(n)!=vm.get(n)}
            self.assertEqual(changed,{'BASLA.txt','WINDOWS-ESITLEME-KUR.cmd','VM-ESITLEME-KUR.cmd'})
            for name in ('windows_lan_setup.py','windows_setup_native.py','windows_lan_open.py'):
                expected=subprocess.check_output(['git','show',commit+':scripts/m8/'+name],cwd=ROOT)
                self.assertEqual(vm[name],expected)
            self.assertEqual(json.loads(vm['setup.json']),dict(schema=1,run_id=SESSION,
                             source_commit=commit,product_source=PRODUCT))
            self.assertIn(b'if errorlevel 1',vm['VM-ESITLEME-KUR.cmd'])
            self.assertIn(b'exit /b 1',vm['VM-ESITLEME-KUR.cmd'])
            self.assertIn(b'py -3 -B windows_lan_setup.py',vm['VM-ESITLEME-KUR.cmd'])


if __name__=='__main__': unittest.main()
