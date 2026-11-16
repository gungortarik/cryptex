import copy
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

import lan_acceptance_receipt as receipts
from lan_acceptance_preflight import Refused

SOURCE, DIGEST = 'a' * 40, 'b' * 64


def report(system='Darwin'):
    return {
        'schema': 1, 'purpose': 'cryptex-real-lan-acceptance-preflight',
        'os': system, 'os_version': '27.2.1' if system == 'Darwin' else '10.0.26200',
        'architecture': 'arm64' if system == 'Darwin' else 'AMD64',
        'private_ipv4_candidates': ['192.168.50.104' if system == 'Darwin' else '192.168.50.137'],
        'normal_development_profile': dict.fromkeys(receipts.PROFILE_FIELDS, 'absent'),
        'legacy_development_profile': 'directory', 'profile_writes': False,
        'credentials_read': False, 'memory_content_read': False,
        'network_transfer_tested': False, 'outcome': 'observed',
        'source': SOURCE, 'collector_sha256': DIGEST,
    }


class ReceiptTests(unittest.TestCase):
    def test_bounded_zip_reads_only_expected_report_without_extraction(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'receipt.zip'
            with zipfile.ZipFile(path, 'x', zipfile.ZIP_DEFLATED) as archive:
                archive.writestr('result.json', json.dumps(report()))
            actual, digest = receipts.read_receipt(path, 'Darwin', SOURCE, DIGEST)
            self.assertEqual(actual, report())
            self.assertEqual(len(digest), 64)
            self.assertEqual(list(Path(directory).iterdir()), [path])

    def test_alias_members_extra_members_and_oversize_refuse(self):
        for members in ([('../result.json', '{}')], [('result.json', '{}'), ('other', 'x')],
                        [('result.json', ' ' * 16385)]):
            with self.subTest(members=[x[0] for x in members]), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'receipt.zip'
                with zipfile.ZipFile(path, 'x', zipfile.ZIP_DEFLATED) as archive:
                    for name, data in members:
                        archive.writestr(name, data)
                with self.assertRaises(Refused):
                    receipts.read_receipt(path, 'Darwin', SOURCE, DIGEST)

    def test_duplicate_keys_and_non_report_values_refuse(self):
        with self.assertRaisesRegex(Refused, 'duplicate_json_key'):
            json.loads('{"schema":1,"schema":1}', object_pairs_hook=receipts.unique_object)
        for value in ([], None, 'text'):
            with self.assertRaises(Refused):
                receipts.validate(value, 'Darwin', SOURCE, DIGEST)

    def test_existing_profiles_and_unexpected_scope_are_not_adopted(self):
        for key, value in [('profile_writes', True), ('network_transfer_tested', 0),
                           ('source', 'c' * 40), ('collector_sha256', 'd' * 64),
                           ('architecture', 'unknown'), ('os_version', '25.1')]:
            changed = report()
            changed[key] = value
            with self.subTest(key=key), self.assertRaises(Refused):
                receipts.validate(changed, 'Darwin', SOURCE, DIGEST)
        for state in ('directory', 'file', 'unsafe_alias', 'unavailable'):
            changed = report()
            changed['normal_development_profile']['root'] = state
            with self.subTest(state=state), self.assertRaisesRegex(Refused, 'normal_profile_not_absent'):
                receipts.validate(changed, 'Darwin', SOURCE, DIGEST)

    def test_addresses_must_be_unique_canonical_private_ipv4(self):
        for addresses in ([], ['8.8.8.8'], ['127.0.0.1'], ['192.168.1.1'] * 2,
                          ['prefix 192.168.1.1'], ['192.168.001.1'], [None]):
            changed = report()
            changed['private_ipv4_candidates'] = addresses
            with self.subTest(addresses=addresses), self.assertRaises(Refused):
                receipts.validate(changed, 'Darwin', SOURCE, DIGEST)

    def test_direct_route_plan_is_not_pairing_or_delivery_authority(self):
        windows, mac = report('Windows'), report()
        before = copy.deepcopy((windows, mac))
        arguments = dict(windows_address='192.168.50.137', mac_address='192.168.50.104',
                         route_prefix='192.168.50.0/24', next_hop='0.0.0.0', source=SOURCE)
        plan = receipts.build_plan(windows, mac, **arguments)
        self.assertEqual((windows, mac), before)
        self.assertFalse(plan['pairing_authorized'])
        self.assertFalse(plan['network_delivery_verified'])
        self.assertFalse(plan['durable_convergence_verified'])
        self.assertTrue(plan['recheck_at_launch_required'])
        for key, value in [('next_hop', '192.168.50.1'), ('route_prefix', '0.0.0.0/0'),
                           ('route_prefix', '192.168.51.0/24'), ('mac_address', '192.168.50.105')]:
            changed = dict(arguments, **{key: value})
            with self.subTest(key=key), self.assertRaises(Refused):
                receipts.build_plan(windows, mac, **changed)


if __name__ == '__main__':
    unittest.main()
