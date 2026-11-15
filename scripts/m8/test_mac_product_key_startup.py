from contextlib import redirect_stdout
import io
import json
from pathlib import Path
import tempfile
import unittest

from mac_identity_preflight import CommandResult, PreflightError
from mac_product_key_startup import exercise


class ProductKeyTests(unittest.TestCase):
    def test_actual_daemon_enrolls_and_changed_metadata_or_event_is_not_accepted(self):
        for failure in ('none', 'metadata', 'event', 'raw', 'failed_enroll'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                profile = root / 'product-owned-key-runtime'
                manifest = root / 'manifest'
                manifest.write_bytes(b'{}')
                calls, cleanups = [], []
                device = '11111111-1111-4111-8111-111111111111'
                key_path = profile / 'identity/devices' / device.encode().hex() / 'event_signing_key.json'
                def runner(args, timeout):
                    calls.append(args)
                    if args[0] == 'creator':
                        (profile / 'identity').mkdir()
                        (profile / 'identity/brain_identity.json').write_bytes(b'public-brain')
                        return CommandResult(0, '')
                    if 'enroll-local' in args:
                        self.assertEqual(Path(args[0]).name, 'cryptexd')
                        (profile / 'identity/local_device.json').write_text(json.dumps({'device_id': device}))
                        key_path.parent.mkdir(parents=True)
                        key_path.write_bytes(b'public-key-only')
                        return CommandResult(2 if failure == 'failed_enroll' else 0, '')
                    if manifest.read_bytes() != b'{}':
                        return CommandResult(2, 'local_signed_startup_Inventory\n')
                    if failure == 'raw':
                        raise OSError('unresolved group')
                    if failure == 'metadata':
                        key_path.write_bytes(b'changed')
                    if failure == 'event':
                        (profile / 'canonical/events').mkdir(parents=True, exist_ok=True)
                        (profile / 'canonical/events/new.seg').write_bytes(b'new')
                    return CommandResult(0, 'development_existing_profile_startup_and_cleanup=passed\n')
                with redirect_stdout(io.StringIO()):
                    if failure == 'none':
                        exercise(root, root / 'cryptex-desktop', 'creator', manifest, runner,
                                 lambda *args: cleanups.append(args))
                    else:
                        with self.assertRaises(PreflightError):
                            exercise(root, root / 'cryptex-desktop', 'creator', manifest, runner,
                                     lambda *args: cleanups.append(args))
                self.assertEqual(len(cleanups), 0 if failure == 'raw' else 1)
                self.assertEqual(manifest.read_bytes(), b'{}')
                self.assertEqual(profile.exists(), failure != 'none')
                self.assertIn('brain-only-v1', calls[0])


if __name__ == '__main__':
    unittest.main()
