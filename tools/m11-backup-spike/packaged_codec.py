"""Assemble and exercise the fixed public-key encryptor in disposable installs."""
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess

SPIKE = Path(__file__).resolve().parent
ROOT = SPIKE.parents[1]


def stage(directory):
    node = Path(shutil.which("node"))
    version = subprocess.check_output([node, "--version"], text=True).strip().lstrip("v")
    if version != (ROOT / ".node-version").read_text().strip():
        raise RuntimeError("Unexpected package runtime version")
    library = SPIKE / "node_modules/openpgp"
    locked = json.loads((SPIKE / "package-lock.json").read_bytes())["packages"]["node_modules/openpgp"]["version"]
    if json.loads((library / "package.json").read_bytes())["version"] != locked:
        raise RuntimeError("Unexpected installed OpenPGP library version")
    directory.mkdir(parents=True, exist_ok=True)
    shutil.copy2(node, directory / node.name)
    shutil.copy2(ROOT / "services/cryptexd/backup-codec/encrypt.cjs", directory)
    shutil.copy2(library / "dist/node/openpgp.min.cjs", directory)
    shutil.copy2(library / "LICENSE", directory / "OpenPGP-LICENSE")


def encrypt(directory, recipient, archive, output):
    node = directory / ("node.exe" if os.name == "nt" else "node")
    command = [node, "--permission", "--no-addons",
               "--allow-fs-read=" + str(directory / "openpgp.min.cjs"), directory / "encrypt.cjs"]
    environment = {key: os.environ[key] for key in ("SystemRoot", "WINDIR") if key in os.environ}
    certificate = recipient.read_bytes()
    with output.open("xb") as out, archive.open("rb") as inp:
        with subprocess.Popen(command, cwd=directory, env=environment, stdin=subprocess.PIPE, stdout=out) as proc:
            try:
                with proc.stdin:
                    proc.stdin.write(struct.pack(">I", len(certificate)))
                    proc.stdin.write(certificate)
                    shutil.copyfileobj(inp, proc.stdin, 1024 * 1024)
                if proc.wait(timeout=90) != 0:
                    raise ValueError("Packaged backup encryptor failed")
            finally:
                if proc.poll() is None:
                    proc.kill()
