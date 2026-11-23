"""Start the ordinary release installation in a disposable native CI account."""
from contextlib import contextmanager
import json
import os
from pathlib import Path
import plistlib
import re
import struct
import subprocess
import sys
import time
from tempfile import TemporaryDirectory

from installed_update import ROOT, SUFFIX, WINDOWS, digest, run


def expected_nsis_desktop(raw):
    # Tauri patches the bundle-type marker while assembling the installer
    # and restores the original release executable after packaging.
    marker = b"__TAURI_BUNDLE_TYPE_VAR_UNK"
    if raw.count(marker) != 1:
        raise RuntimeError("Review the release binary's Tauri bundle-type marker")
    return raw.replace(marker, b"__TAURI_BUNDLE_TYPE_VAR_NSS")


def install(release, base):
    if WINDOWS:
        target = base / "Programs/Cryptex"
        if target.exists():
            raise RuntimeError("Expected an unused per-user install location")
        installer, = (release / "bundle/nsis").glob("*-setup.exe")
        run(installer, "/S", "/D=" + str(target), timeout=180)
        desktop = target / "cryptex-desktop.exe"
        with desktop.open("rb") as exe:
            if exe.read(2) != b"MZ":
                raise RuntimeError("Missing DOS header in installed Desktop")
            exe.seek(0x3c)
            header, = struct.unpack("<I", exe.read(4))
            exe.seek(header)
            if exe.read(4) != b"PE\0\0":
                raise RuntimeError("Missing PE header in installed Desktop")
            exe.seek(header + 24 + 68)
            if struct.unpack("<H", exe.read(2))[0] != 2:
                raise RuntimeError("Installed Desktop must use the Windows GUI subsystem")
        expected = expected_nsis_desktop((release / desktop.name).read_bytes())
        if desktop.read_bytes() != expected:
            raise RuntimeError("Installed Desktop differs from the expected NSIS payload")
        staged, = (ROOT / "apps/desktop/src-tauri/binaries").glob("cryptexd-*.exe")
    else:
        bundle = release / "bundle/macos/Cryptex.app"
        target = Path.home() / "Applications/Cryptex.app"
        if target.exists():
            raise RuntimeError("Expected an unused per-user install location")
        target.parent.mkdir(exist_ok=True)
        run("ditto", bundle, target)
        with (target / "Contents/Info.plist").open("rb") as stream:
            info = plistlib.load(stream)
        if (info["CFBundleIdentifier"], info["LSMinimumSystemVersion"]) != ("com.cryptex.desktop", "26.0"):
            raise RuntimeError("Unexpected bundle identity or macOS compatibility baseline")
        executable = Path("Contents/MacOS") / info["CFBundleExecutable"]
        desktop = target / executable
        if digest(desktop) != digest(bundle / executable):
            raise RuntimeError("Installed Desktop differs from the packaged executable")
        staged = (bundle / executable).with_name("cryptexd")
    daemon = desktop.with_name("cryptexd" + SUFFIX)
    if digest(daemon) != digest(staged):
        raise RuntimeError("Installed daemon differs from the packaged sidecar")
    resources = target if WINDOWS else target / "Contents/Resources"
    verify_notices(resources)
    return desktop, daemon


def verify_notices(resources):
    config_dir = ROOT / "apps/desktop/src-tauri"
    config = json.loads((config_dir / "tauri.bundle.conf.json").read_text(encoding="utf-8"))
    for source, destination in config["bundle"]["resources"].items():
        if destination.startswith("notices/") and digest(resources / destination) != digest(config_dir / source):
            raise RuntimeError(f"Installed attribution differs from packaging input: {destination}")
    native = json.loads((resources / "notices/rust.json").read_bytes())
    native_packages = {entry["name"] for entry in native["packages"]}
    if not {"tauri", "libsqlite3-sys", "ed25519-dalek"}.issubset(native_packages) or not native["documents"]:
        raise RuntimeError("Installed native dependency inventory is incomplete")
    entries = json.loads((resources / "notices/frontend.json").read_bytes())
    packages = {entry["name"] for entry in entries}
    required = {"react", "react-dom", "scheduler", "@tauri-apps/api"}
    if not required.issubset(packages) or len(packages) != len(entries):
        raise RuntimeError("Installed frontend notice inventory is incomplete or ambiguous")
    for entry in entries:
        if not all(isinstance(entry.get(key), str) and entry[key].strip()
                   for key in ("version", "identifier", "text")):
            raise RuntimeError("Missing frontend attribution or license text")
    for notice in ("focaccia/LICENSE-MIT.txt", "focaccia/LICENSE-UNICODE.txt",
                   "tauri-api/LICENSE-MIT", "tauri-api/LICENSE-APACHE-2.0"):
        if not (resources / "notices" / notice).is_file():
            raise RuntimeError(f"Missing installed notice: {notice}")


