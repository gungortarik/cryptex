"""M11 container rehearsal. All keys, profiles and backup targets are disposable."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import time
import unittest
import zipfile

from restore_publication import publish_exclusive


HERE = Path(__file__).resolve().parent
FIXTURE = HERE.parents[1] / "crates/events/tests/fixtures/m8-old-reader-v1-v3.json"
NODE = ["node", str(HERE / "envelope.mjs")]
MIB = 1024 * 1024


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def make_source(profile):
    fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
    wire = fixture["baseline_wire"].encode("utf-8")
    stem = fixture["device_id"].encode().hex()
    pack = profile / "canonical/events" / stem / "00000000000000000000.seg"
    pack.parent.mkdir(parents=True)
    pack.write_bytes(struct.pack("<4sHI", b"CXE1", 1, len(wire)) + wire)
    scratch = profile / "blob.tmp"
    with scratch.open("xb") as out:
        for _ in range(64):
            out.write(os.urandom(MIB))
        out.write(os.urandom(123))
    blob = profile / "canonical/blobs" / digest(scratch)
    blob.parent.mkdir()
    scratch.rename(blob)
    (profile / "search.sqlite").write_bytes(b"disposable index sentinel")
    return {
        path.relative_to(profile).as_posix(): (path.stat().st_size, digest(path))
        for path in (pack, blob)
    }


def publish(archive, target, public_key, interrupt=False):
    with archive.open("rb") as source, tempfile.NamedTemporaryFile(
        dir=target.parent, suffix=".partial", delete=False
    ) as out:
        stage = Path(out.name)
        with subprocess.Popen(
            NODE + ["encrypt", public_key], stdin=subprocess.PIPE, stdout=out
        ) as proc:
            try:
                if interrupt:
                    proc.stdin.write(source.read(8 * MIB))
                    proc.stdin.flush()
                    deadline = time.monotonic() + 15
                    while stage.stat().st_size < 65536:
                        if proc.poll() is not None or time.monotonic() > deadline:
                            raise RuntimeError("Interrupted-write checkpoint not reached")
                        time.sleep(0.05)
                    proc.kill()
                else:
                    shutil.copyfileobj(source, proc.stdin, MIB)
                proc.stdin.close()
                if proc.wait(timeout=90):
                    raise ChildProcessError("Writer exited before backup completed")
            finally:
                if proc.poll() is None:
                    proc.kill()
                    proc.wait(timeout=10)
        out.flush()
        os.fsync(out.fileno())
    stage.replace(target)


def restore(backup, key, expected, destination):
    # Under our exclusive temporary parent. Only the independent fixture inventory
    # supplies output paths. No archive-supplied path or metadata is installed.
    if destination.exists():
        raise FileExistsError(destination)
    with tempfile.TemporaryDirectory(dir=destination.parent) as scratch:
        work = Path(scratch)
        archive = work / "decrypted.zip"
        with backup.open("rb") as source, archive.open("xb") as out:
            subprocess.run(NODE + ["decrypt", key], stdin=source, stdout=out,
                           check=True, timeout=90)
        tree = work / "restored"
        with zipfile.ZipFile(archive) as bundle:
            if sorted(bundle.namelist()) != sorted(expected):
                raise ValueError("Unexpected, missing or duplicate archive member")
            for name, (size, checksum) in expected.items():
                member = bundle.getinfo(name)
                if member.file_size != size or member.compress_type != zipfile.ZIP_STORED:
                    raise ValueError("Unexpected member size or encoding")
                path = tree / name
                path.parent.mkdir(parents=True, exist_ok=True)
                with bundle.open(member) as inp, path.open("xb") as out:
                    shutil.copyfileobj(inp, out, MIB)
                if path.stat().st_size != size or digest(path) != checksum:
                    raise ValueError("Restored member failed independent comparison")
        publish_exclusive(tree, destination)


class Rehearsal(unittest.TestCase):
    def test_recover_after_writer_loss(self):
        with tempfile.TemporaryDirectory(prefix="cryptex-m11-") as tmp:
            root = Path(tmp)
            key = root / "offline-secret.asc"
            public = root / "recipient.asc"
            subprocess.run(NODE + ["keygen", public, key], check=True, timeout=30)
            profile = root / "source"
            expected = make_source(profile)
            archive = root / "source.zip"
            with zipfile.ZipFile(archive, "x") as bundle:
                for name in expected:
                    bundle.write(profile / name, arcname=name)
            target = root / "backup.pgp"
            publish(archive, target, public)
            initial = digest(target)
            # Verify the first generation before attempting its replacement.
            restore(target, key, expected, root / "first-rehearsal")
            shutil.rmtree(root / "first-rehearsal")
            with self.assertRaises(ChildProcessError):
                publish(archive, target, public, interrupt=True)
            partial, = root.glob("*.partial")
            self.assertGreater(partial.stat().st_size, 65535)
            self.assertEqual(digest(target), initial)
            # A later backup can complete even with the interrupted staging file present.
            publish(archive, root / "retry.pgp", public)
            # Recover after losing the source, plaintext bundle and writer key.
            shutil.rmtree(profile)
            archive.unlink()
            public.unlink()
            restored = root / "fresh"
            wrong = root / "wrong-secret.asc"
            subprocess.run(NODE + ["keygen", root / "wrong.asc", wrong], check=True, timeout=30)
            damaged = root / "changed.pgp"
            shutil.copyfile(target, damaged)
            with damaged.open("r+b") as out:
                out.seek(-12, os.SEEK_END)
                byte = out.read(1)
                out.seek(-1, os.SEEK_CUR)
                out.write(bytes([byte[0] ^ 1]))
            for sample, secret in ((partial, key), (damaged, key), (target, wrong)):
                with self.subTest(sample=sample.name, secret=secret.name):
                    with self.assertRaises(subprocess.CalledProcessError):
                        restore(sample, secret, expected, restored)
                    self.assertFalse(restored.exists())
            restore(target, key, expected, restored)
            actual = {
                p.relative_to(restored).as_posix(): (p.stat().st_size, digest(p))
                for p in restored.rglob("*") if p.is_file()
            }
            self.assertEqual(actual, expected)
            restore(root / "retry.pgp", key, expected, root / "retry-restored")
            with self.assertRaises(FileExistsError):
                restore(target, key, expected, restored)
            print("Recovered signed frame and 67108987-byte blob after source loss. "
                  "Interrupted writer, altered ciphertext, wrong key and occupied destination checked.")


if __name__ == "__main__":
    unittest.main(verbosity=2)
