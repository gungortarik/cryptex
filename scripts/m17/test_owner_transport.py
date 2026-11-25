"""Run the owner-selected transport transition with fresh, synthetic peers."""
import hashlib
import io
import os
import platform
from pathlib import Path
import subprocess
import sys
import tempfile
import urllib.request
import uuid
import zipfile


ARCHIVES = {
    "windows-amd64": (
        "39571e4d0900c2a2cab14c0b170f49751340a869e49734ccc8079d9b98a7974b",
        "3ce7da770f546a8d02b65b2e7ee31f2a3b8b7c3f990ede045590cca4172ecfe1",
    ),
    "macos-arm64": (
        "4a3153cce6f3bdc6290824e409b1622c3b78ad8b8a8b2fc846c0061b61e44d4a",
        "2df075b463231a307c91da657157beaed06383a941ba7ea04d33d536e3c4af38",
    ),
}


def run_trial(native_test=None):
    target = {
        ("win32", "amd64"): "windows-amd64",
        ("darwin", "arm64"): "macos-arm64",
    }[(sys.platform, platform.machine().lower())]
    support = (Path(os.environ["LOCALAPPDATA"]) if sys.platform == "win32"
               else Path.home() / "Library/Application Support")
    parent = support / "Cryptex-Dev/p0-4"
    parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    root = parent / ("m17-transport-" + uuid.uuid4().hex)
    with tempfile.TemporaryDirectory() as tmp:
        binaries = []
        for version, expected in zip(("2.1.5", "2.1.7"), ARCHIVES[target], strict=True):
            stem = f"syncthing-{target}-v{version}"
            name = "syncthing.exe" if sys.platform == "win32" else "syncthing"
            url = f"https://github.com/syncthing/syncthing/releases/download/v{version}/{stem}.zip"
            with urllib.request.urlopen(url, timeout=90) as response:
                data = response.read(64 * 1024 * 1024 + 1)
            if hashlib.sha256(data).hexdigest() != expected:
                raise ValueError(f"Official archive digest mismatch for {stem}")
            binary = Path(tmp) / (version + "-" + name)
            with zipfile.ZipFile(io.BytesIO(data)) as archive:
                binary.write_bytes(archive.read(f"{stem}/{name}"))
            binary.chmod(0o500)
            binaries.append(binary)
        runner = Path(__file__).resolve().parents[2] / "tools/m0-lifecycle-compare/scripts/run-syncthing-upgrade-proof.mjs"
        args = ["node", runner, "--m17-owner-transport", "--root", root,
                "--baseline", binaries[0], "--candidate", binaries[1]]
        if native_test is not None:
            args.extend(["--native-stage-test", native_test])
        subprocess.run(args, check=True, timeout=320)


if __name__ == "__main__":
    run_trial()
