import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

from mac_identity_preflight import CommandResult
from mac_inventory_diagnostic import collect, summary


class DiagnosticTests(unittest.TestCase):
    def test_closed_error_metadata_preserves_errno_and_discards_private_output(self):
        result = summary(CommandResult(2, 'inventory_error path 1 0\n'))
        self.assertEqual((result['stage'], result['os_error'], result['returned_size']), ('path', 1, 0))
        for raw in ('inventory_error private 1 0\n', 'inventory_error path 999 0\n',
                    'inventory_error path 1 0\n/private/user secret', '123 1 123 1234 0 0\nsecret'):
            self.assertNotIn('secret', json.dumps(summary(CommandResult(2, raw))))
            self.assertNotIn('stage', summary(CommandResult(2, raw)))
        self.assertNotIn('stage', summary(CommandResult(2, 'inventory_error path 1 0\n', timed_out=True)))

    def test_success_exports_only_count_not_pids_or_births(self):
        report = summary(CommandResult(0, '123456 1 123456 123456789 123 0\n'))
        self.assertEqual(report['observed_live_process_count'], 1)
        self.assertNotIn('123456', json.dumps(report))

    def test_collector_only_compiles_and_reads_three_times(self):
        calls = []
        def runner(args, timeout):
            calls.append(args)
            return CommandResult(0, '') if len(calls) == 1 else CommandResult(2, 'inventory_error info 1 0\n')
        with patch('mac_inventory_diagnostic.sys.platform', 'darwin'):
            report = collect(Path('/package'), runner)
        self.assertEqual(report['outcome'], 'diagnostic_collected')
        self.assertEqual(len(report['samples']), 3)
        self.assertEqual(len(calls), 4)
        self.assertEqual(calls[0][0], '/usr/bin/xcrun')
        self.assertTrue(all(str(args[0]).endswith('probe') for args in calls[1:]))

    @unittest.skipUnless(sys.platform == 'darwin', 'actual Darwin compilation and libproc')
    def test_actual_probe_collects_without_application_or_keychain(self):
        report = collect(Path(__file__).parent)
        self.assertEqual(report['compile']['exit_code'], 0)
        self.assertEqual(report['outcome'], 'diagnostic_collected')
        self.assertEqual(len(report['samples']), 3)
        for sample in report['samples']:
            self.assertEqual(sample['exit_code'], 0)
            self.assertGreater(sample['observed_live_process_count'], 0)


if __name__ == '__main__':
    unittest.main()
