import contextlib
import io
import json
import os
from pathlib import Path
import runpy
import sys
import tempfile
import unittest
import zipfile
from unittest.mock import patch

import mac_combined_loss_check
from mac_identity_preflight import PreflightError


class CombinedOwnerTests(unittest.TestCase):
    def run_owner(self, parent, failure=None, expired=False):
        package = parent / 'package'
        package.mkdir()
        script = package / 'MAC-BIRLIKTE.command'
        script.write_bytes(Path(__file__).with_name('mac_combined_loss_owner.py').read_bytes())
        (package/'fixture.json').write_text(json.dumps({'source':'a'*40, 'architecture':'arm64',
            'expires_utc':'2020-01-01T00:00:00+00:00' if expired else '2126-01-01T00:00:00+00:00'}), encoding='utf-8')
        real_mkdtemp = tempfile.mkdtemp
        def mkdtemp(**kwargs):
            return real_mkdtemp(dir=parent, **kwargs)
        def check(_binary, root, _run):
            root.mkdir()
            if failure: raise PreflightError(failure)
        mask = os.umask(0o077)
        try:
            with (patch.object(sys, 'platform', 'darwin'), patch('platform.machine', return_value='arm64'),
                  patch('tempfile.mkdtemp', side_effect=mkdtemp),
                  patch.object(mac_combined_loss_check, 'check_combined_loss', side_effect=check) as driver,
                  contextlib.redirect_stdout(io.StringIO()), self.assertRaises(SystemExit) as status):
                runpy.run_path(str(script), run_name='__main__')
            results = list(package.glob('MAC-BIRLIKTE-SONUC-*.zip'))
            self.assertEqual(len(results), 1)
            with zipfile.ZipFile(results[0]) as archive:
                self.assertEqual(archive.namelist(), ['result.json'])
                report = json.loads(archive.read('result.json'))
            return report, status.exception.code, driver.call_count
        finally:
            os.umask(mask)

    def test_success_requires_scenario_and_cleanup_and_emits_no_runtime_details(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report, code, calls = self.run_owner(root)
            self.assertEqual((code,calls), (0,1))
            self.assertEqual(report['outcome'], 'passed')
            self.assertEqual(report['source'], 'a'*40)
            self.assertTrue(all(report.get(key) is True for key in (
                'combined_supervisor_loss_and_refusal_verified','credential_cleanup_verified','runtime_cleanup_verified',
                'guardian_output_loss_cleanup_verified')))
            self.assertEqual(len(report), 7)
            self.assertEqual([p.name for p in root.iterdir()], ['package'])

    def test_failed_scenario_preserves_evidence_and_never_claims_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report, code, calls = self.run_owner(root, failure='combined_loss_engines_not_live')
            self.assertEqual((code,calls), (1,1))
            self.assertEqual(report['outcome'], 'failed')
            self.assertEqual(report['diagnostic'], 'combined_loss_engines_not_live')
            self.assertNotIn('credential_cleanup_verified', report)
            self.assertNotIn('runtime_cleanup_verified', report)
            self.assertEqual(len(list(root.iterdir())), 2)

    def test_expired_fixture_does_not_start_any_driver(self):
        with tempfile.TemporaryDirectory() as directory:
            report, code, calls = self.run_owner(Path(directory), expired=True)
            self.assertEqual((code,calls), (1,0))
            self.assertEqual(report['outcome'], 'fixture_expired')


if __name__ == '__main__':
    unittest.main()