def wait_snapshot(root, process, note):
    snapshot = root / "gui-proof.snapshot"
    deadline = time.monotonic() + 45
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"Desktop exited before {note}: {process.returncode}")
        if (root / "start.error").exists():
            raise RuntimeError((root / "start.error").read_text(encoding="utf-8"))
        if snapshot.exists():
            body = snapshot.read_text(encoding="utf-8")
            fields = dict(line.split("=", 1) for line in body.splitlines() if "=" in line)
            if body.endswith("\n") and fields.get("note") == note and fields.get("desktop_pid") == str(process.pid):
                return fields
        time.sleep(0.15)
    raise TimeoutError(f"Installed Desktop did not reach {note}")


@contextmanager
def running_desktop(desktop, runtime, env=None):
    with subprocess.Popen([desktop], cwd=desktop.parent, env=env) as process:
        try:
            yield process
        finally:
            if process.poll() is None:
                try:
                    if runtime.is_dir():
                        (runtime / "quit.request").touch()
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=10)
                    raise

    if process.returncode:
        raise RuntimeError(f"Installed Desktop exited with {process.returncode}")


def expect_external_service_refusal(runtime, process):
    error = runtime / "start.error"
    deadline = time.monotonic() + 45
    while time.monotonic() < deadline and process.poll() is None:
        if error.exists():
            code = error.read_text(encoding="utf-8").strip()
            if code:
                if code != "SIDECAR_REQUEST_MISMATCH":
                    raise RuntimeError(f"Unexpected sidecar refusal: {code}")
                error.unlink()
                return code
        time.sleep(0.15)
    raise RuntimeError("Installed Desktop did not reject the external service selection")


def main():
    user = os.environ.get("USERNAME" if WINDOWS else "USER", "").lower()
    if (os.environ.get("GITHUB_ACTIONS") != "true" or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted"
            or not (WINDOWS and re.fullmatch(r"cpxci[0-9a-f]{12}", user) or sys.platform == "darwin" and user == "runner")):
        raise RuntimeError("Expected a disposable native CI account")
    base = Path(os.environ["LOCALAPPDATA"]) if WINDOWS else Path.home() / "Library/Application Support"
    runtime = base / "Cryptex-Dev/runtime/p0-2"
    if runtime.exists() or os.environ.get("CRYPTEXD_BIN"):
        raise RuntimeError("Expected a fresh profile without a daemon override")
    desktop, daemon = install(ROOT / "target/release", base)
    with TemporaryDirectory() as outside:
        foreign = Path(outside) / daemon.name
        foreign.write_bytes(b"inert sidecar selection fixture")
        with running_desktop(desktop, runtime, {**os.environ, "CRYPTEXD_BIN": str(foreign)}) as denied:
            env_refusal = expect_external_service_refusal(runtime, denied)
            status = run(daemon, "lifecycle", "status", "--runtime-root", runtime,
                         capture_output=True, text=True, timeout=10).stdout.splitlines()
            if (runtime / "ready").exists() or "pid=none" not in status or "instance=held" in status:
                raise RuntimeError("Daemon started despite the rejected environment selection")
        with running_desktop(desktop, runtime) as process:
            launch = wait_snapshot(runtime, process, "desktop_launched")
            pid = (runtime / "ready").read_text().strip()
            if not pid.isdecimal() or int(pid) < 1:
                raise RuntimeError("Missing packaged daemon readiness")
            pending = runtime / "start.pending"
            pending.write_text(str(foreign), encoding="utf-8")
            pending.replace(runtime / "start.request")
            file_refusal = expect_external_service_refusal(runtime, process)
            for request, note, visible in [(None, "desktop_launched", "true"),
                                           ("close.request", "window_closed_hidden", "false"),
                                           ("show.request", "window_restored_via_show.request", "true")]:
                if request:
                    (runtime / request).touch(exist_ok=False)
                state = wait_snapshot(runtime, process, note) if request else launch
                if (state["daemon_pid"], state["daemon_pid_observation"], state["window_visible"]) != (pid, "live", visible):
                    raise RuntimeError(f"Unexpected packaged window or daemon state after {note}")
    status = run(daemon, "lifecycle", "status", "--runtime-root", runtime,
                 capture_output=True, text=True, timeout=10).stdout.splitlines()
    if process.returncode != 0 or "pid=none" not in status or "instance=held" in status:
        raise RuntimeError("Package smoke cleanup failed")
    receipt = {"platform": sys.platform, "desktop_sha256": digest(desktop), "daemon_sha256": digest(daemon),
               "desktop_pid": process.pid, "daemon_pid": int(pid), "installed_window_hidden_and_restored": True,
               "external_environment_refusal": env_refusal, "external_request_file_refusal": file_refusal,
               "daemon_lock_released": True}
    print(json.dumps(receipt), flush=True)
    (Path(os.environ["RUNNER_TEMP"]) / "m14-release-smoke.json").write_text(json.dumps(receipt, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
