import json
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
import zipfile

import mac_message_identity as probe
from mac_identity_preflight import PreflightError


class MessageIdentityReceiptTests(unittest.TestCase):
    def test_exact_message_package_rejects_tampering_and_other_probe_scope(self):
        spec = importlib.util.spec_from_file_location('message_packager', Path(__file__).with_name('package-macos-message.py'))
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        files = {name: Path(__file__).with_name(name).read_bytes() for name in probe.FILES}
        payload = module.packager.package_bytes('a' * 40, files, probe.FILES, probe.SCOPE)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with zipfile.ZipFile(io.BytesIO(payload)) as archive:
                self.assertEqual(archive.namelist(), list(probe.FILES) + ['MANIFEST.json'])
                self.assertEqual(archive.getinfo('MAC-SUREC.command').external_attr >> 16, 0o100755)
                archive.extractall(root)
            self.assertEqual(probe.verify_package(root, probe.FILES, probe.SCOPE), 'a' * 40)
            with self.assertRaises(PreflightError):
                probe.verify_package(root)  # XPC scope must not accept a Mach package.
            (root / 'mac_message_identity.c').write_bytes(b'tampered\n')
            with self.assertRaises(PreflightError):
                probe.verify_package(root, probe.FILES, probe.SCOPE)

    @staticmethod
    def valid_rows():
        rows = []
        for name, result in probe.EXPECTED.items():
            row = {"case": name, "result": result, "pass": True}
            if name != 'forged_enrollment_pid_token_pair':
                row.update(same_pid=name != 'same_binary_shared_send_right_forged_payload',
                           same_generation=name != 'same_pid_exec_old_epoch')
            rows.append(row)
        return rows

    def test_complete_inventory_preserves_scope_and_mechanism_specific_observations(self):
        rows = self.valid_rows()
        self.assertEqual(probe.observations('\n'.join(json.dumps(r) for r in rows)), rows)
        self.assertEqual(probe.SCOPE, 'raw_mach_message_instance_probe_only')

    def test_same_pid_exec_requires_observed_generation_change(self):
        for field in ('same_pid', 'same_generation'):
            rows = self.valid_rows()
            row = next(r for r in rows if r['case'] == 'same_pid_exec_old_epoch')
            row[field] = not row[field]
            with self.assertRaises(PreflightError):
                probe.observations('\n'.join(json.dumps(r) for r in rows))

    def test_missing_duplicate_failed_unknown_and_non_boolean_receipts_refuse(self):
        rows = self.valid_rows()
        failed = [dict(r, **({'pass': False} if i == 0 else {})) for i, r in enumerate(rows)]
        wrong_result = [dict(r, **({'result': 'accepted'} if i == 0 else {})) for i, r in enumerate(rows)]
        numeric = [dict(r, **({'pass': 1} if i == 0 else {})) for i, r in enumerate(rows)]
        for data in (rows[:-1], rows + [rows[0]], failed, wrong_result, numeric):
            with self.assertRaises(PreflightError):
                probe.observations('\n'.join(json.dumps(r) for r in data))


if __name__ == '__main__':
    unittest.main()
