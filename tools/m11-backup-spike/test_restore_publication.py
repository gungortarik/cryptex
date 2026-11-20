"""Exercise publication and retry at the daemon rehearsal's archive boundary."""

from concurrent.futures import ThreadPoolExecutor
import io
from pathlib import Path
import tempfile
from threading import Barrier
import unittest
import zipfile

from daemon_roundtrip import unpack


FIRST = "canonical/events/ab/00000000000000000000.seg"
SECOND = "canonical/events/ab/00000000000000000001.seg"


def bundle(prefix):
    archive = io.BytesIO()
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_STORED) as writer:
        writer.writestr(FIRST, prefix + b"-first")
        writer.writestr(SECOND, prefix + b"-second")
    return archive.getvalue()


class RestorePublication(unittest.TestCase):
    def setUp(self):
        sandbox = tempfile.TemporaryDirectory()
        self.addCleanup(sandbox.cleanup)
        self.root = Path(sandbox.name)
        self.destination = self.root / "archive"

    def contents(self):
        return {p.relative_to(self.destination).as_posix(): p.read_bytes()
                for p in self.destination.rglob("*") if p.is_file()}

    def test_late_crc_failure_leaves_destination_absent_and_retry_recovers_both_members(self):
        intact = bundle(b"source")
        damaged = intact.replace(b"source-second", b"broken-second", 1)
        with self.assertRaises(zipfile.BadZipFile):
            unpack(io.BytesIO(damaged), self.destination)
        self.assertEqual(list(self.root.iterdir()), [])
        unpack(io.BytesIO(intact), self.destination)
        self.assertEqual(self.contents(), {FIRST: b"source-first", SECOND: b"source-second"})

    def test_existing_empty_and_populated_directories_are_preserved(self):
        self.destination.mkdir()
        for existing in ({}, {"keep": b"original"}):
            with self.subTest(existing=existing):
                for name, contents in existing.items():
                    (self.destination / name).write_bytes(contents)
                with self.assertRaises(OSError):
                    unpack(io.BytesIO(bundle(b"new")), self.destination)
                self.assertEqual(self.contents(), existing)
                self.assertEqual(list(self.root.iterdir()), [self.destination])

    def test_concurrent_restores_publish_one_complete_tree(self):
        start = Barrier(2)

        def restore(prefix):
            archive = io.BytesIO(bundle(prefix))
            start.wait(timeout=10)
            try:
                unpack(archive, self.destination)
            except OSError:
                return None
            return prefix

        with ThreadPoolExecutor(max_workers=2) as pool:
            results = list(pool.map(restore, [b"one", b"two"]))
        self.assertEqual(results.count(None), 1)
        winner = next(prefix for prefix in results if prefix is not None)
        self.assertEqual(self.contents(), {FIRST: winner + b"-first", SECOND: winner + b"-second"})
        self.assertEqual(list(self.root.iterdir()), [self.destination])


if __name__ == "__main__":
    unittest.main(verbosity=2)
