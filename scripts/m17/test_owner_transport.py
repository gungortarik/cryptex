"""Run the owner-selected transport transition with fresh, synthetic peers."""
import hashlib
import io
import os
import platform
import shutil
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


def run_trial(native_test=None, previous_test=None):
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
                if native_test is not None:
                    package = Path(tmp) / version
                    package.mkdir(mode=0o700)
                    for leaf in (name, "LICENSE.txt", "AUTHORS.txt"):
                        data = archive.read(f"{stem}/{leaf}")
                        if version == "2.1.5" and sys.platform == "win32" and leaf != name:
                            # The Windows 1.0.1 enrollment installed the repository's LF notices.
                            notices = Path(__file__).resolve().parents[2] / "third_party/syncthing/2.1.5"
                            data = (notices / leaf).read_bytes()
                        (package / leaf).write_bytes(data)
                    (package / name).chmod(0o500)
            binary.chmod(0o500)
            binaries.append(binary)
        runner = Path(__file__).resolve().parents[2] / "tools/m0-lifecycle-compare/scripts/run-syncthing-upgrade-proof.mjs"
        args = ["node", runner, "--m17-owner-transport", "--root", root,
                "--baseline", binaries[0], "--candidate", binaries[1]]
        if native_test is not None:
            stage_test = "managed_profile::native::stopped_enrollment::stage_enrolled_previous_release"
            trial = Path(tmp).resolve()
            source = trial / "source"
            source.mkdir(mode=0o700)
            managed = source / "managed-transport"
            managed.mkdir(mode=0o700)
            shutil.copytree(trial / "2.1.5", managed / "package")
            (trial / "receiver").mkdir(mode=0o700)
            result = subprocess.run([native_test, stage_test, "--exact", "--ignored"],
                                    env=dict(os.environ, CRYPTEX_PAIRED_HANDOFF=str(trial),
                                             CRYPTEX_PREVIOUS_TEST=str(previous_test)),
                                    text=True, stdout=subprocess.PIPE, timeout=240)
            if result.returncode or f"test {stage_test} ... ok" not in result.stdout:
                raise RuntimeError(f"Stopped service enrollment check failed: {result.stdout[-2048:]}")
            print("stopped_previous_pair_staged_with_saved_endpoints_and_original_credential=passed", flush=True)
            print("enrolled_transport_eight_interrupted_selections_restored=passed", flush=True)
            test = "managed_profile::exchange_install::tests::paired_release_handoff::recognize_retained_package"
            result = subprocess.run([native_test, test, "--exact", "--ignored"],
                                    env=dict(os.environ, CRYPTEX_M17_RELEASE_PAIR=str(Path(tmp).resolve())),
                                    check=True, text=True, stdout=subprocess.PIPE, timeout=60)
            if f"test {test} ... ok" not in result.stdout:
                raise RuntimeError("Native retained-package check did not execute")
            print("previous_and_candidate_package_pins_and_damaged_notice_checked=passed", flush=True)
            args.extend(["--native-stage-test", native_test])
        subprocess.run(args, check=True, timeout=320)


if __name__ == "__main__":
    run_trial()
