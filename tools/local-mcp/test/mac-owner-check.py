"""Fetch verified runtimes into a temporary directory and check discovery only."""
import hashlib
import os
from pathlib import Path
import platform
import subprocess
import sys
import tarfile
import tempfile
import shutil

ROOT = Path(__file__).resolve().parent
AGY = "https://storage.googleapis.com/antigravity-public/antigravity-cli/1.5.14-5302690847260672"
ARCHIVES = {
    "arm64": (
        f"{AGY}/darwin-arm/cli_mac_arm64.tar.gz",
        "26830fd46bfb9200d073cf3e71bf94b707552557b4380ec01afd6e99f3a2b030e65422dab1867f77318078692defc859a7547d17ec4b0b1e9369f13c9c3caea6",
        "8294b7aa9b03997481c06babf1e8b270c859358f27da57a11509afe537ac381d",
    ),
    "x64": (
        f"{AGY}/darwin-x64/cli_mac_x64.tar.gz",
        "1b6a1fc6dbe46ba6d9d0405d1b6095189f35ebfc4c3c33ceb1fd1b6d0c9fd702b52ac10b64b8b52aed0d8d71ae02c5df3cffd5e3e3d37549c2daffcce2a853c7",
        "d1b5e999db158c62fe8f7267a4476b035d8bd93b1a605bac24a3f0dd166e3316",
    ),
}

def fetch_executable(dest, url, digest, member):
    archive = dest.with_suffix(".tgz")
    subprocess.run(["curl", "--fail", "--silent", "--show-error", "--location", "--retry", "2",
                    "--max-time", "180", "--output", archive, url], check=True)
    h = hashlib.new("sha512" if len(digest) == 128 else "sha256")
    with archive.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    if h.hexdigest() != digest:
        raise RuntimeError("Indirilen dosyanin ozeti uyusmuyor")
    with tarfile.open(archive) as tar:
        item = tar.getmember(member)
        if not item.isfile():
            raise ValueError(member)
        with tar.extractfile(item) as source, dest.open("xb") as target:
            shutil.copyfileobj(source, target)
    dest.chmod(0o700)


if __name__ == "__main__":
    if sys.platform != "darwin":
        sys.exit("Bu paket Mac icindir.")
    arch = {"arm64": "arm64", "x86_64": "x64"}[platform.machine()]
    agy_url, agy_hash, node_hash = ARCHIVES[arch]
    with tempfile.TemporaryDirectory(prefix="cryptex-m12-tools-") as temp:
        tools = Path(temp)
        print("Gecici Antigravity ve Node.js hazirlaniyor...", flush=True)
        fetch_executable(tools / "agy", agy_url, agy_hash, "antigravity")
        node = f"node-v24.19.0-darwin-{arch}"
        fetch_executable(tools / "node", f"https://nodejs.org/dist/v24.19.0/{node}.tar.gz",
                         node_hash, node + "/bin/node")
        dest = ROOT.parent
        mode = ["--expect-signed-out"] if sys.argv[1:] == ["--expect-signed-out"] else ["--interactive", "--receipt-dir", str(dest)]
        result = subprocess.run([sys.executable, ROOT / "antigravity-acp.py", tools / "agy",
                                 ROOT / "bridge/stdio.mjs", *mode],
                                env={**os.environ, "PATH": temp + os.pathsep + os.environ["PATH"]})
        sys.exit(result.returncode)
