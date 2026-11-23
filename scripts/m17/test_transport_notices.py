import json
import base64
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from transport_notices import documents_for, main, module_ref


class NoticeTests(unittest.TestCase):
    def test_source_documents_follow_binary_go_version_and_retain_exact_bytes(self):
        checksum = "h1:" + "a" * 43 + "="
        module = {"Path": "example.com/source", "Version": "v1.2.3", "Sum": checksum}
        def upstream(repo, commit, name):
            data = f"example.com/source v1.2.3 {checksum}".encode() if name == "go.sum" else name.encode()
            return f"https://example.invalid/{repo}/{commit}/{name}", data

        with tempfile.TemporaryDirectory() as tmp:
            inspection, result = Path(tmp) / "in", Path(tmp) / "out"
            inspection.write_text(json.dumps({"release": "v2.1.7", "targets": [
                {"target": "macos-arm64", "archive_sha256": "a" * 64, "linked_modules": [module], "go_version": "go1.27.3"}]}))
            with (patch("transport_notices.source_commit", side_effect=["a" * 40, "b" * 40]) as commits,
                  patch("transport_notices.source_file", side_effect=upstream),
                  patch("transport_notices.documents_for", return_value=(module["Path"], module["Version"], checksum, {}))):
                main(inspection, result)
            self.assertEqual(commits.call_args_list[-1].args, ("golang/go", "go1.27.3"))
            record = json.loads(result.read_bytes())
            self.assertEqual(record["missing_module_notices"], ["example.com/source@v1.2.3"])
            self.assertEqual(len(record["upstream_documents"]), 3)
            for url, key in record["upstream_documents"].items():
                data = base64.b64decode(record["documents_base64"][key], validate=True)
                self.assertEqual(hashlib.sha256(data).hexdigest(), key)
                self.assertTrue(url.endswith(data.decode()))

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
