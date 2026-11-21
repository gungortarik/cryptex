"""Portable, disposable M11 Kit rehearsal. The unlock credential is held separately."""

import base64
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zipfile

from rehearse import NODE
from restore_publication import publish_exclusive


MEMBERS = ("recovery.sec", "recovery.pub", "writer.pub", "brain-identity.sha256", "bootstrap.pk8")
SELECTION = "selected-backup.json"
LIMIT = 128 * 1024


def public_recipient(members):
    if len(members["brain-identity.sha256"]) != hashlib.sha256().digest_size:
        raise ValueError("Invalid Brain identity digest")
    pair = [members[name].decode("ascii") for name in ("recovery.sec", "recovery.pub")]
    keys = [*pair, base64.b64encode(members["bootstrap.pk8"]).decode("ascii")]
    return subprocess.run(NODE + ["kit-public-recipient"], input=json.dumps(keys).encode("ascii"),
                          stdout=subprocess.PIPE, check=True, timeout=90).stdout


def seal(source, password, destination):
    contents = {}
    bundle = io.BytesIO()
    with zipfile.ZipFile(bundle, "w", compression=zipfile.ZIP_STORED) as archive:
        members = (*MEMBERS, SELECTION) if (source / SELECTION).exists() else MEMBERS
        for name in members:
            with (source / name).open("rb") as item:
                value = item.read(LIMIT + 1)
            if not value or len(value) > LIMIT:
                raise ValueError("Invalid Kit member size")
            contents[name] = value
            archive.writestr(name, value)
    if bundle.tell() > LIMIT:
        raise ValueError("Oversized Kit")
    public_recipient(contents)
    with tempfile.TemporaryDirectory(dir=destination.parent) as temp:
        staged = Path(temp) / "kit.pgp"
        with staged.open("xb") as out:
            subprocess.run(NODE + ["lock-kit", password], input=bundle.getvalue(),
                           stdout=out, check=True, timeout=90)
            out.flush()
            os.fsync(out.fileno())
        publish_exclusive(staged, destination)


def read_kit(source, password):
    with source.open("rb") as src:
        result = subprocess.run(NODE + ["unlock-kit", password], stdin=src,
                                stdout=subprocess.PIPE, check=True, timeout=90)
    if len(result.stdout) > LIMIT:
        raise ValueError("Oversized Kit")
    with zipfile.ZipFile(io.BytesIO(result.stdout)) as archive:
        entries = archive.infolist()
        if (sorted(archive.namelist()) not in (sorted(MEMBERS), sorted((*MEMBERS, SELECTION)))
                or any(e.compress_type != zipfile.ZIP_STORED or e.file_size == 0 for e in entries)
                or sum(e.file_size for e in entries) > LIMIT):
            raise ValueError("Unexpected Kit inventory")
        contents = {name: archive.read(name) for name in archive.namelist()}
        return contents, public_recipient(contents)


def publish_members(contents, destination):
    with tempfile.TemporaryDirectory(dir=destination.parent) as temp:
        tree = Path(temp) / "result"
        tree.mkdir(mode=0o700)
        for name, value in contents.items():
            with (tree / name).open("xb") as output:
                if os.name != "nt":
                    os.fchmod(output.fileno(), 0o600)
                output.write(value)
                output.flush()
                os.fsync(output.fileno())
        publish_exclusive(tree, destination)


def unseal(source, password, destination):
    contents, _ = read_kit(source, password)
    publish_members(contents, destination)


def export_recipient(source, password, destination):
    contents, public = read_kit(source, password)
    publish_members({"recovery.pub": public, "brain-identity.sha256": contents["brain-identity.sha256"]},
                    destination)
    return hashlib.sha256(public).hexdigest()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Export the public backup certificate from an offline Cryptex Recovery Kit.")
    parser.add_argument("kit", type=Path, help="encrypted Recovery Kit")
    parser.add_argument("unlock_file", type=Path, help="separately held Kit unlock credential file")
    parser.add_argument("public_folder", type=Path, help="new directory for the two public files")
    args = parser.parse_args()
    try:
        digest = export_recipient(args.kit, args.unlock_file, args.public_folder)
    except (OSError, ValueError, subprocess.SubprocessError, zipfile.BadZipFile):
        parser.exit(1, "Could not verify and export the Recovery Kit certificate.\n")
    print(f"Recovery certificate SHA-256: {digest}")
