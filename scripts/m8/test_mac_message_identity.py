import json
import unittest

import mac_message_identity as probe
from mac_identity_preflight import PreflightError


class MessageIdentityReceiptTests(unittest.TestCase):
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
