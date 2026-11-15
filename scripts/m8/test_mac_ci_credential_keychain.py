from contextlib import redirect_stdout
import io
import json
from pathlib import Path
import shlex
import tempfile
import unittest
from unittest.mock import patch

from mac_ci_credential_keychain import require_item, image_hash, CredentialKeychain
from mac_ci_existing_profile import exercise
from mac_identity_preflight import CommandResult, PreflightError


def metadata(provisioned=False):
    return dict(interaction_disabled_status=0, lookup_status=0, containing_keychain_status=0,
                keychain_state_status=0, default_keychain_status=0, copy_access_status=0,
                partition_contents_status=0, expected_keychain_status=0, keychain_unlocked=True,
                item_in_default_keychain=True, item_in_expected_keychain=True,
                partition_format_recognized=True, creator_cdhash_allowed=True,
                different_image_cdhash=True, product_cdhash_allowed=provisioned,
                match_count=1, partition_acl_count=1, partition_count=2 if provisioned else 1)


class Fake:
    def __init__(self, root, failure=None):
        self.root, self.failure = root, failure
        self.calls = []
        self.original = [str(root / 'original/login.keychain-db'), str(root / 'original/signing.keychain-db')]
        self.default, self.search = self.original[:1], self.original[:]
        self.path = None
        self.profile = root / 'ci-authorized-existing-profile'
        self.device = '11111111-1111-4111-8111-111111111111'
        self.provisioned = False
        self.failed = False

    def __call__(self, args, timeout):
        self.calls.append(args)
        if 'create-keychain' in args:
            self.path = Path(args[-1])
        if self.failure in args and not self.failed:
            self.failed = True
            return CommandResult(2, 'SHOULD_NOT_BE_PRINTED')
        command = args[1] if len(args) > 1 else ''
        output = ''
        if args[0] == '/usr/bin/security':
            if command == 'create-keychain':
                self.path.write_bytes(b'disposable')
            elif command == 'default-keychain':
                if '-s' in args:
                    if self.failure == 'raw_restore' and args[-1] == self.original[0]:
                        raise OSError('unresolved restoration process')
                    if self.failure == 'restore' and args[-1] == self.original[0]:
                        return CommandResult(2, 'RESTORE_SECRET')
                    self.default = args[args.index('-s') + 1:]
                output = shlex.join(self.default)
            elif command == 'list-keychains':
                if '-s' in args:
                    self.search = args[args.index('-s') + 1:]
                output = shlex.join(self.search)
            elif command == 'set-generic-password-partition-list':
                self.provisioned = True
            elif command == 'delete-keychain':
                self.path.unlink()
            elif command == 'delete-generic-password':
                pass
        elif command == 'pilot-fixture':
            if self.failure == 'raw_creator':
                raise OSError('unresolved creator')
            key = self.profile / 'identity/devices' / self.device.encode().hex() / 'event_signing_key.json'
            key.parent.mkdir(parents=True)
            key.write_bytes(b'public-key')
            (self.profile / 'identity/brain_identity.json').write_bytes(b'public-brain')
            (self.profile / 'identity/local_device.json').write_text(json.dumps({'device_id': self.device}))
            event = self.profile / 'canonical/events/local/00000000000000000000.seg'
            event.parent.mkdir(parents=True)
            event.write_bytes(b'unchanged-signed-synthetic-event')
            if self.failure == 'late_fixture':
                return CommandResult(2, 'SENSITIVE_FIXTURE_OUTPUT')
        elif Path(args[0]).name == 'item-probe':
            value = metadata(self.provisioned)
            if self.failure == 'foreign':
                value['item_in_expected_keychain'] = False
            if self.failure == 'duplicate':
                value['match_count'] = 2
            if self.failure == 'extra_partition' and self.provisioned:
                value['partition_count'] = 3
            output = json.dumps(value)
        elif args[0] == '/usr/bin/codesign':
            output = 'CDHash=' + ('a' if Path(args[-1]).name == 'creator' else 'b') * 40
        elif command == '--cryptex-development-check':
            if self.failure == 'raw':
                raise OSError('unresolved writer')
            if (self.root / 'manifest').read_bytes() != b'{}':
                return CommandResult(2, 'local_signed_startup_Inventory\n')
            if self.failure == 'changed':
                (self.profile / 'canonical/events/new.seg').write_bytes(b'changed')
            output = 'development_existing_profile_startup_and_cleanup=passed\n'
        return CommandResult(0, output)


