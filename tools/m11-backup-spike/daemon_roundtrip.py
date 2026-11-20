"""Round-trip only the synthetic daemon test's canonical files into private staging."""

from pathlib import Path
import hashlib
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

from rehearse import MIB, NODE
from restore_publication import publish_exclusive
from recovery_kit import seal, unseal


MEMBER = re.compile(r"canonical/(?:events/(?:[0-9a-f]{2}){1,128}/[0-9]{20}\.(?:seg|open)|blobs/[0-9a-f]{64})")
DEVICE_RUNTIME = re.compile(r"canonical/events/[0-9a-f]+/(?:writer_tip\.json|writer\.lock|[0-9]{20}\.open\.lock)")
BRAIN_IDENTITY = "identity/brain_identity.json"


def brain_identity_bytes(source):
    path = source / BRAIN_IDENTITY
    if (path.parent.is_symlink() or path.parent.is_junction() or path.is_symlink()
            or not path.is_file()):
        raise ValueError("Invalid Brain identity file")
    with path.open("rb") as identity:
        data = identity.read(4097)
    if not 0 < len(data) <= 4096:
        raise ValueError("Invalid Brain identity size")
    return data


def envelope(command, keys, source, output):
    with source.open("rb") as inp, output.open("xb") as out:
        subprocess.run(NODE + [command, *keys], stdin=inp, stdout=out, check=True, timeout=90)


def unpack(bundle, destination, expected_identity_digest):
    with zipfile.ZipFile(bundle) as archive:
        members = archive.infolist()
        if (not 0 < len(members) <= 100 or len(set(archive.namelist())) != len(members)
                or sum(item.file_size for item in members) > 32 * MIB):
            raise ValueError("Invalid fixture inventory")
        for item in members:
            if ((item.filename != BRAIN_IDENTITY and not MEMBER.fullmatch(item.filename))
                    or item.compress_type != zipfile.ZIP_STORED):
                raise ValueError("Unexpected canonical member")
        if BRAIN_IDENTITY not in archive.namelist():
            raise ValueError("Missing canonical Brain identity")
        if (archive.getinfo(BRAIN_IDENTITY).file_size > 4096
                or hashlib.sha256(archive.read(BRAIN_IDENTITY)).digest() != expected_identity_digest):
            raise ValueError("Brain identity differs from Recovery Kit")
        with tempfile.TemporaryDirectory(dir=destination.parent, prefix=".m11-restore-") as temp:
            tree = Path(temp) / "tree"
            for item in members:
                dest = tree / item.filename
                dest.parent.mkdir(parents=True, exist_ok=True)
                with dest.open("xb") as out, archive.open(item) as inp:
                    if item.filename.startswith("canonical/blobs/"):
                        digest = hashlib.sha256()
                        while chunk := inp.read(MIB):
                            digest.update(chunk)
                            out.write(chunk)
                        if digest.hexdigest() != dest.name:
                            raise ValueError("Attachment content differs from its canonical name")
                    else:
                        shutil.copyfileobj(inp, out, MIB)
            publish_exclusive(tree, destination)


def write_generation(source, writer, backup):
    with tempfile.TemporaryDirectory(prefix="cryptex-m11-pack-") as temp:
        plain = Path(temp) / "source.zip"
        with zipfile.ZipFile(plain, "x") as archive:
            archive.writestr(BRAIN_IDENTITY, brain_identity_bytes(source))
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
        with (kit / "brain-identity.sha256").open("xb") as checkpoint:
            checkpoint.write(hashlib.sha256(brain_identity_bytes(source)).digest())
        write_generation(source, writer, backup)


def recover(kit, destination, backup):
    recovery, signer, recipient = (kit / name for name in ("recovery.sec", "writer.pub", "recovery.pub"))
    with tempfile.TemporaryDirectory(prefix="cryptex-m11-unpack-") as temp:
        work = Path(temp)
        decoded = work / "verified.zip"
        envelope("decrypt-verify", [recovery, signer], backup, decoded)
        unsigned = work / "unsigned.pgp"
        envelope("encrypt", [recipient], decoded, unsigned)
        for attempt, sample, verifier in (("unsigned", unsigned, signer),
                                          ("wrong-signer", backup, recipient)):
            try:
                envelope("decrypt-verify", [recovery, verifier], sample, work / attempt)
            except subprocess.CalledProcessError:
                continue
            raise AssertionError("Accepted unauthorized backup")
        unpack(decoded, destination, (kit / "brain-identity.sha256").read_bytes())


if __name__ == "__main__":
    command, *args = sys.argv[1:]
    actions = {"prepare": prepare, "recover": recover, "write-generation": write_generation,
               "seal-kit": seal, "open-kit": unseal}
    try:
        actions[command](*(Path(arg) for arg in args))
    except (OSError, ValueError, subprocess.SubprocessError, zipfile.BadZipFile):
        sys.exit("M11 archive operation failed")
