from contextlib import redirect_stdout
import io
import json
from pathlib import Path
import tempfile
import unittest
import sys

from mac_identity_preflight import CommandResult, PreflightError
from mac_product_key_startup import exercise, profile_snapshot


class ProductKeyTests(unittest.TestCase):
    @unittest.skipUnless(sys.platform == 'darwin', 'actual Darwin symlink refusal')
    def test_dangling_event_directory_is_not_an_empty_canonical_tree(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            device = '11111111-1111-4111-8111-111111111111'
            key = root / 'identity/devices' / device.encode().hex() / 'event_signing_key.json'
            key.parent.mkdir(parents=True)
            key.write_bytes(b'public')
            (root / 'identity/brain_identity.json').write_bytes(b'public')
            (root / 'identity/local_device.json').write_text(json.dumps({'device_id': device}))
            (root / 'canonical').mkdir()
            (root / 'canonical/events').symlink_to(root / 'absent')
            with self.assertRaises(PreflightError):
                profile_snapshot(root)

    def test_actual_daemon_enrolls_and_changed_metadata_or_event_is_not_accepted(self):
        for failure in ('none', 'alias', 'metadata', 'event', 'raw', 'failed_enroll'):
            if failure == 'alias' and sys.platform != 'darwin':
                continue
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as directory:
                parent = Path(directory).resolve(strict=True)
                root = parent / 'owned'
                root.mkdir(mode=0o700)
                supplied_root = root
                if failure == 'alias':
                    supplied_root = parent / 'alias'
                    supplied_root.symlink_to(root, target_is_directory=True)
                profile = root / 'product-owned-key-runtime'
                manifest = root / 'manifest'
                manifest.write_bytes(b'{}')
                calls, cleanups = [], []
                device = '11111111-1111-4111-8111-111111111111'
                key_path = profile / 'identity/devices' / device.encode().hex() / 'event_signing_key.json'
                def runner(args, timeout):
                    calls.append(args)
                    if '--runtime-root' in args:
                        self.assertEqual(Path(args[args.index('--runtime-root') + 1]), profile)
                    if 'prepare-empty-stream-v1' in args:
                        (profile / 'canonical/events/inventory.lock').write_bytes(b'')
                        stream = profile / 'canonical/events' / device.encode().hex()
                        stream.mkdir()
                        (stream / 'writer.lock').write_bytes(b'')
                        return CommandResult(0, '')
                    if args[0] == 'creator':
                        (profile / 'identity').mkdir()
                        (profile / 'identity/brain_identity.json').write_bytes(b'public-brain')
                        (profile / 'canonical/events').mkdir(parents=True)
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
                    if failure in ('none', 'alias'):
                        exercise(supplied_root, root / 'cryptex-desktop', 'creator', manifest, runner,
                                 lambda *args: cleanups.append(args))
                    else:
                        with self.assertRaises(PreflightError):
                            exercise(root, root / 'cryptex-desktop', 'creator', manifest, runner,
                                     lambda *args: cleanups.append(args))
                self.assertEqual(len(cleanups), 0 if failure == 'raw' else 1)
                self.assertEqual(manifest.read_bytes(), b'{}')
                self.assertEqual(profile.exists(), failure not in ('none', 'alias'))
                self.assertIn('brain-only-v1', calls[0])


if __name__ == '__main__':
    unittest.main()
