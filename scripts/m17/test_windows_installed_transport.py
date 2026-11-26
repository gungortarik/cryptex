from pathlib import Path
import unittest

from windows_installed_transport import compare_inventory


class RetainedInventoryTests(unittest.TestCase):
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
