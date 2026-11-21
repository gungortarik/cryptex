"""Install pinned vendor binaries into the hosted runner's temporary tool directory."""
import hashlib
import os
from pathlib import Path
import platform
import subprocess
import sys

CURSOR = "https://downloads.cursor.com/lab/2026.11.19-f005e9d"
AGY = "https://storage.googleapis.com/antigravity-public/antigravity-cli/1.5.14-5302690847260672"
GROK = "https://storage.googleapis.com/grok-build-public-artifacts/cli/grok-1.0.92"
PACKAGES = {
    ("Windows", "amd64"): {
        "cursor.zip": (f"{CURSOR}/windows/x64/agent-cli-package.zip", "a3097c90b528ed9be693d3fe1a018cde5fbc09f1d4a3a2ec50ae57149fc13bb2"),
        "agy.exe": (f"{AGY}/windows-x64/cli_windows_x64.exe", "785548b23242ec9112759bb4b525d94a3d4d2e9868e5cedb9ea34e5ce01dbb00abf5de66c41d8f4903e8a4379445f685483f999ece91867bfbd9d4812760d835"),
        "grok.exe": (f"{GROK}-windows-x86_64.exe", "060e51f93743b540e59c5cf9c81c0ce7b48b61c263f09235d981a6ababb1c9d7"),
    },
    ("Darwin", "arm64"): {
        "cursor.tgz": (f"{CURSOR}/darwin/arm64/agent-cli-package.tar.gz", "16bb33c59733725b466d31f988490de332747f0910d2d64b393c363e510e6c7d"),
        "agy.tgz": (f"{AGY}/darwin-arm/cli_mac_arm64.tar.gz", "26830fd46bfb9200d073cf3e71bf94b707552557b4380ec01afd6e99f3a2b030e65422dab1867f77318078692defc859a7547d17ec4b0b1e9369f13c9c3caea6"),
        "grok": (f"{GROK}-macos-aarch64", "8b5f67b24f2cc9a4341d8e2eb7f67e3228d7d647ed04752a4f932f7133e5ab08"),
    },
}

def main():
    dest = Path(sys.argv[1]).resolve()
    dest.mkdir(parents=True, exist_ok=True)
    for name, (url, digest) in PACKAGES[platform.system(), platform.machine().lower()].items():
        target = dest / name
        subprocess.run(["curl", "--fail", "--silent", "--show-error", "--location",
                        "--retry", "3", "--max-time", "150", url, "--output", target], check=True)
        with target.open("rb") as stream:
            received = hashlib.file_digest(stream, "sha512" if len(digest) == 128 else "sha256").hexdigest()
        if received != digest:
            raise ValueError(f"Wrong vendor archive digest for {name}")
        if name.startswith("cursor."):
            subprocess.run(["tar", "-xf", target, "-C", dest], check=True)
        elif name == "agy.tgz":
            subprocess.run(["tar", "-xf", target, "-C", dest, "antigravity"], check=True)
            (dest / "antigravity").rename(dest / "agy")
        elif os.name != "nt":
            target.chmod(0o755)

if __name__ == "__main__":
    main()
