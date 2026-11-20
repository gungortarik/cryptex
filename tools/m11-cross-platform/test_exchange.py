import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from exchange import fetch


class FetchTests(unittest.TestCase):
    def fetch_after(self, first_error):
        attempts = []
        listing = {"artifacts": [{"name": "m11-backup-macos-1", "expired": False}]}

        def cli(args, **kwargs):
            if args[1] == "api":
                return subprocess.CompletedProcess(args, 0, json.dumps(listing).encode())
            folder = Path(args[-1])
            folder.mkdir()
            attempts.append(folder)
            if len(attempts) == 1:
                (folder / "partial").write_bytes(b"interrupted")
                return subprocess.CompletedProcess(args, 1, stderr=first_error)
            self.assertFalse(attempts[0].exists())
            (folder / "backup.pgp").write_bytes(b"complete")
            return subprocess.CompletedProcess(args, 0)

        env = {"GITHUB_RUN_ATTEMPT": "1", "GITHUB_RUN_ID": "42", "GITHUB_REPOSITORY": "test/example"}
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, env), \
             patch("exchange.subprocess.run", side_effect=cli), patch("exchange.time.sleep"):
            root = Path(directory)
            try:
                fetch(root, "backup", "macos")
            except RuntimeError:
                self.assertEqual(len(attempts), 1)
                self.assertFalse((root / "incoming").exists())
                raise
            self.assertEqual(list((root / "incoming").iterdir()), [root / "incoming/backup.pgp"])
            self.assertEqual(len(attempts), 2)

    def test_retry_starts_from_empty_directory(self):
        self.fetch_after(b"connection reset")

    def test_failed_digest_is_terminal(self):
        with self.assertRaisesRegex(RuntimeError, "integrity"):
            self.fetch_after(b"digest mismatch")


if __name__ == "__main__":
    unittest.main()
