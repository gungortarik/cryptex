import copy
import unittest
import json
import tempfile
from pathlib import Path
from unittest.mock import patch
from mac_identity_preflight import CommandResult
from lan_canonical_verification import collect, parse_output
from lan_canonical_verification import BRAIN, MAC, WINDOWS, Refused, compare, validate_receipt, command

RUN = '99999999-9999-4999-8999-999999999999'


def receipt(local=WINDOWS, peer=MAC):
    return dict(schema=1, purpose='cryptex-local-canonical-verification', run_id=RUN, brain_id=BRAIN,
                local_device_id=local, peer_device_id=peer, canonical_set_sha256='a'*64,
                canonical_records=4, canonical_devices=2, signatures_and_chains_verified=True,
                remote_durability_verified=False, network_delivery_verified=False, derived_results_verified=False)


class Receipts(unittest.TestCase):
    def test_signed_mac_progress_is_not_confused_with_the_receipt(self):
        stages = ''.join(json.dumps(dict(m8ProfileStage=stage, phase=phase))+'\n'
                         for stage in ('key_lock', 'key_store') for phase in ('begin', 'complete'))
        record = json.dumps(receipt(MAC, WINDOWS))+'\n'
        self.assertEqual(parse_output(stages+record, dict(run_id=RUN), MAC, WINDOWS), receipt(MAC, WINDOWS))
        for prefix in (stages.replace('complete', 'failed'), record,
                       '{"m8ProfileStage":"key_store","phase":"begin","extra":"body"}\n'):
            with self.assertRaises(Refused):
                parse_output(prefix+record, dict(run_id=RUN), MAC, WINDOWS)

    def test_matching_sets_do_not_assert_transport_or_interruption(self):
        result = compare(receipt(), receipt(MAC, WINDOWS), RUN)
        self.assertTrue(result['same_verified_canonical_set'])
        self.assertFalse(result['network_delivery_verified'])
        self.assertFalse(result['interruption_verified'])
        self.assertFalse(result['remote_protection_claim'])

    def test_mismatch_is_not_convergence(self):
        for key, value in [('canonical_set_sha256', 'b'*64), ('canonical_records', 5), ('canonical_devices', 3)]:
            other = receipt(MAC, WINDOWS)
            other[key] = value
            self.assertFalse(compare(receipt(), other, RUN)['same_verified_canonical_set'])

    def test_wrong_identity_run_unverified_or_extra_data_refuses(self):
        for key, value in [('local_device_id', MAC), ('peer_device_id', WINDOWS), ('run_id', 'old'),
                           ('brain_id', RUN), ('schema', True), ('canonical_records', True),
                           ('signatures_and_chains_verified', False), ('canonical_set_sha256', 'Z'*64),
                           ('network_delivery_verified', True), ('remote_durability_verified', True),
                           ('memory_body', 'must not be accepted')]:
            value_dict = copy.deepcopy(receipt())
            value_dict[key] = value
            with self.subTest(key=key), self.assertRaises(Refused):
                validate_receipt(value_dict, RUN, WINDOWS, MAC)

    def test_exact_arguments_keep_paths_out_of_shell(self):
        args = command('C:/with space/cryptexd.exe', 'C:/runtime space', {'run_id': RUN}, WINDOWS, MAC)
        self.assertEqual(args[0], 'C:/with space/cryptexd.exe')
        self.assertEqual(args[1:4], ['replication', 'verify-local', '--runtime-root'])
        self.assertEqual(args[4], 'C:/runtime space')
        self.assertNotIn('--confirm', args)

    def test_mac_adapter_uses_bounded_runner_output_and_original_identities(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            daemon = folder / 'cryptexd'
            daemon.write_bytes(b'test executable fixture')
            desktop = folder / 'cryptex-desktop'
            plan = dict(run_id=RUN, source_commit='a'*40, source_sha256='b'*64)
            report = {}
            result = CommandResult(0, json.dumps(receipt(MAC, WINDOWS)))
            with patch('os.geteuid', return_value=1000, create=True), \
                    patch('platform.mac_ver', return_value=('26.0', (), 'arm64')), \
                    patch('mac_existing_update.prepare', return_value=desktop) as prepare, \
                    patch('mac_identity_preflight.run_bounded', return_value=result) as runner:
                self.assertEqual(collect(folder, plan, 'Darwin', report)['local_device_id'], MAC)
                self.assertEqual(prepare.call_args.args[2], dict(source_commit='a'*40, source_sha256='b'*64))
                self.assertEqual(prepare.call_args.kwargs, dict(create_launcher=False))
                self.assertIn(MAC, runner.call_args.args[0])
                self.assertEqual(runner.call_args.args[1], 90)
                result.timed_out = True
                with self.assertRaisesRegex(Refused, 'daemon_verification_refused'):
                    collect(folder, plan, 'Darwin', report)


if __name__ == '__main__':
    unittest.main()
