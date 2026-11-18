import json
import os
from pathlib import Path
import tempfile
import unittest

import lan_transfer_diagnostic as d


class DiagnosticTests(unittest.TestCase):
    def fixture(self, base, system):
        root = Path(base).resolve() / 'profile'
        (root / 'identity').mkdir(parents=True)
        local, peer, ip, _, peer_ip, peer_port = d.SIDES[system]
        (root / 'identity/brain_identity.json').write_text(json.dumps({'brain_id': d.BRAIN}))
        (root / 'identity/local_device.json').write_text(json.dumps({'brain_id': d.BRAIN, 'device_id': local}))
        stream = root / 'canonical/events' / local.encode().hex()
        stream.mkdir(parents=True)
        (stream / '00000000000000000001.seg').write_bytes(b'PRIVATE MEMORY BODY')
        (stream / '00000000000000000002.open').write_bytes(b'PRIVATE OPEN BODY')
        (root / 'connection-settings').mkdir()
        (root / 'connection-settings/connections.json').write_text(json.dumps({
            'schema': 1, 'paused': False, 'listen': ip,
            'peers': [{'device_id': peer, 'endpoint': f'{peer_ip}:{peer_port}'}]}))
        return root, stream

    def test_both_sides_redacted_preserved_and_exact_probe_targets(self):
        for system, side in d.SIDES.items():
            with self.subTest(system=system), tempfile.TemporaryDirectory() as base:
                root, _ = self.fixture(base, system)
                before = {str(p.relative_to(root)): p.read_bytes() for p in root.rglob('*') if p.is_file()}
                calls = []
                report = d.collect(root, system, lambda *args: calls.append(args) or 'observed')
                self.assertEqual(calls, [(side[2], side[2], side[3]), (side[2], side[4], side[5])])
                self.assertEqual(report['settings']['value'], dict(paused=False, local_ip_matches=True,
                    peer_count=1, peer_endpoint_matches=True))
                self.assertEqual(report['directories']['canonical_local']['value']['sealed_segments'], 1)
                self.assertEqual(report['directories']['canonical_local']['value']['open_files'], 1)
                self.assertNotIn('PRIVATE', json.dumps(report))
                self.assertEqual(before, {str(p.relative_to(root)): p.read_bytes() for p in root.rglob('*') if p.is_file()})

    def test_disk_inventory_collects_both_streams_without_contacting_either_device(self):
        for system in d.SIDES:
            with self.subTest(system=system), tempfile.TemporaryDirectory() as base:
                root, _ = self.fixture(base, system)
                incoming = root / 'transport/incoming' / d.SIDES[system][1].encode().hex()
                incoming.mkdir(parents=True)
                (incoming / '00000000000000000002.seg').write_bytes(b'test')
                report = d.collect(root, system, probe=None)
                self.assertNotIn('tcp_local', report)
                self.assertNotIn('tcp_peer', report)
                self.assertEqual(report['directories']['canonical_local']['value']['segments'],
                                 [['00000000000000000001', len(b'PRIVATE MEMORY BODY')]])
                self.assertEqual(report['directories']['exchange_peer']['value']['segments'],
                                 [['00000000000000000002', 4]])
                self.assertFalse(report['canonical_signatures_verified'])

    def test_wrong_profile_never_probes(self):
        with tempfile.TemporaryDirectory() as base:
            root, _ = self.fixture(base, 'Windows')
            (root / 'identity/local_device.json').write_text('{}')
            with self.assertRaises(d.Refused):
                d.collect(root, 'Windows', lambda *_: self.fail('network probe'))

    def test_duplicate_oversized_and_hardlinked_metadata_refused(self):
        with tempfile.TemporaryDirectory() as base:
            root, _ = self.fixture(base, 'Windows')
            file = root / 'identity/brain_identity.json'
            for raw in ('{"brain_id":1,"brain_id":2}', '{' + ' ' * (1024 * 1024)):
                file.write_text(raw)
                with self.assertRaises(d.Refused):
                    d.read_metadata(file)
            file.write_text('{}')
            os.link(file, root / 'identity/alias.json')
            with self.assertRaises(d.Refused):
                d.read_metadata(file)

    def test_io_errors_remain_explicit_without_raw_paths(self):
        def fail():
            raise PermissionError(13, 'PRIVATE PATH')
        result = d.observe(fail)
        self.assertEqual(result['status'], 'unavailable')
        self.assertNotIn('PRIVATE', json.dumps(result))

    def test_metadata_is_not_recursively_traversed(self):
        with tempfile.TemporaryDirectory() as base:
            root, stream = self.fixture(base, 'Windows')
            child = stream / 'nested'; child.mkdir()
            (child / '00000000000000000002.seg').write_text('private')
            result = d.inventory(stream)
            self.assertEqual(result['directories'], 1)
            self.assertEqual(result['sealed_segments'], 1)

    @unittest.skipIf(os.name == 'nt', 'POSIX alias semantics')
    def test_posix_alias_and_fifo_refused(self):
        with tempfile.TemporaryDirectory() as base:
            root, _ = self.fixture(base, 'Darwin')
            alias = Path(base).resolve() / 'alias'; alias.symlink_to(root, target_is_directory=True)
            with self.assertRaises(OSError):
                d.read_metadata(alias / 'identity/brain_identity.json')
            fifo = root / 'identity/fifo'; os.mkfifo(fifo)
            with self.assertRaises(d.Refused):
                d.read_metadata(fifo)


if __name__ == '__main__':
    unittest.main()
