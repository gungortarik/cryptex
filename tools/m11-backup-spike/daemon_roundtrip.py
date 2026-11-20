"""Round-trip only the synthetic daemon test's canonical files into private staging."""

from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

from rehearse import MIB, NODE
from restore_publication import publish_exclusive


MEMBER = re.compile(r"canonical/(?:events/(?:[0-9a-f]{2}){1,128}/[0-9]{20}\.(?:seg|open)|blobs/[0-9a-f]{64})")
DEVICE_RUNTIME = re.compile(r"canonical/events/[0-9a-f]+/(?:writer_tip\.json|writer\.lock|[0-9]{20}\.open\.lock)")


def envelope(command, keys, source, output):
    with source.open("rb") as inp, output.open("xb") as out:
        subprocess.run(NODE + [command, *keys], stdin=inp, stdout=out, check=True, timeout=90)


def unpack(bundle, destination):
    with zipfile.ZipFile(bundle) as archive:
        members = archive.infolist()
        if (not 0 < len(members) <= 100 or len(set(archive.namelist())) != len(members)
                or sum(item.file_size for item in members) > 32 * MIB):
            raise ValueError("Invalid fixture inventory")
        for item in members:
            if not MEMBER.fullmatch(item.filename) or item.compress_type != zipfile.ZIP_STORED:
                raise ValueError("Unexpected canonical member")
        with tempfile.TemporaryDirectory(dir=destination.parent, prefix=".m11-restore-") as temp:
            tree = Path(temp) / "tree"
            for item in members:
                dest = tree / item.filename
                dest.parent.mkdir(parents=True, exist_ok=True)
                with dest.open("xb") as out, archive.open(item) as inp:
                    shutil.copyfileobj(inp, out, MIB)
            publish_exclusive(tree, destination)


def write_generation(source, writer, backup, unsigned=None):
    with tempfile.TemporaryDirectory(prefix="cryptex-m11-pack-") as temp:
        plain = Path(temp) / "source.zip"
        with zipfile.ZipFile(plain, "x") as archive:
            for path in sorted((source / "canonical").rglob("*")):
                name = path.relative_to(source).as_posix()
                if path.is_symlink():
                    raise ValueError("Unexpected link in source fixture")
                if path.is_dir() or name == "canonical/events/inventory.lock" or DEVICE_RUNTIME.fullmatch(name):
                    continue
                if not path.is_file() or not MEMBER.fullmatch(name):
                    raise ValueError(f"Unexpected file in synthetic fixture: {name}")
                archive.write(path, name)
        recipient = writer / "recovery.pub"
        if unsigned is not None:
            envelope("encrypt", [recipient], plain, unsigned)
        with tempfile.TemporaryDirectory(dir=backup.parent) as staging:
            encrypted = Path(staging) / "backup.pgp"
            envelope("sign-encrypt", [recipient, writer / "writer.sec"], plain, encrypted)
            publish_exclusive(encrypted, backup)


def prepare(source, kit, backup, writer=None):
    kit.mkdir()
    # The single-generation case discards the backup signer before recovery.
    with tempfile.TemporaryDirectory(prefix="cryptex-m11-writer-") as temp:
        writer = Path(temp) if writer is None else writer
        for public, private in ((writer / "recovery.pub", kit / "recovery.sec"),
                                (kit / "writer.pub", writer / "writer.sec")):
            subprocess.run(NODE + ["keygen", public, private], check=True, timeout=30)
        shutil.copyfile(writer / "recovery.pub", kit / "recovery.pub")
        write_generation(source, writer, backup, unsigned=kit / "unsigned.pgp")


def recover(kit, destination, backup):
    recovery, signer, recipient = (kit / name for name in ("recovery.sec", "writer.pub", "recovery.pub"))
    unsigned = kit / "unsigned.pgp"
    with tempfile.TemporaryDirectory(prefix="cryptex-m11-unpack-") as temp:
        work = Path(temp)
        for attempt, sample, verifier in (("unsigned", unsigned, signer),
                                          ("wrong-signer", backup, recipient)):
            try:
                envelope("decrypt-verify", [recovery, verifier], sample, work / attempt)
            except subprocess.CalledProcessError:
                continue
            raise AssertionError("Accepted unauthorized backup")
        decoded = work / "verified.zip"
        envelope("decrypt-verify", [recovery, signer], backup, decoded)
        unpack(decoded, destination)


if __name__ == "__main__":
    command, *args = sys.argv[1:]
    actions = {"prepare": prepare, "recover": recover, "write-generation": write_generation}
    actions[command](*(Path(arg) for arg in args))