class CredentialTests(unittest.TestCase):
    def test_metadata_requires_single_exact_item_and_no_extra_partition(self):
        for provisioned in (False, True):
            require_item(json.dumps(metadata(provisioned)), provisioned)
            for key, changed in (('match_count', 0), ('match_count', 2), ('partition_acl_count', 2),
                                 ('partition_count', 3), ('item_in_expected_keychain', False),
                                 ('expected_keychain_status', -1), ('creator_cdhash_allowed', False),
                                 ('product_cdhash_allowed', not provisioned), ('match_count', True),
                                 ('secret', 'unexpected')):
                captured = io.StringIO()
                with self.subTest(provisioned=provisioned, key=key, changed=changed), redirect_stdout(captured):
                    with self.assertRaises(PreflightError):
                        require_item(json.dumps(metadata(provisioned) | {key: changed}), provisioned)
                    self.assertNotIn('unexpected', captured.getvalue())
                    self.assertNotIn('secret', captured.getvalue())
        for output in ('', 'CDHash=' + 'g' * 40, ('CDHash=' + 'a' * 40 + '\n') * 2):
            with self.assertRaises(PreflightError):
                image_hash(output)

    def test_non_ci_refuses_before_any_command_or_directory_creation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            calls = []
            with patch.dict('os.environ', {'GITHUB_ACTIONS': 'false'}), self.assertRaises(PreflightError):
                with CredentialKeychain(root, lambda *args: calls.append(args)):
                    self.fail('unreachable')
            self.assertEqual(calls, [])
            self.assertEqual(list(root.iterdir()), [])

    def test_nonempty_profile_exact_authorization_and_cleanup_on_failures(self):
        failures = (None, 'create-keychain', 'unlock-keychain', 'late_fixture', 'foreign', 'duplicate',
                    'extra_partition', 'set-generic-password-partition-list', 'changed', 'raw',
                    'restore', 'raw_restore', 'raw_creator', 'delete-keychain', 'delete-generic-password')
        for failure in failures:
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                (root / 'Library/Keychains').mkdir(parents=True, mode=0o700)
                manifest = root / 'manifest'
                manifest.write_bytes(b'{}')
                fake = Fake(root, failure)
                captured = io.StringIO()
                with patch('mac_ci_credential_keychain.require_hosted_mac'), patch('pathlib.Path.home', return_value=root), redirect_stdout(captured):
                    if failure is None:
                        exercise(root, root / 'cryptex-desktop', root / 'creator', manifest, fake)
                    else:
                        with self.assertRaises(PreflightError):
                            exercise(root, root / 'cryptex-desktop', root / 'creator', manifest, fake)
                self.assertNotIn('SECRET', captured.getvalue())
                self.assertNotIn('SENSITIVE', captured.getvalue())
                self.assertNotIn('SHOULD_NOT', captured.getvalue())
                self.assertEqual(manifest.read_bytes(), b'{}')
                if failure not in ('restore', 'raw', 'raw_creator', 'raw_restore'):
                    self.assertEqual(fake.default, fake.original[:1])
                if failure not in ('raw', 'raw_creator', 'raw_restore'):
                    self.assertEqual(fake.search, fake.original)
                for args in fake.calls:
                    if 'set-generic-password-partition-list' in args:
                        self.assertEqual(args[-1], str(fake.path))
                        self.assertEqual(args[args.index('-a') + 1], 'device-event-signing-seed:' + fake.device)
                        self.assertEqual(args[args.index('-s') + 1], 'Cryptex-Dev-DeviceCredentials')
                        self.assertEqual(args[args.index('-S') + 1], 'cdhash:' + 'a' * 40 + ',cdhash:' + 'b' * 40)
                    if 'delete-generic-password' in args:
                        self.assertEqual(args[-1], str(fake.path))
                if failure in ('foreign', 'duplicate'):
                    self.assertFalse(any('set-generic-password-partition-list' in args for args in fake.calls))
                if failure in ('raw', 'raw_creator', 'raw_restore'):
                    self.assertFalse(any('delete-keychain' in args for args in fake.calls))
                    if failure in ('raw', 'raw_creator'):
                        self.assertFalse(any('delete-generic-password' in args for args in fake.calls))
                        self.assertEqual(fake.default, [str(fake.path)])
                    if failure == 'raw_restore':
                        self.assertEqual(fake.calls[-1], ['/usr/bin/security', 'default-keychain', '-d', 'user', '-s', fake.original[0]])
                elif failure != 'delete-keychain':
                    self.assertFalse(fake.path.exists())
                if failure is None:
                    self.assertFalse(fake.profile.exists())
                    self.assertIn('ci_authorized_nonempty_profile_startup_restart_refusals=passed', captured.getvalue())
                else:
                    self.assertNotIn('ci_authorized_nonempty_profile_startup_restart_refusals=passed', captured.getvalue())
                # Only fake files were made here; no real Keychain or process.
                if fake.path.exists():
                    fake.path.unlink()
                fake.path.parent.rmdir()


if __name__ == '__main__':
    unittest.main()
