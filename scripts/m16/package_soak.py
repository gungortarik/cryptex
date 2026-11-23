"""Create the owner packet from the same test executable used on native CI."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import zipfile

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "scripts/m8"))
from lan_cutoff_packet import MAC_HASH, WIN_HASH, build_runner, check_hash
from mac_transport_package import prepare

SOURCES = (
    "scripts/m16/owner_soak.py", "scripts/m16/sync_stress.py", "scripts/m16/suspend_clock.py",
    "scripts/m9/test_paired_attachment.py", "scripts/m8/lan_cutoff_packet.py",
    "scripts/m8/mac_transport_package.py", "scripts/m8/mac_identity_preflight.py",
    "third_party/syncthing/2.1.5/LICENSE.txt", "third_party/syncthing/2.1.5/AUTHORS.txt",
)


def build(output, windows_artifact):
    system = platform.system()
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip()
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        root = work / "Cryptex-M16"
        binaries = root / "bin"
        binaries.mkdir(parents=True)
        for source in SOURCES:
            destination = root / source
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(REPO / source, destination)
        if system == "Windows" and windows_artifact:
            runner = binaries / "cryptexd-test.exe"
            native = windows_artifact / "syncthing.exe"
            check_hash(native, WIN_HASH)
            shutil.copyfile(windows_artifact / "cutoff-test.exe", runner)
            launcher = root / "KONTROL.cmd"
            launcher.write_bytes(b'@echo off\r\ncd /d "%~dp0"\r\npython -B scripts\\m16\\owner_soak.py\r\npause\r\n')
        elif system == "Darwin" and not windows_artifact:
            prepare(work / "upstream", lambda cmd, _: subprocess.check_output(cmd, stderr=subprocess.STDOUT, text=True))
            native = work / "upstream/syncthing"
            check_hash(native, MAC_HASH[platform.machine()])
            runner = binaries / "cryptexd-test"
            shutil.copy2(build_runner(REPO, REPO / "target"), runner)
            launcher = root / "KONTROL.command"
            launcher.write_bytes(b'#!/bin/sh\ncd "$(dirname "$0")" || exit 1\npython3 -B scripts/m16/owner_soak.py\nprintf "\\nEnter ile kapatin."\nread -r answer\n')
            launcher.chmod(0o755)
        else:
            raise RuntimeError("unsupported package environment")
        shutil.copy2(native, binaries / native.name)
        manifest = dict(system=system, revision=revision, runner_sha256=hashlib.sha256(runner.read_bytes()).hexdigest())
        (root / "build.json").write_text(json.dumps(manifest), encoding="ascii")
        with zipfile.ZipFile(output, "x", zipfile.ZIP_DEFLATED, compresslevel=3) as archive:
            for path in sorted(root.rglob("*")):
                if path.is_file():
                    archive.write(path, path.relative_to(work).as_posix())


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    parser.add_argument("--windows-artifact", type=Path)
    args = parser.parse_args()
    build(args.output, args.windows_artifact)
