"""Owner-authorized disposable CI credential provisioning; never owner-Mac code."""
import json
import os
from pathlib import Path
import re
import secrets
import shlex
import sys
import uuid

from mac_development_startup_check import FencedRun
from mac_identity_preflight import PreflightError, checked, unique_object

SECURITY = '/usr/bin/security'
SERVICE = 'Cryptex-Dev-DeviceCredentials'


def require_hosted_mac():
    if (sys.platform != 'darwin' or os.environ.get('GITHUB_ACTIONS') != 'true'
            or os.environ.get('RUNNER_ENVIRONMENT') != 'github-hosted'
            or os.environ.get('RUNNER_OS') != 'macOS'):
        raise PreflightError('disposable_macos_ci_required')


def keychain_paths(output, single=False):
    values = shlex.split(output)
    if ((single and len(values) != 1) or not values or len(values) != len(set(values))
            or any(not Path(value).is_absolute() for value in values)):
        raise PreflightError('ci_keychain_preferences_invalid')
    return values


def image_hash(output):
    values = re.findall(r'^CDHash=([0-9a-f]{40}|[0-9a-f]{64})$', output, re.MULTILINE)
    if len(values) != 1:
        raise PreflightError('ci_credential_image_hash_invalid')
    return values[0]


def require_item(output, provisioned):
    value = json.loads(output, object_pairs_hook=unique_object)
    statuses = ('interaction_disabled_status', 'lookup_status', 'containing_keychain_status',
                'keychain_state_status', 'default_keychain_status', 'copy_access_status',
                'partition_contents_status', 'expected_keychain_status')
    yes = ('keychain_unlocked', 'item_in_default_keychain', 'item_in_expected_keychain',
           'partition_format_recognized', 'creator_cdhash_allowed', 'different_image_cdhash')
    allowed = set(statuses + yes + ('product_cdhash_allowed', 'match_count',
                                  'partition_acl_count', 'partition_count'))
    if (not isinstance(value, dict)
            or set(value) != allowed
            or any(type(value.get(k)) is not int or value[k] != 0 for k in statuses)
            or any(value.get(k) is not True for k in yes)
            or value.get('product_cdhash_allowed') is not provisioned
            or any(type(value.get(k)) is not int or value[k] != expected for k, expected in (
                ('match_count', 1), ('partition_acl_count', 1), ('partition_count', 2 if provisioned else 1)))):
        raise PreflightError('ci_credential_item_not_exact')


class CredentialKeychain:
    def __init__(self, root, runner):
        # Separate from the outer signing TemporaryDirectory: if a producer is
        # unresolved, its default Keychain must survive that outer cleanup.
        self.root = root.resolve(strict=True).parent / ('cx-ci-credential-' + uuid.uuid4().hex)
        self.path = self.root / 'credentials.keychain-db'
        self.password = secrets.token_hex(32)
        self.fence = FencedRun(runner)
        self.runner = runner
        self.original_default = self.original_search = None
        self.preferences_attempted = self.created = False

    def run(self, args, label, timeout=30):
        # No raw command output or argv is printed, including on errors.
        result = self.fence.outcome(args, timeout)
        return checked(lambda *_: result, [], label, timeout)

    def __enter__(self):
        require_hosted_mac()
        self.root.mkdir(mode=0o700)  # Exclusive; never accept an existing scope.
        try:
            self.original_default = keychain_paths(self.run(
                [SECURITY, 'default-keychain', '-d', 'user'], 'capture_default'), single=True)
            self.original_search = keychain_paths(self.run(
                [SECURITY, 'list-keychains', '-d', 'user'], 'capture_search'))
            # create-keychain can change the search list even on a late failure.
            self.preferences_attempted = self.created = True
            self.run([SECURITY, 'create-keychain', '-p', self.password, self.path], 'create_credential_keychain')
            self.run([SECURITY, 'unlock-keychain', '-p', self.password, self.path], 'unlock_credential_keychain')
            self.run([SECURITY, 'list-keychains', '-d', 'user', '-s',
                      *self.original_search, self.path], 'select_credential_search')
            self.run([SECURITY, 'default-keychain', '-d', 'user', '-s', self.path], 'select_credential_default')
            selected = keychain_paths(self.run([SECURITY, 'default-keychain', '-d', 'user'],
                                               'verify_credential_default'), single=True)
            if Path(selected[0]).resolve(strict=True) != self.path.resolve(strict=True):
                raise PreflightError('ci_credential_default_mismatch')
            return self
        except BaseException:
            self.__exit__(*sys.exc_info())
            raise

    def __exit__(self, *_):
        stopped = self.fence.stopped
        if not stopped:
            self.password = None
            # A late SecItemAdd must not land in the restored original default.
            # The disposable runner owns final process teardown; retain scope.
            raise PreflightError('ci_credential_cleanup_process_unresolved')
        cleanup = FencedRun(self.runner)
        errors = []
        unresolved = False

        def attempt(args, label):
            nonlocal unresolved
            if unresolved:
                return None
            try:
                result = cleanup.outcome(args, 30)
                return checked(lambda *_: result, [], label, 30)
            except BaseException:
                unresolved |= not cleanup.stopped
                errors.append(label)
                return None

        if self.preferences_attempted:
            attempt([SECURITY, 'default-keychain', '-d', 'user', '-s', *self.original_default], 'restore_default')
            attempt([SECURITY, 'list-keychains', '-d', 'user', '-s', *self.original_search], 'restore_search')
            for command, expected in (('default-keychain', self.original_default), ('list-keychains', self.original_search)):
                output = attempt([SECURITY, command, '-d', 'user'], 'verify_restored_preferences')
                try:
                    if output is None or shlex.split(output) != expected:
                        errors.append('restored_preferences_mismatch')
                except ValueError:
                    errors.append('restored_preferences_invalid')
        if self.created and not unresolved and self.path.exists():
            attempt([SECURITY, 'delete-keychain', self.path], 'delete_credential_keychain')
            if self.path.exists():
                errors.append('credential_keychain_remains')
        self.password = None
        if unresolved:
            raise PreflightError('ci_credential_cleanup_process_unresolved')
        if errors:
            raise PreflightError('ci_credential_cleanup_failed')
        return False

    def authorize(self, probe, account, creator, daemon):
        args = [probe, account, creator, daemon, self.path]
        require_item(self.run(args, 'inspect_credential_before'), False)
        hashes = [image_hash(self.run(['/usr/bin/codesign', '-d', '--verbose=4', binary],
                                     'inspect_credential_image')) for binary in (creator, daemon)]
        if hashes[0] == hashes[1]:
            raise PreflightError('ci_credential_images_not_distinct')
        self.run([SECURITY, 'set-generic-password-partition-list', '-s', SERVICE,
                  '-a', account, '-S', ','.join('cdhash:' + value for value in hashes),
                  '-k', self.password, self.path], 'authorize_exact_ci_credential')
        require_item(self.run(args, 'inspect_credential_after'), True)
