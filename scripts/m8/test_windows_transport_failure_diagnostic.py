import json
import os
from pathlib import Path
import tempfile
import unittest

if os.name == 'nt':
    import windows_transport_failure_diagnostic as d


@unittest.skipUnless(os.name == 'nt', 'Windows native handles')
class FailureTests(unittest.TestCase):
    def test_only_fixed_markers_leave_parser(self):
        raw = b'PRIVATE PATH and secret\nListener starting at PRIVATE IP\nConnection closed PRIVATE TOKEN\n'
        result = d.summarize(raw, False)
        self.assertNotIn('PRIVATE', json.dumps(result))
        self.assertEqual(result['marker_counts']['listener_started'], 1)
        self.assertEqual(result['marker_counts']['connection_closed'], 1)

    def test_shared_writer_is_read_without_changes_and_tail_is_bounded(self):
        with tempfile.TemporaryDirectory() as base:
            path = Path(base) / 'native.log'
            raw = b'private text\n' * 100000 + b'Exiting\n'
            with path.open('wb') as writer:
                writer.write(raw); writer.flush()
                result = d.read_log(path)
            self.assertTrue(result['tail_only'])
            self.assertEqual(result['bytes_read'], 1024 * 1024)
            self.assertEqual(result['marker_counts']['shutdown_requested'], 1)
            self.assertEqual(path.read_bytes(), raw)

    def test_hardlinked_log_is_refused(self):
        with tempfile.TemporaryDirectory() as base:
            path = Path(base) / 'native.log'; path.write_text('Exiting')
            os.link(path, Path(base) / 'alias')
            with self.assertRaises(d.Refused):
                d.read_log(path)

    def test_wrong_vm_does_not_read_log_or_processes(self):
        with tempfile.TemporaryDirectory() as base:
            root = Path(base); (root / 'identity').mkdir()
            for name in ('brain_identity.json', 'local_device.json'):
                (root / 'identity' / name).write_text('{}')
            with self.assertRaises(d.Refused):
                d.collect(root, lambda *_: self.fail('log'), lambda: self.fail('processes'))

    def test_process_reader_rejects_unexpected_fields(self):
        with self.assertRaises(d.Refused):
            d.processes(lambda *_: '{"secret":"private"}')


if __name__ == '__main__':
    unittest.main()
