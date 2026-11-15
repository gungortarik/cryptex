import json
import unittest
from mac_identity_preflight import PreflightError
from mac_keychain_item_ci import sanitize_probe


class MetadataTests(unittest.TestCase):
    def test_accepts_only_fixed_metadata_types(self):
        fields = {'lookup_status': 0, 'keychain_unlocked': True, 'creator_cdhash_allowed': False}
        self.assertEqual(sanitize_probe(json.dumps(fields)), fields)

    def test_rejects_payload_fields_types_and_unbounded_numbers(self):
        for fields in ({'secret': 'test'}, {'lookup_status': '0'}, {'lookup_status': True},
                       {'keychain_unlocked': 1}, {'lookup_status': 2**32}, ['raw']):
            with self.subTest(fields=fields), self.assertRaises(PreflightError):
                sanitize_probe(json.dumps(fields))


if __name__ == '__main__':
    unittest.main()
