"""Exercise publication and retry at the daemon rehearsal's archive boundary."""

from concurrent.futures import ThreadPoolExecutor
import io
import hashlib
from pathlib import Path
import tempfile
from threading import Barrier
import unittest
import zipfile

from daemon_roundtrip import BRAIN_IDENTITY, unpack


FIRST = "canonical/events/ab/00000000000000000000.seg"
SECOND = "canonical/events/ab/00000000000000000001.seg"
IDENTITY = b'{"schema_version":1,"brain_id":"11111111-1111-4111-8111-111111111111","created_at":"2026-01-01T00:00:00Z"}\n'
DIGEST = hashlib.sha256(IDENTITY).digest()


def bundle(prefix, identity=IDENTITY):
    archive = io.BytesIO()
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_STORED) as writer:
        if identity is not None:
            writer.writestr(BRAIN_IDENTITY, identity)
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
            unpack(io.BytesIO(damaged), self.destination, DIGEST)
        self.assertEqual(list(self.root.iterdir()), [])
        unpack(io.BytesIO(intact), self.destination, DIGEST)
        self.assertEqual(self.contents(), {BRAIN_IDENTITY: IDENTITY, FIRST: b"source-first", SECOND: b"source-second"})

    def test_missing_or_different_identity_never_publishes(self):
        for identity in (None, IDENTITY.replace(b"2026-01", b"2026-02")):
            with self.subTest(identity=identity), self.assertRaises(ValueError):
                unpack(io.BytesIO(bundle(b"source", identity)), self.destination, DIGEST)
            self.assertFalse(self.destination.exists())

    def test_existing_empty_and_populated_directories_are_preserved(self):
        self.destination.mkdir()
        for existing in ({}, {"keep": b"original"}):
            with self.subTest(existing=existing):
                for name, contents in existing.items():
                    (self.destination / name).write_bytes(contents)
                with self.assertRaises(OSError):
                    unpack(io.BytesIO(bundle(b"new")), self.destination, DIGEST)
                self.assertEqual(self.contents(), existing)
                self.assertEqual(list(self.root.iterdir()), [self.destination])

    def test_concurrent_restores_publish_one_complete_tree(self):
        start = Barrier(2)

        def restore(prefix):
            archive = io.BytesIO(bundle(prefix))
            start.wait(timeout=10)
            try:
                unpack(archive, self.destination, DIGEST)
            except OSError:
                return None
            return prefix

        with ThreadPoolExecutor(max_workers=2) as pool:
            results = list(pool.map(restore, [b"one", b"two"]))
        self.assertEqual(results.count(None), 1)
        winner = next(prefix for prefix in results if prefix is not None)
        self.assertEqual(self.contents(), {BRAIN_IDENTITY: IDENTITY, FIRST: winner + b"-first", SECOND: winner + b"-second"})
        self.assertEqual(list(self.root.iterdir()), [self.destination])


if __name__ == "__main__":
    unittest.main(verbosity=2)
