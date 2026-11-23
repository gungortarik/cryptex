import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from transport_notices import documents_for, module_ref


class NoticeTests(unittest.TestCase):
    def test_follow_the_version_actually_compiled_for_a_replacement(self):
        dep = {"Path": "example.com/old", "Version": "v1.2.3",
               "Replace": {"Path": "example.com/new", "Version": "v1.4.7"}}
        self.assertEqual(module_ref(dep), ("example.com/new", "v1.4.7"))
        dep["Replace"]["Path"] = "../local"
        with self.assertRaises(ValueError):
            module_ref(dep)

    def test_capture_original_nested_and_root_notices_only_after_checksum_verification(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "LICENSE").write_bytes(b"terms\r\n")
            (root / "embedded").mkdir()
            (root / "embedded/NOTICE.txt").write_bytes(b"attribution")
            metadata = {"Path": "example.com/source", "Version": "v1.2.3", "Sum": "h1:test", "Dir": directory}
            with patch("transport_notices.subprocess.check_output", return_value=json.dumps(metadata)):
                args = ("example.com/source", "v1.2.3", "h1:test", directory)
                self.assertEqual(documents_for(args)[3], {"LICENSE": b"terms\r\n", "embedded/NOTICE.txt": b"attribution"})
                with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                    documents_for((args[0], args[1], "h1:changed", directory))
