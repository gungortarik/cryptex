from pathlib import Path
import tempfile
import unittest

from windows_installed_transport import PEER_STREAM, compare_inventory, expected_event_inventory, inventory


class RetainedInventoryTests(unittest.TestCase):
    def test_only_the_approved_peers_new_empty_lock_matches(self):
        with tempfile.TemporaryDirectory() as folder:
            events = Path(folder)
            segment = events / "note.seg"
            segment.write_bytes(b"original")
            before = inventory(events)
            (events / PEER_STREAM).mkdir()
            lock = events / PEER_STREAM / "writer.lock"
            with self.assertRaisesRegex(RuntimeError, "not initialized"):
                expected_event_inventory(before, events)
            lock.touch()
            expected = expected_event_inventory(before, events)
            self.assertEqual(expected, inventory(events))
            self.assertEqual(expected_event_inventory(expected, events), expected)
            lock.write_bytes(b"unexpected")
            with self.assertRaisesRegex(RuntimeError, "empty"):
                expected_event_inventory(before, events)
            damaged_baseline = inventory(events)
            lock.write_bytes(b"")
            with self.assertRaisesRegex(RuntimeError, "contained data"):
                expected_event_inventory(damaged_baseline, events)
            (events / "other-peer").mkdir()
            (events / "other-peer/writer.lock").touch()
            segment.write_bytes(b"changed")
            self.assertEqual(compare_inventory("events", expected, inventory(events)), [
                "events.added.writer.lock=1", "events.changed.canonical_segment=1",
            ])

    def test_reports_every_change_without_identifiers_or_contents(self):
        event = Path("synthetic-device/0000.seg")
        unknown = Path("private-test-name")
        lock = Path("inventory.lock")
        before = {event: b"before", unknown: b"secret"}
        after = {event: b"after", lock: b""}
        self.assertEqual(compare_inventory("events", before, before), [])
        self.assertEqual(compare_inventory("events", before, after), [
            "events.added.inventory.lock=1",
            "events.changed.canonical_segment=1",
            "events.missing.other=1",
        ])


if __name__ == "__main__":
    unittest.main()
