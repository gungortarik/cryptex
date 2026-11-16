import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from windows_pairing_diagnostic import BRAIN,LOCAL,TRANSACTION,LIMIT,collect,read_json,journal_summary
from windows_setup_native import Refused


class JournalTests(unittest.TestCase):
    def test_terminal_v3_and_no_raw_transcript_export(self):
        result=journal_summary(dict(schema_version=3,transactions=[],terminal_receipts=[dict(
            kind='anchored',snapshot={'transaction_id':TRANSACTION},verified_anchor={},
            transcript_messages=['sensitive sentinel']*4)]))
        self.assertEqual(result['matching_records'][0]['message_count'],4)
        self.assertTrue(result['matching_records'][0]['terminal'])
        self.assertNotIn('sensitive sentinel',json.dumps(result))

    def test_unknown_schema_refused(self):
        with self.assertRaisesRegex(Refused,'journal_schema'):
            journal_summary({'schema_version':4})


@unittest.skipUnless(os.name=='nt','Windows path pins')
class DiskTests(unittest.TestCase):
    def fixture(self,root):
        (root/'identity').mkdir()
        (root/'identity/brain_identity.json').write_text(json.dumps({'brain_id':BRAIN}))
        (root/'identity/local_device.json').write_text(json.dumps({'brain_id':BRAIN,'device_id':LOCAL}))
        (root/'bootstrap').mkdir()
        (root/'bootstrap/pending.json').write_text(json.dumps(dict(schema_version=2,transactions=[dict(
            snapshot={'transaction_id':TRANSACTION},state='anchored',
            transcript_messages=['secret sentinel']*4,append_attempts=[{}])],exchanges=[])))

    def test_collect_preserves_every_file_and_does_not_initialize_missing_paths(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);self.fixture(root)
            before={p.relative_to(root):p.read_bytes() for p in root.rglob('*') if p.is_file()}
            result=collect(root)
            after={p.relative_to(root):p.read_bytes() for p in root.rglob('*') if p.is_file()}
            self.assertEqual(before,after)
            self.assertFalse((root/'canonical').exists())
            self.assertEqual(result['journal']['value']['matching_records'][0]['state'],'anchored')
            self.assertNotIn('secret sentinel',json.dumps(result))

    def test_other_device_refuses_before_inventory(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);self.fixture(root)
            (root/'identity/local_device.json').write_text(json.dumps({'brain_id':BRAIN,'device_id':'host'}))
            with patch('windows_pairing_diagnostic.directory_summary',side_effect=AssertionError('scanned')):
                with self.assertRaisesRegex(Refused,'wrong_vm_profile'): collect(root)

    def test_duplicate_keys_oversize_and_hardlink_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'metadata.json'
            path.write_bytes(b'{"a":1,"a":2}')
            with self.assertRaisesRegex(Refused,'duplicate_json_key'): read_json(path)
            path.write_bytes(b' '*(LIMIT+1))
            with self.assertRaisesRegex(Refused,'metadata_size_bound'): read_json(path)
            path.write_bytes(b'{}')
            os.link(path,Path(temp)/'link.json')
            with self.assertRaisesRegex(Refused,'unsafe_file_links'): read_json(path)


if __name__=='__main__': unittest.main()
