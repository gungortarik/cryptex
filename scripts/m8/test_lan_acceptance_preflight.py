import hashlib
import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import lan_acceptance_preflight as preflight


class PreflightTests(unittest.TestCase):
    def test_private_addresses_exclude_public_loopback_linklocal_and_invalid(self):
        addresses = preflight.private_addresses(
            '192.168.1.2 10.3.2.1 172.16.4.2 172.31.255.4 192.168.1.2 '
            '127.0.0.1 169.254.1.2 172.15.1.1 172.32.1.1 8.8.8.8 999.1.2.3')
        self.assertEqual(addresses, ['10.3.2.1', '172.16.4.2', '172.31.255.4', '192.168.1.2'])

    def test_mac_receipt_excludes_broadcast_and_preserves_existing_profile(self):
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary).resolve()
            root = home / 'Library/Application Support/Cryptex-Dev/runtime/p0-2'
            (root / 'identity').mkdir(parents=True)
            secret = root / 'identity/brain_identity.json'
            secret.write_text('PRIVATE-SENTINEL-not-a-readable-profile', encoding='utf-8')
            calls = []

            def runner(arguments):
                calls.append(arguments)
                if arguments[0] == '/sbin/ifconfig':
                    return 'en0:\n\tinet 192.168.4.5 netmask 0xffffff00 broadcast 192.168.4.255\nlo0:\n\tinet 127.0.0.1 netmask 0xff000000\n'
                return '26.1\n'

            before = secret.read_bytes()
            with patch.object(Path, 'read_bytes', side_effect=AssertionError('profile content read')):
                report = preflight.collect('Darwin', home, {}, runner)
            self.assertEqual(report['private_ipv4_candidates'], ['192.168.4.5'])
            self.assertEqual(report['normal_development_profile']['brain_metadata'], 'file')
            self.assertEqual(secret.read_bytes(), before)
            self.assertNotIn('PRIVATE-SENTINEL', json.dumps(report))
            self.assertNotIn(str(home), json.dumps(report))
            self.assertEqual(calls, [['/sbin/ifconfig', '-a'], ['/usr/bin/sw_vers', '-productVersion']])
            self.assertFalse(report['profile_writes'])
            self.assertFalse(report['network_transfer_tested'])

    def test_windows_uses_fixed_observer_and_does_not_create_profile(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            calls = []

            def runner(arguments):
                calls.append(arguments)
                return '10.0.0.7\r\n127.0.0.1\r\n'

            report = preflight.collect('Windows', root, {
                'LOCALAPPDATA': str(root), 'SystemRoot': str(root / 'system')}, runner)
            self.assertEqual(report['private_ipv4_candidates'], ['10.0.0.7'])
            self.assertEqual(set(report['normal_development_profile'].values()), {'absent'})
            self.assertEqual(list(root.iterdir()), [])
            self.assertEqual(len(calls), 1)
            self.assertIn('Get-NetIPAddress', calls[0][-1])

    def test_unsafe_ancestor_does_not_get_followed(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            unsafe = base / 'alias'
            actual_lstat = Path.lstat

            def metadata(path):
                if path == unsafe:
                    class Link:
                        st_mode = stat.S_IFLNK | 0o777
                        st_file_attributes = 0
                    return Link()
                if path == unsafe / 'secret':
                    self.fail('followed unsafe parent')
                return actual_lstat(path)

            with patch.object(Path, 'lstat', metadata):
                self.assertEqual(preflight.path_state(unsafe / 'secret'), 'unsafe_alias')

    def test_refusals_are_structured_and_do_not_create_files(self):
        with self.assertRaisesRegex(preflight.Refused, 'windows_or_macos_required'):
            preflight.collect('Linux')
        with self.assertRaisesRegex(preflight.Refused, 'windows_locator_missing'):
            preflight.collect('Windows', environment={})
        with self.assertRaisesRegex(preflight.Refused, 'profile_locator_not_absolute'):
            preflight.path_state(Path('relative'))

    def test_package_binds_script_revision_and_executable_mode(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'preflight.zip'
            preflight.package(output, 'a' * 40)
            with zipfile.ZipFile(output) as archive:
                self.assertIsNone(archive.testzip())
                prefix = 'Cryptex-LAN-Hazirlik/'
                script = archive.read(prefix + 'ESITLEME-HAZIRLA.command')
                metadata = json.loads(archive.read(prefix + 'package.json'))
                self.assertEqual(metadata['source'], 'a' * 40)
                self.assertEqual(metadata['script_sha256'], hashlib.sha256(script).hexdigest())
                self.assertEqual(archive.getinfo(prefix + 'ESITLEME-HAZIRLA.command').external_attr >> 16, 0o100700)
                self.assertEqual(len(archive.namelist()), 4)
            with self.assertRaises(FileExistsError):
                preflight.package(output, 'a' * 40)
            with self.assertRaisesRegex(preflight.Refused, 'source_revision_invalid'):
                preflight.package(output, 'bad')


if __name__ == '__main__':
    unittest.main()
