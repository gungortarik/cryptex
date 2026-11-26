import json
from pathlib import Path
import tempfile
import unittest

from paired_release_handoff import verify_installed_journals


def write_receipt(directory, version, *, port=1, schema=1, indent=None):
    binding = json.dumps({"engine_version": version, "ports": {"api": port}}, separators=(",", ":"))
    journal = {"schema": schema, "phase": "committed", "binding": binding}
    (directory / "exchange-binding.json").write_bytes(binding.encode())
    (directory / "exchange-install.json").write_bytes((json.dumps(journal, indent=indent) + "\n").encode())


class InstalledJournalTests(unittest.TestCase):
    def test_only_the_verified_pin_changes_on_disk(self):
        with tempfile.TemporaryDirectory() as folder:
            previous, current = (Path(folder) / name for name in ("old", "new"))
            previous.mkdir()
            current.mkdir()
            write_receipt(previous, "2.1.5")
            write_receipt(current, "2.1.7")
            verify_installed_journals(previous, current)
            for change in ({"port": True}, {"schema": True}, {"port": 2}, {"indent": 2}):
                with self.subTest(change=change):
                    write_receipt(current, "2.1.7", **change)
                    with self.assertRaises(RuntimeError):
                        verify_installed_journals(previous, current)
            write_receipt(current, "2.1.5")
            with self.assertRaises(RuntimeError):
                verify_installed_journals(previous, current)
            write_receipt(current, "2.1.7")
            (current / "exchange-binding.json").write_bytes((previous / "exchange-binding.json").read_bytes())
            with self.assertRaises(RuntimeError):
                verify_installed_journals(previous, current)


if __name__ == "__main__":
    unittest.main()
