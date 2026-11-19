"""Native receipt publication and restart tests on temporary owner-shaped directories."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock
import uuid

import m9_owner_migration as migration
from lan_transfer_diagnostic import BRAIN, MAC, WINDOWS, Refused, read_metadata


class OwnerMigrationTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.state = Path(temp.name).resolve()
        self.root = self.state / 'runtime'
        self.root.mkdir()
        self.lock = self.root / 'daemon.lock'
        self.lock.touch()
        self.plan = {'run_id': str(uuid.uuid4())}
        self.receipt = dict(schema=1, purpose='cryptex-local-canonical-verification',
                            run_id=self.plan['run_id'], brain_id=BRAIN, local_device_id=WINDOWS,
                            peer_device_id=MAC, canonical_set_sha256=migration.PREVIOUS_DIGEST,
                            canonical_records=16, canonical_devices=2, signatures_and_chains_verified=True,
                            remote_durability_verified=False, network_delivery_verified=False,
                            derived_results_verified=False)
        self.report = {}

    def convert(self):
        if self.lock.is_file():
            self.lock.unlink()
            self.lock.mkdir()
            (self.lock / 'reader-m9.pid').touch()
        return migration.MIGRATED

    def perform(self, receipts, migrate=None):
        migration.transition(self.root, self.state, self.plan, WINDOWS, MAC,
                             Mock(side_effect=receipts), migrate or self.convert, self.report)

    def test_complete_and_repeat(self):
        self.perform([self.receipt, self.receipt])
        self.perform([self.receipt])
        self.assertEqual(read_metadata(self.state / 'verified.json'), self.receipt)
        self.assertEqual(self.report['outcome'], 'migrated_and_verified')

    def test_newer_verified_history_is_preserved(self):
        newer = dict(self.receipt, canonical_records=17, canonical_set_sha256='a' * 64)
        self.perform([newer, newer])
        self.assertEqual(self.report['after'], newer)

    def test_wrong_identity_or_previous_data_stops_before_cutover(self):
        for change in (dict(local_device_id=MAC), dict(canonical_records=15),
                       dict(canonical_set_sha256='f' * 64), dict(signatures_and_chains_verified=False)):
            with self.subTest(change=change), self.assertRaises(Refused):
                self.perform([dict(self.receipt, **change)])
            self.assertTrue(self.lock.is_file())
            self.assertFalse((self.state / 'before.json').exists())

    def test_cutover_without_original_baseline_refused(self):
        self.convert()
        with self.assertRaisesRegex(Refused, 'original_m8_receipt_required'):
            self.perform([])

    def test_interrupted_cutover_reuses_saved_receipt(self):
        def interrupt():
            self.lock.rename(self.root / 'm9-lock.prepare')
            raise OSError('power_loss')

        with self.assertRaises(OSError):
            self.perform([self.receipt], interrupt)
        def resume():
            (self.root / 'm9-lock.prepare').rename(self.lock)
            return self.convert()
        self.perform([self.receipt], resume)
        self.assertEqual(self.report['before'], self.receipt)

    def test_postverification_failure_keeps_baseline(self):
        with self.assertRaisesRegex(Refused, 'signed_history_changed'):
            self.perform([self.receipt, dict(self.receipt, canonical_set_sha256='e' * 64)])
        self.assertFalse((self.state / 'verified.json').exists())
        self.assertEqual(read_metadata(self.state / 'before.json'), self.receipt)
        self.perform([self.receipt])

    def test_concurrent_publication_cannot_replace_receipt(self):
        path = self.state / 'before.json'
        migration.publish(path, self.receipt)
        with self.assertRaises(OSError):
            migration.publish(path, {'replaced': True})
        self.assertEqual(read_metadata(path), self.receipt)

    def test_migration_must_report_disabled_writes_and_publish_layout(self):
        for reply in (migration.MIGRATED.replace('blocked', 'enabled'), migration.MIGRATED):
            with self.subTest(reply=reply), self.assertRaises(Refused):
                self.perform([self.receipt], lambda: reply)
            self.assertFalse((self.state / 'verified.json').exists())

    def test_enrolled_device_check(self):
        identity = self.root / 'identity'
        identity.mkdir()
        (identity / 'brain_identity.json').write_text(json.dumps({'brain_id': BRAIN}))
        (identity / 'local_device.json').write_text(json.dumps({'brain_id': BRAIN, 'device_id': MAC}))
        migration.verify_identity(self.root, MAC)
        with self.assertRaisesRegex(Refused, 'wrong_enrolled_profile'):
            migration.verify_identity(self.root, WINDOWS)
