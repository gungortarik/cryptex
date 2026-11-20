"""Round-trip only the synthetic daemon test's canonical files into private staging."""

from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

from rehearse import MIB, NODE
from restore_publication import publish_tree


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
            publish_tree(tree, destination)


def prepare(source, kit):
    kit.mkdir()
    with tempfile.TemporaryDirectory(prefix="cryptex-m11-pack-") as temp:
        work = Path(temp)
        # The recipient and authorized backup signer have independent fixture keys.
        def keypair(name):
            public, private = work / (name + ".pub"), work / (name + ".sec")
            subprocess.run(NODE + ["keygen", public, private], check=True, timeout=30)
            return public, private

        recipient, recovery = keypair("recovery")
        signer, signing = keypair("writer")
        plain = work / "source.zip"
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
        encrypted = work / "backup.pgp"
        envelope("sign-encrypt", [recipient, signing], plain, encrypted)
        # Also encrypt the legitimate inventory without an authorized signature.
        unsigned = work / "unsigned.pgp"
        envelope("encrypt", [recipient], plain, unsigned)
        plain.unlink()
        signing.unlink()
        for asset in (recipient, recovery, signer, encrypted, unsigned):
            shutil.move(asset, kit / asset.name)


def recover(kit, destination):
    recovery, signer, recipient = (kit / name for name in ("recovery.sec", "writer.pub", "recovery.pub"))
    encrypted, unsigned = kit / "backup.pgp", kit / "unsigned.pgp"
    with tempfile.TemporaryDirectory(prefix="cryptex-m11-unpack-") as temp:
        work = Path(temp)
        for attempt, backup, verifier in (("unsigned", unsigned, signer),
                                          ("wrong-signer", encrypted, recipient)):
            try:
                envelope("decrypt-verify", [recovery, verifier], backup, work / attempt)
            except subprocess.CalledProcessError:
                continue
            raise AssertionError("Accepted unauthorized backup")
        decoded = work / "verified.zip"
        envelope("decrypt-verify", [recovery, signer], encrypted, decoded)
        unpack(decoded, destination)


if __name__ == "__main__":
    command, first, second = sys.argv[1:]
    {"prepare": prepare, "recover": recover}[command](Path(first), Path(second))
