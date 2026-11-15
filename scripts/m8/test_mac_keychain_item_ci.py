import json
import unittest
from pathlib import Path
import tempfile
from contextlib import redirect_stdout
import io
from mac_identity_preflight import PreflightError, CommandResult
from mac_keychain_item_ci import sanitize_probe, exercise


class MetadataTests(unittest.TestCase):
    def test_failed_creator_cleans_generated_key_only_after_group_reaped(self):
        for raw in (False, True):
            with self.subTest(raw=raw), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                calls = []
                device = '11111111-1111-4111-8111-111111111111'
                def runner(args, timeout):
                    calls.append(args)
                    if 'signed-transfer-v1' in args:
                        identity = root / 'probe-runtime/identity'
                        identity.mkdir()
                        (identity / 'local_device.json').write_text(json.dumps({'device_id': device}))
                        if raw:
                            raise OSError('group state unknown')
                        return CommandResult(2, '')
                    if '-r-' in args:
                        return CommandResult(0, 'designated => fixture\n')
                    return CommandResult(0, '')
                report = {}
                with redirect_stdout(io.StringIO()), self.assertRaises(OSError if raw else PreflightError):
                    exercise(root / 'package', root, report, runner=runner)
                cleanups = [args for args in calls if 'delete-generic-password' in args]
                self.assertEqual(len(cleanups), 0 if raw else 1)
                self.assertEqual(report['captured_groups_reaped'], not raw)
                if cleanups:
                    self.assertEqual(cleanups[0][-1], 'device-event-signing-seed:' + device)

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
