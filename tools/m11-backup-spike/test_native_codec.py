"""Read native-writer output with the independently pinned OpenPGP.js reader."""

import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile
import zipfile

from daemon_roundtrip import BRAIN_IDENTITY, envelope, unpack
from rehearse import MIB, NODE


def check(executable):
    with tempfile.TemporaryDirectory(prefix="cryptex-m11-codec-") as tmp:
        work = Path(tmp)
        for name in ("kit", "writer", "other"):
            subprocess.run(NODE + ["keygen", work / (name + ".pub"), work / (name + ".sec")],
                           check=True, timeout=30)
        identity = b'{"schema_version":1,"brain_id":"11111111-1111-4111-8111-111111111111","created_at":"2026-01-01T00:00:00Z"}\n'
        pack = "canonical/events/ab/00000000000000000000.seg"
        archive = work / "source.zip"
        with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_STORED) as bundle:
            bundle.writestr(BRAIN_IDENTITY, identity)
            bundle.writestr(pack, b"synthetic interoperability payload" * (MIB // 32))
        expected = hashlib.sha256(archive.read_bytes()).digest()
        encrypted = work / "backup.pgp"
        with archive.open("rb") as source, encrypted.open("xb") as dest:
            subprocess.run([executable, work / "kit.pub", work / "writer.sec"],
                           stdin=source, stdout=dest, check=True, timeout=90)
        archive.unlink()
        (work / "writer.sec").unlink()
        decoded = work / "recovered.zip"
        envelope("decrypt-verify", [work / "kit.sec", work / "writer.pub"], encrypted, decoded)
        if hashlib.sha256(decoded.read_bytes()).digest() != expected:
            raise ValueError("Native encrypted archive changed")
        unpack(decoded, work / "restored", hashlib.sha256(identity).digest())
        for secret, signer in (("other.sec", "writer.pub"), ("kit.sec", "other.pub")):
            try:
                envelope("decrypt-verify", [work / secret, work / signer], encrypted,
                         work / (secret + "-" + signer + ".trial"))
            except subprocess.CalledProcessError:
                continue
            raise ValueError("Wrong recovery credential accepted")
        print("m11_native_writer=passed js_restore=verified wrong_recipient=denied wrong_signer=denied")


if __name__ == "__main__":
    check(Path(sys.argv[1]).resolve(strict=True))
