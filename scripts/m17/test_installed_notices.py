import copy
import hashlib
import unittest

from check_installed_provenance import verify_installed_notices


class InstalledNoticesTests(unittest.TestCase):
    def test_preserves_target_graph_versions_and_embedded_document_bytes(self):
        text = hashlib.sha256(b"notice").hexdigest()
        binary = hashlib.sha256(b"\xff").hexdigest()
        target = "x86_64-pc-windows-msvc"
        graph = {("crate", "1.0.0"), ("crate", "2.0.0")}
        notices = {"scope": "normal and build dependencies", "target": target,
                   "packages": [{"name": n, "version": v, "documents": [{"sha256": doc}]}
                                for (n, v), doc in zip(sorted(graph), [text, binary])],
                   "documents": {text: {"text": "notice"}, binary: {"base64": "/w=="}}}
        verify_installed_notices(notices, target, graph)
        for wrong_graph in (graph - {("crate", "1.0.0")}, graph | {("other", "1.0.0")}):
            with self.assertRaisesRegex(ValueError, "locked release graph"):
                verify_installed_notices(notices, target, wrong_graph)
        with self.assertRaisesRegex(ValueError, "different target"):
            verify_installed_notices(notices, "aarch64-apple-darwin", graph)
        changed = copy.deepcopy(notices)
        changed["documents"][binary]["base64"] = "AA=="
        with self.assertRaisesRegex(ValueError, "digest"):
            verify_installed_notices(changed, target, graph)
        changed = copy.deepcopy(notices)
        changed["packages"].append(changed["packages"][0])
        with self.assertRaisesRegex(ValueError, "duplicate"):
            verify_installed_notices(changed, target, graph)
        del notices["documents"][text]
        with self.assertRaisesRegex(ValueError, "references"):
            verify_installed_notices(notices, target, graph)


if __name__ == "__main__":
    unittest.main()
