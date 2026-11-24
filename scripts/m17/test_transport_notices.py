import json
import base64
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from transport_notices import documents_for, main, module_ref, release_notice


class NoticeTests(unittest.TestCase):
    def test_distribution_copy_matches_target_and_keeps_original_bytes(self):
        upstream = b"upstream terms\r\n"
        upstream_record = {"contents_base64": base64.b64encode(upstream).decode(),
                           "bytes": len(upstream), "sha256": hashlib.sha256(upstream).hexdigest()}
        target = {"target": "macos-arm64", "archive_sha256": "a" * 64, "executable_sha256": "b" * 64,
                  "upstream_notices": {"LICENSE.txt": upstream_record, "AUTHORS.txt": upstream_record}}
        data = "© example\r\n".encode()
        key = hashlib.sha256(data).hexdigest()
        texts = {key: base64.b64encode(data).decode()}
        mods = [{"module": name, "version": "v1.0.0", "targets": [platform], "documents": {"LICENSE": key}}
                for name, platform in (("example/mac", "macos-arm64"), ("example/win", "windows-amd64"))]
        output = release_notice("v2.1.7", target, mods, texts, [])
        for needle in (b"Syncthing v2.1.7/LICENSE.txt", b"Syncthing v2.1.7/AUTHORS.txt",
                       b"example/mac@v1.0.0/LICENSE", b"Executable SHA-256: " + b"b" * 64, data):
            self.assertIn(needle, output)
        self.assertNotIn(b"example/win", output)
        self.assertEqual(output.count(upstream), 1)
        texts[key] = base64.b64encode(b"changed notice").decode()
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            release_notice("v2.1.7", target, mods, texts, [])

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

    def test_distribution_requires_coverage_for_both_targets_and_keeps_the_diagnostic(self):
        checksum = "h1:" + "a" * 43 + "="
        mac, win = "example.org/mac", "example.org/win"
        version = "v1.2.3"
        tag = "v2.1.7"
        data = b"upstream"
        record = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(),
                  "contents_base64": base64.b64encode(data).decode()}
        targets = [{"target": platform, "archive_sha256": "a" * 64, "executable_sha256": "b" * 64,
                    "linked_modules": [{"Path": path, "Version": version, "Sum": checksum}],
                    "go_version": "go1.27.3", "upstream_notices": {"LICENSE.txt": record, "AUTHORS.txt": record}}
                   for platform, path in [("macos-arm64", mac), ("windows-amd64", win)]]

        def source(repo, commit, filename):
            sums = "".join(f"{path} {version} {checksum}\n" for path in [mac, win])
            return f"{repo}/{commit}/{filename}", sums.encode() if filename == "go.sum" else data

        def collect(args):
            path, v, h, _ = args
            return path, v, h, {"LICENSE": b"terms"} if path == mac else windows_files

        with (tempfile.TemporaryDirectory() as tmp,
              patch("transport_notices.source_commit", return_value="a" * 40),
              patch("transport_notices.source_file", side_effect=source),
              patch("transport_notices.documents_for", side_effect=collect)):
            inspection, report, distribution = (Path(tmp) / name for name in ["input.json", "report.json", "notices"])
            inspection.write_text(json.dumps({"release": tag, "targets": targets}), encoding="utf-8")
            for windows_files in ({}, {"AUTHORS": b"Names", "PATENTS": b"Grant"}):
                with self.assertRaisesRegex(ValueError, "example.org/win@v1.2.3 on windows-amd64"):
                    main(inspection, report, distribution)
                self.assertEqual(json.loads(report.read_bytes())["missing_module_notices"], [f"{win}@{version}"])
                self.assertFalse(distribution.exists())
            windows_files = {"LICENSE": b"Win terms", "PATENTS": b"Win grant\r\n"}
            main(inspection, report, distribution)
            self.assertEqual(json.loads(report.read_bytes())["missing_module_notices"], [])
            win_notice = (distribution / "Syncthing-windows-amd64-NOTICES.txt").read_bytes()
            mac_notice = (distribution / "Syncthing-macos-arm64-NOTICES.txt").read_bytes()
            self.assertIn(b"Win grant\r\n", win_notice)
            self.assertNotIn(b"Win grant", mac_notice)

    def test_capture_original_nested_and_root_notices_only_after_checksum_verification(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "LICENSE").write_bytes(b"terms\r\n")
            (root / "embedded").mkdir()
            (root / "embedded/NOTICE.txt").write_bytes(b"attribution")
            (root / "PATENTS").write_bytes(b"grant")
            metadata = {"Path": "example.com/source", "Version": "v1.2.3", "Sum": "h1:test", "Dir": directory}
            with patch("transport_notices.subprocess.check_output", return_value=json.dumps(metadata)):
                args = ("example.com/source", "v1.2.3", "h1:test", directory)
                self.assertEqual(documents_for(args)[3], {"LICENSE": b"terms\r\n", "PATENTS": b"grant",
                                                         "embedded/NOTICE.txt": b"attribution"})
                with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                    documents_for((args[0], args[1], "h1:changed", directory))
                (root / "LICENSE").write_bytes(b" \t\r\n")
                with self.assertRaisesRegex(ValueError, "Empty notice text"):
                    documents_for(args)
