import copy
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import Mock
import zipfile

import m9_owner_reader as reader
from package_m9_reader import reader_members


class OwnerReaderTests(unittest.TestCase):
    def setUp(self):
        self.plan = dict(schema=1, run_id='e0606b70-3554-46ce-a433-1d7b96ab1fab',
                         source_commit='a' * 40, source_sha256='6' * 64,
                         hashes={'cryptexd.exe': '1' * 64}, purpose='cryptex-owner-m9-reader')

    def report(self, system):
        windows = system == 'Windows'
        local, peer = reader.SIDES[system]
        build, other = ('1' * 64, '2' * 64) if windows else ('2' * 64, '1' * 64)
        nonce, echo = ('3' * 64, '4' * 64) if windows else ('4' * 64, '3' * 64)
        attempt = ('5' if windows else '6') * 32
        receipt = dict(schema=1, purpose='cryptex-verified-m9-reader', attempt=attempt,
                       local=local, peer=peer, brain=reader.BRAIN, local_build=build, peer_build=other,
                       local_nonce=nonce, peer_nonce=echo, resolved_blob=reader.EXPECTED_BLOB)
        return dict(schema=1, purpose=reader.RESULT, attempt=attempt, receipt=receipt, os=system,
                    outcome='local_peer_verified', run_id=self.plan['run_id'], source_commit=self.plan['source_commit'],
                    daemon_sha256=build, production_v2_activation='blocked')

    def test_only_reciprocal_current_receipts_pass(self):
        windows, mac = self.report('Windows'), self.report('Darwin')
        self.assertTrue(reader.compare(windows, mac, self.plan))
        self.assertTrue(reader.compare(mac, windows, self.plan))
        for changes in [dict(outcome='incomplete'), dict(os='Darwin'), dict(run_id='old'),
                        dict(source_commit='b' * 40), dict(attempt='8' * 32), dict(production_v2_activation='open')]:
            with self.subTest(changes=changes), self.assertRaises(reader.Refused):
                reader.compare(dict(windows, **changes), mac, self.plan)
        for field, value in [('local_nonce', 'c' * 64), ('peer_nonce', '3' * 64), ('peer_build', 'f' * 64),
                             ('resolved_blob', '0' * 64), ('local', reader.MAC), ('local_build', '9' * 64)]:
            altered = copy.deepcopy(windows)
            altered['receipt'][field] = value
            with self.subTest(field=field), self.assertRaises(reader.Refused):
                reader.compare(altered, mac, self.plan)

    def test_wait_ignores_an_old_attempt_and_checks_process_exit(self):
        with TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            report = self.report('Windows')
            path = root / (report['attempt'] + '.receipt')
            (root / 'old.receipt').write_text(json.dumps(report['receipt']))
            child = Mock()
            child.poll.return_value = None
            with self.assertRaisesRegex(reader.Refused, 'timeout'):
                reader.wait_for_receipt(path, child, report, self.plan, timeout=1,
                                        clock=Mock(side_effect=[0, 2]), sleep=Mock())
            path.write_text(json.dumps(report['receipt']))
            self.assertEqual(reader.wait_for_receipt(path, child, report, self.plan), report['receipt'])
            child.poll.return_value = 1
            with self.assertRaisesRegex(reader.Refused, 'desktop_exited'):
                reader.wait_for_receipt(path, child, report, self.plan)

    def test_archives_are_bounded_and_compare_offline(self):
        with TemporaryDirectory() as temp:
            paths = []
            for system in reader.SIDES:
                path = Path(temp) / (system + '.zip')
                with zipfile.ZipFile(path, 'x') as archive:
                    archive.writestr('result.json', json.dumps(self.report(system)))
                paths.append(path)
            self.assertTrue(reader.compare(*(reader.read_archive(path) for path in paths), self.plan))
            with zipfile.ZipFile(paths[0], 'a') as archive:
                archive.writestr('extra', 'unexpected')
            with self.assertRaises(reader.Refused):
                reader.read_archive(paths[0])

    def test_packet_has_one_reader_launcher_per_platform(self):
        folder, members = reader_members({}, self.plan)
        self.assertEqual(folder, 'Cryptex-M9-Okuyucu/')
        launchers = {key for key in members if key.endswith(('.command', '.cmd'))}
        self.assertEqual(launchers, {'M9-OKUYUCU-MAC.command', 'M9-OKUYUCU-VM.cmd'})
        for name in launchers:
            self.assertIn(b'm9_owner_reader.py', members[name])
        parsed = json.loads(members['candidate.json'])
        self.assertEqual(parsed['purpose'], 'cryptex-owner-m9-reader')
        self.assertNotEqual(parsed['run_id'], self.plan['run_id'])
