"""Portable, disposable M11 Kit rehearsal. The unlock credential is held separately."""

import io
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


def seal(source, password, destination):
    bundle = io.BytesIO()
    with zipfile.ZipFile(bundle, "w", compression=zipfile.ZIP_STORED) as archive:
        members = (*MEMBERS, SELECTION) if (source / SELECTION).exists() else MEMBERS
        for name in members:
            with (source / name).open("rb") as item:
                value = item.read(LIMIT + 1)
            if not value or len(value) > LIMIT:
                raise ValueError("Invalid Kit member size")
            archive.writestr(name, value)
    if bundle.tell() > LIMIT:
        raise ValueError("Oversized Kit")
    with tempfile.TemporaryDirectory(dir=destination.parent) as temp:
        staged = Path(temp) / "kit.pgp"
        with staged.open("xb") as out:
            subprocess.run(NODE + ["lock-kit", password], input=bundle.getvalue(),
                           stdout=out, check=True, timeout=90)
            out.flush()
            os.fsync(out.fileno())
        publish_exclusive(staged, destination)


def unseal(source, password, destination):
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
        with tempfile.TemporaryDirectory(dir=destination.parent) as temp:
            tree = Path(temp) / "secrets"
            tree.mkdir(mode=0o700)
            for name in archive.namelist():
                with os.fdopen(os.open(tree / name, os.O_CREAT | os.O_EXCL | os.O_WRONLY |
                                      getattr(os, "O_BINARY", 0), 0o600), "wb") as output:
                    output.write(archive.read(name))
            publish_exclusive(tree, destination)
