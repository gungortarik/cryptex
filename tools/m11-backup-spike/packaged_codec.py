"""Assemble the fixed public-key encryptor for disposable installations."""
import hashlib
import json
from pathlib import Path
import shutil
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
    archive = SPIKE / "node_modules/fflate"
    inputs = {"runtime": node, "entry": ROOT / "services/cryptexd/backup-codec/encrypt.cjs",
              "restore": ROOT / "services/cryptexd/backup-codec/restore.cjs",
              "zip": archive / "umd/index.js",
              "openpgp": library / "dist/node/openpgp.min.cjs"}
    pins = {}
    for role, source in inputs.items():
        with source.open("rb") as package_input:
            pins[role] = hashlib.file_digest(package_input, "sha256").hexdigest()
        shutil.copy2(source, directory / ("fflate.cjs" if role == "zip" else source.name))
    shutil.copy2(archive / "LICENSE", directory / "fflate-LICENSE")
    shutil.copy2(library / "LICENSE", directory / "OpenPGP-LICENSE")
    return pins
