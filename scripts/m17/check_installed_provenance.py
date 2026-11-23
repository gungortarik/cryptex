"""Compare the installed release resource to the independent CI checkout and its toolchain."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def output(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True, timeout=30).strip()


def main(file):
    record = json.loads(Path(file).read_bytes())
    commit = output("git", "rev-parse", "HEAD")
    triple, = [line.split()[1] for line in output("rustc", "-vV").splitlines() if line.startswith("host:")]
    expected = {
        "schema": 1, "target": triple, "source": {"commit": commit, "clean": True},
        "inputs_sha256": {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
                          for name in ("Cargo.lock", "pnpm-lock.yaml", "rust-toolchain.toml", ".node-version", "package.json")},
        "node": output("node", "--version"), "rustc": output("rustc", "--version"),
    }
    if commit != os.environ["GITHUB_SHA"] or record != expected:
        raise ValueError("Installed build provenance differs from the CI revision, toolchain or lockfiles")
    print(f"Verified installed build from clean checkout {commit} for {triple}.")


if __name__ == "__main__":
    main(sys.argv[1])
