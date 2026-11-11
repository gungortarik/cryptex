"""Package exact committed signed message-bootstrap experiment, outside Git."""
import argparse
import hashlib
import importlib.util
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
from mac_bootstrap_signed import FILES, SCOPE

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("xpc_packager", Path(__file__).with_name("package-macos-xpc.py"))
packager = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packager)


def build(revision, output=None):
    commit = subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "--verify", revision + "^{commit}"], text=True).strip()
    files = {name: subprocess.check_output(["git", "-C", str(ROOT), "show", f"{commit}:scripts/m8/{name}"]) for name in FILES}
    payload = packager.package_bytes(commit, files, FILES, SCOPE)
    output = (output or Path.home() / "Downloads" / f"cryptex-mac-mesaj-{commit[:8]}.zip").resolve()
    if output == ROOT or ROOT in output.parents:
        raise ValueError("output inside repository")
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("xb") as target:
        target.write(payload)
    print("source_commit=" + commit)
    print("package_sha256=" + hashlib.sha256(payload).hexdigest())
    print("package=" + str(output))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", default="HEAD")
    parser.add_argument("--output", type=Path)
    options = parser.parse_args()
    build(options.revision, options.output)
