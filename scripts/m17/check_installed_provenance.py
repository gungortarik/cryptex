"""Compare the installed release resource to the independent CI checkout and its toolchain."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from verify_rust_notices import verify_documents

ROOT = Path(__file__).resolve().parents[2]


def output(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True, timeout=30).strip()


def release_dependencies(triple):
    workspace = json.loads(output("cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"))
    local = {(p["name"], p["version"]) for p in workspace["packages"]}
    tree = output("cargo", "tree", "--locked", "--target", triple, "-p", "cryptex-desktop", "-p", "cryptexd",
                  "--features", "tauri/custom-protocol,cryptexd/m17-owner-transport",
                  "--edges", "normal,build", "--prefix", "none", "--format", "{p}")
    result = set()
    for line in tree.splitlines():
        if not line.strip():
            continue
        name, version, *_ = line.split()
        if not version.startswith("v"):
            raise ValueError("Unrecognized Cargo release graph line")
        result.add((name, version[1:]))
    return result - local


def verify_installed_notices(notices, triple, dependencies):
    if notices["target"] != triple or notices["scope"] != "normal and build dependencies":
        raise ValueError("Installed Rust notices have a different target or scope")
    if verify_documents(notices) != dependencies:
        raise ValueError("Installed Rust notice packages differ from the locked release graph")


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
    notices = json.loads((Path(file).parent / "notices/rust.json").read_bytes())
    verify_installed_notices(notices, triple, release_dependencies(triple))
    print(f"Verified installed Rust notices: {len(notices['packages'])} locked packages, "
          f"{len(notices['documents'])} SHA-256-verified documents.")
    print(f"Verified installed build from clean checkout {commit} for {triple}.")


if __name__ == "__main__":
    main(sys.argv[1])
