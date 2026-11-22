"""Native CLI acceptance with a synthetic source and an isolated user profile."""

import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import unittest


BINARY = Path(os.environ["CRYPTEX_INVENTORY_BIN"]).resolve()


def snapshot(root):
    result = {}
    for item in root.rglob("*"):
        stat = item.stat()
        digest = hashlib.sha256(item.read_bytes()).hexdigest() if item.is_file() else None
        result[item.relative_to(root)] = (stat.st_mode, stat.st_mtime_ns, digest)
    return result


class InventoryTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.base = Path(temp.name)
        self.source = self.base / "legacy"
        self.source.mkdir()
        self.profile = self.base / "profile"
        self.profile.mkdir()

    def scan(self, source=None, *, expected=0):
        environment = os.environ.copy()
        for key in ("HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "XDG_DATA_HOME",
                    "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME"):
            environment[key] = str(self.profile)
        run = subprocess.run([BINARY, source or self.source], cwd=self.profile, env=environment,
                             capture_output=True, timeout=30)
        self.assertEqual(run.returncode, expected, (run.stderr, run.stdout))
        self.assertEqual(run.stderr, b"")
        self.assertNotIn(b"private body marker", run.stdout)
        data = json.loads(run.stdout)
        self.assertTrue(all(not Path(row["path"]).is_absolute()
                            for row in data["entries"] + data["findings"]))
        self.assertEqual(data["schema"], "cryptex.cortex.inventory.v1")
        self.assertIs(data["complete"], expected == 0)
        self.assertEqual(list(self.profile.iterdir()), [])
        return data

    def test_read_only_repeatable_inventory(self):
        notes = self.source / "Notes"
        notes.mkdir()
        (self.source / "Empty").mkdir()
        note = notes / "özet.MD"
        note.write_bytes(b"private body marker")
        note.chmod(0o444)
        self.addCleanup(note.chmod, 0o600)
        database = self.source / "legacy.sqlite"
        with sqlite3.connect(database) as db:
            db.execute("create table notes(body)")
            db.execute("insert into notes values ('private body marker')")
        db.close()
        (self.source / "unmapped").write_bytes(b"\x00")
        before = snapshot(self.source)
        result = self.scan()
        self.assertEqual(result, self.scan())
        self.assertEqual(result["findings"], [])
        self.assertEqual(result["entries"], [
            {"kind": "directory", "path": "Empty"},
            {"kind": "directory", "path": "Notes"},
            {"kind": "file", "path": "Notes/özet.MD", "extension": "md", "bytes": note.stat().st_size},
            {"kind": "file", "path": "legacy.sqlite", "extension": "sqlite", "bytes": database.stat().st_size},
            {"kind": "file", "path": "unmapped", "extension": None, "bytes": 1},
        ])
        self.assertEqual(snapshot(self.source), before)

    def test_missing_and_non_directory_source(self):
        file = self.base / "file"
        file.touch()
        for source, issue in [(self.base / "missing", "disappeared"), (file, "not_directory")]:
            with self.subTest(issue=issue):
                result = self.scan(source, expected=2)
                self.assertEqual(result["entries"], [])
                self.assertEqual(result["findings"], [{"path": "", "issue": issue}])

    def test_indirect_root_and_child(self):
        outside = self.base / "outside"
        outside.mkdir()
        (outside / "hidden-note").touch()
        link = self.source / "alias"
        if os.name == "nt":
            subprocess.run(["cmd", "/d", "/c", "mklink", "/J", "alias", "..\\outside"],
                           cwd=self.source, check=True, capture_output=True)
        else:
            link.symlink_to(outside, target_is_directory=True)
        (self.source / "ordinary").touch()
        result = self.scan(expected=2)
        self.assertEqual([item["path"] for item in result["entries"]], ["ordinary"])
        self.assertEqual(result["findings"], [{"path": "alias", "issue": "link_or_reparse_point"}])
        self.assertEqual(self.scan(link, expected=2)["findings"],
                         [{"path": "", "issue": "link_or_reparse_point"}])

    def test_unreadable_directory(self):
        denied = self.source / "denied"
        denied.mkdir()
        (denied / "hidden-note").touch()
        (self.source / "ordinary").touch()
        if os.name == "nt":
            subprocess.run(["icacls", denied, "/deny", "*S-1-1-0:(RD)"], check=True, capture_output=True)
            self.addCleanup(subprocess.run, ["icacls", denied, "/remove:d", "*S-1-1-0"],
                            check=True, capture_output=True)
        else:
            denied.chmod(0)
            self.addCleanup(denied.chmod, 0o700)
        result = self.scan(expected=2)
        self.assertIn("ordinary", [item["path"] for item in result["entries"]])
        self.assertNotIn("denied/hidden-note", [item["path"] for item in result["entries"]])
        self.assertEqual(result["findings"], [{"path": "denied", "issue": "access_denied"}])

    def test_empty_source(self):
        result = self.scan()
        self.assertEqual((result["entries"], result["findings"]), ([], []))

    def test_bounded_depth_reports_unvisited_subtree(self):
        deepest = self.source
        for _ in range(65):
            deepest = deepest / "d"
        deepest.mkdir(parents=True)
        (deepest / "hidden-note").touch()
        result = self.scan(expected=2)
        self.assertEqual(len(result["entries"]), 65)
        self.assertEqual(result["findings"], [
            {"path": deepest.relative_to(self.source).as_posix(), "issue": "depth_limit"}
        ])

    @unittest.skipIf(os.name == "nt", "POSIX filesystem entries")
    def test_special_file(self):
        os.mkfifo(self.source / "pipe")
        result = self.scan(expected=2)
        self.assertEqual(result["entries"], [])
        self.assertEqual(result["findings"], [{"path": "pipe", "issue": "special_file"}])


if __name__ == "__main__":
    unittest.main()
