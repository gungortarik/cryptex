"""Round-trip only the synthetic daemon test's canonical files into private staging."""

from pathlib import Path
import hashlib
import os
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


def pack(source, plain):
    with zipfile.ZipFile(plain, "x") as archive:
        archive.writestr(BRAIN_IDENTITY, brain_identity_bytes(source))
        for path in sorted((source / "canonical").rglob("*")):
            name = path.relative_to(source).as_posix()
            if path.is_symlink() or path.is_junction():
                raise ValueError("Unexpected link in source fixture")
            if path.is_dir() or name == "canonical/events/inventory.lock" or DEVICE_RUNTIME.fullmatch(name):
                continue
            if not path.is_file() or not MEMBER.fullmatch(name):
                raise ValueError(f"Unexpected file in synthetic fixture: {name}")
            archive.write(path, name)


def write_generation(source, writer, backup):
    with tempfile.TemporaryDirectory(prefix="cryptex-m11-pack-") as temp:
        plain = Path(temp) / "source.zip"
        pack(source, plain)
        recipient = writer / "recovery.pub"
        with tempfile.TemporaryDirectory(dir=backup.parent) as staging:
            encrypted = Path(staging) / "backup.pgp"
            envelope("sign-encrypt", [recipient, writer / "writer.sec"], plain, encrypted)
            publish_exclusive(encrypted, backup)


def seal_inventory(recipient, payload, inventory, backup):
    with tempfile.TemporaryDirectory() as temp:
        inner = Path(temp) / "inner.zip"
        with zipfile.ZipFile(inner, "x") as archive:
            archive.write(payload, "content.zip")
            archive.write(inventory, "inventory.json")
        with tempfile.TemporaryDirectory(dir=backup.parent) as staging:
            encrypted = Path(staging) / "backup.pgp"
            installed = os.environ.get("CRYPTEX_M11_TEST_INSTALLED_CODEC")
            if installed:
                from packaged_codec import encrypt
                encrypt(Path(installed), recipient, inner, encrypted)
            else:
                envelope("encrypt", [recipient], inner, encrypted)
            publish_exclusive(encrypted, backup)


def open_payload(kit, backup, destination):
    """Leave authenticated envelope contents only in the caller's private staging.

    Before extracting a v2 payload the daemon must authenticate its inventory.
    """
    with tempfile.TemporaryDirectory(dir=destination.parent) as temp:
        work = Path(temp)
        stage = work / "payload"
        stage.mkdir()
        plain, mode = work / "decrypted.zip", work / "mode"
        signer = kit / "writer.pub"
        envelope("decrypt-backup", [kit / "recovery.sec", signer if signer.exists() else "-", mode], backup, plain)
        with zipfile.ZipFile(plain) as archive:
            names = archive.namelist()
            if mode.read_bytes() == b"inventory":
                if sorted(names) != ["content.zip", "inventory.json"]:
                    raise ValueError("Missing signed inventory")
                for name, limit in (("inventory.json", 4 * MIB), ("content.zip", 128 * MIB)):
                    meta = archive.getinfo(name)
                    if meta.compress_type != zipfile.ZIP_STORED or not 0 < meta.file_size <= limit:
                        raise ValueError("Invalid inner backup member")
                    with archive.open(meta) as inp, (stage / name).open("xb") as out:
                        shutil.copyfileobj(inp, out, MIB)
            elif mode.read_bytes() != b"legacy" or any(name in ("inventory.json", "content.zip") for name in names):
                raise ValueError("Unexpected backup format")
        if not (stage / "inventory.json").exists():
            plain.rename(stage / "content.zip")
        publish_exclusive(stage, destination)


def unpack_checked(payload, destination, checkpoint):
    unpack(payload, destination, checkpoint.read_bytes())


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
               "seal-kit": seal, "open-kit": unseal, "pack": pack, "seal-inventory": seal_inventory,
               "open-payload": open_payload, "unpack-checked": unpack_checked}
    try:
        actions[command](*(Path(arg) for arg in args))
    except (OSError, ValueError, subprocess.SubprocessError, zipfile.BadZipFile):
        sys.exit("M11 archive operation failed")
