import hashlib
import io
import json
from pathlib import Path
import ssl
import sys
import tempfile
import unittest
import zipfile
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).parent))
from mac_lan_setup import public_certificate, source_members, setup
from mac_identity_preflight import PreflightError

class LanSetupTests(unittest.TestCase):
    def test_public_certificate_is_selected_by_exact_existing_identity(self):
        der = b'public certificate fixture'
        pem = ssl.DER_cert_to_PEM_cert(der)
        fingerprint = hashlib.sha1(der).hexdigest().upper()
        self.assertEqual(public_certificate(pem, fingerprint), der)
        with self.assertRaises(PreflightError): public_certificate(pem, '0'*40)

    def test_archive_refuses_alias_traversal_and_links_before_writes(self):
        for name, mode in [('../outside', 0o100600), ('a//b', 0o100600), ('a\\b', 0o100600),
                           ('/outside', 0o100600), ('link', 0o120777)]:
            raw = io.BytesIO()
            with zipfile.ZipFile(raw, 'w') as archive:
                entry = zipfile.ZipInfo(name)
                entry.filename = name  # Preserve adversarial backslashes on Windows too.
                entry.external_attr = mode << 16
                archive.writestr(entry, b'x')
            with zipfile.ZipFile(raw) as archive:
                with self.assertRaises(PreflightError): source_members(archive)

    def test_setup_order_product_owns_keys_and_note_and_never_cleans_profile(self):
        calls = []
        class Profile:
            def __init__(self, home): self.path = home/'runtime'
            def acquire(self): self.path.mkdir(); return self.path
            def check(self): pass
            def close(self): calls.append('close')
            def remove(self): raise AssertionError('persistent profile removed')
        def run(args, stage, timeout, **kwargs):
            calls.append((stage, args, kwargs))
            return {'first_note_capture': 'first_note=committed_public',
                    'product_transport_enrollment': 'transport_enrollment=local_only',
                    'initialized_startup_restart': 'development_existing_profile_startup_and_cleanup=passed'}.get(stage, '')
        with tempfile.TemporaryDirectory() as temp, patch('mac_lan_setup.DefaultProfile', Profile):
            report = {}
            setup(Path('/bundle'), Path('/creator'), Path(temp), {'run_id': 'run', 'brain_id': 'brain'}, run, lambda _: True, report)
            self.assertTrue(report['persistent_profile_preserved'])
            self.assertEqual([c[0] for c in calls[:-1]], ['public_brain_create', 'product_key_enrollment',
                'first_note_capture', 'product_transport_enrollment', 'initialized_startup_restart', 'initialized_startup_restart'])
            self.assertEqual(calls[0][1][0], Path('/creator'))
            self.assertEqual(calls[1][1][0], Path('/bundle/Contents/MacOS/cryptexd'))
            self.assertIn('input_data', calls[2][2])
            self.assertTrue((Path(temp)/'runtime/pilot-runtime.marker').exists())

    def test_declining_setup_creates_nothing(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaises(PreflightError):
                setup(Path('/bundle'), Path('/creator'), Path(temp), {'run_id': 'run'},
                      lambda *a: self.fail('command ran'), lambda _: False, {})
            self.assertEqual(list(Path(temp).iterdir()), [])

if __name__ == '__main__': unittest.main()
