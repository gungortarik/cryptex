import json
from pathlib import Path
from types import SimpleNamespace
import unittest

from mac_keychain_context_check import compare_contexts


class DiscoveryTests(unittest.TestCase):
    def test_reports_comparison_without_private_paths_or_stderr(self):
        calls = []

        def runner(args, **kwargs):
            calls.append(args)
            temporary = kwargs['env']['HOME'] == str(Path('/isolated'))
            return SimpleNamespace(returncode=0,
                                   stdout=b'"/private/owner/login.keychain-db"' if not temporary else b'',
                                   stderr=b'private diagnostic')

        result = compare_contexts(Path('/owner'), Path('/isolated'), runner)
        self.assertTrue(result['original_context_stable'])
        self.assertFalse(result['search_list_discovery_equal'])
        self.assertFalse(result['default_discovery_equal'])
        self.assertEqual(len(calls), 6)
        self.assertTrue(all(c[1] in ('default-keychain', 'list-keychains') for c in calls))
        self.assertNotIn('private', json.dumps(result))
        self.assertNotIn('owner', json.dumps(result))

    def test_failed_discovery_is_not_successful_empty_search(self):
        def runner(args, **kwargs):
            return SimpleNamespace(returncode=44 if kwargs['env']['HOME'] == str(Path('/isolated')) else 0,
                                   stdout=b'', stderr=b'ignored')

        result = compare_contexts(Path('/owner'), Path('/isolated'), runner)
        self.assertFalse(result['search_list_discovery_equal'])
        self.assertEqual(result['temporary_exit_codes'], [44, 44])
