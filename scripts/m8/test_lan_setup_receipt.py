import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

from lan_setup_receipt import STAGES, validate, read
from lan_acceptance_preflight import Refused

SOURCE = 'a' * 40
SESSION = 'b2ad8bb6-2d65-4690-b3c6-6a7b3f5adac4'


def success():
    return dict(schema=1, purpose='cryptex-persistent-lan-setup', platform='Darwin',
                pairing_authorized=False, network_delivery_verified=False, convergence_verified=False,
                run_id=SESSION, source_commit=SOURCE, outcome='prepared_local_only',
                signer_certificate_sha256='b'*64, release_inventory_sha256='c'*64,
                first_note_sha256=hashlib.sha256(('Cryptex LAN acceptance ' + SESSION + ': Mac first public note.').encode()).hexdigest(),
                **{k: True for k in STAGES})


class SetupReceiptTests(unittest.TestCase):
    def test_success_is_only_a_bound_setup_observation(self):
        self.assertEqual(validate(success(), SOURCE, SESSION)['outcome'], 'prepared_local_only')
        for key, value in [('source_commit', 'd'*40), ('pairing_authorized', True),
                           ('network_delivery_verified', True), ('convergence_verified', True),
                           ('first_note_sha256', '0'*64), ('schema', True), ('extra', 'private body')]:
            altered = success(); altered[key] = value
            with self.assertRaises(Refused): validate(altered, SOURCE, SESSION)
        for key in STAGES:
            altered = success(); del altered[key]
            with self.assertRaises(Refused): validate(altered, SOURCE, SESSION)

    def test_incomplete_result_is_not_success_and_unknown_diagnostics_refuse(self):
        value = success()
        value.update(outcome='refused_or_incomplete', failed_stage='product_transport_enrollment')
        del value['initialized_startup_restart']
        self.assertEqual(validate(value, SOURCE, SESSION)['outcome'], 'refused_or_incomplete')
        value['error'] = 'unbounded raw command contents'
        with self.assertRaises(Refused): validate(value, SOURCE, SESSION)

    def test_archive_is_bounded_exact_and_duplicate_json_keys_refuse(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'result.zip'
            for contents in ({'result.json': json.dumps(success())},
                             {'../result.json': json.dumps(success())},
                             {'result.json': '{"schema":1,"schema":1}'},
                             {'result.json': 'x'*16385},
                             {'result.json': json.dumps(success()), 'extra': 'x'}):
                with zipfile.ZipFile(path, 'w') as archive:
                    for name, data in contents.items(): archive.writestr(name, data)
                if contents == {'result.json': json.dumps(success())}:
                    self.assertEqual(read(path, SOURCE, SESSION)[0], success())
                else:
                    with self.assertRaises(Refused): read(path, SOURCE, SESSION)


if __name__ == '__main__': unittest.main()
