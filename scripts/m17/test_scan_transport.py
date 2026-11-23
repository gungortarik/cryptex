import json
import unittest
from scan_transport import findings


class ScanTests(unittest.TestCase):
    def test_preserves_deduplicated_advisories(self):
        data = [{"config": {"scan_mode": "binary"}}, {"SBOM": {"modules": [{"path": "stdlib"}]}},
                {"finding": {"osv": "GO-1"}}, {"finding": {"osv": "GO-1"}}]
        result = findings("  \n".join(map(json.dumps, data)))
        self.assertEqual(result["findings"], ["GO-1"])

    def test_missing_inventory_and_nonbinary_scans_are_incomplete(self):
        for raw in ["", '{"config":{"scan_mode":"binary"}}', '{"config":{"scan_mode":"source"}} {"SBOM":{"modules":[{}]}}']:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                findings(raw)
