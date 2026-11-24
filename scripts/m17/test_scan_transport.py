import base64
import json
import hashlib
import io
import unittest
import zipfile
from scan_transport import findings, upstream_notices


class ScanTests(unittest.TestCase):
    def test_inventory_hashes_original_notice_bytes_per_platform(self):
        with zipfile.ZipFile(io.BytesIO(), "w") as archive:
            for name, payload in [("AUTHORS.txt", b"contributors\r\n"), ("LICENSE.txt", b"terms\r\n")]:
                archive.writestr("release/" + name, payload)
            self.assertEqual(upstream_notices(archive, "release")["LICENSE.txt"],
                             {"bytes": 7, "sha256": hashlib.sha256(b"terms\r\n").hexdigest(),
                              "contents_base64": base64.b64encode(b"terms\r\n").decode("ascii")})

    def test_notice_must_belong_to_the_inspected_release(self):
        with zipfile.ZipFile(io.BytesIO(), "w") as archive:
            archive.writestr("other/LICENSE.txt", b"terms")
            with self.assertRaisesRegex(ValueError, "LICENSE.txt"):
                upstream_notices(archive, "release")

    def test_preserves_deduplicated_advisories(self):
        data = [{"config": {"scan_mode": "binary"}}, {"SBOM": {"modules": [{"path": "stdlib"}]}},
                {"finding": {"osv": "GO-1"}}, {"finding": {"osv": "GO-1"}}]
        result = findings("  \n".join(map(json.dumps, data)))
        self.assertEqual(result["findings"], ["GO-1"])

    def test_missing_inventory_and_nonbinary_scans_are_incomplete(self):
        for raw in ["", '{"config":{"scan_mode":"binary"}}', '{"config":{"scan_mode":"source"}} {"SBOM":{"modules":[{}]}}']:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                findings(raw)
