"""Exercise a real NSIS installation or installed .app in one disposable OS account."""
import getpass
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
import tarfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
APP = ROOT / "apps/desktop"
WINDOWS = sys.platform == "win32"
SUFFIX = ".exe" if WINDOWS else ""
PRODUCT = "Cryptex M14 Update Check"


def run(*args, **kwargs):
    return subprocess.run(args, cwd=ROOT, check=True, **kwargs)


def digest(file):
    with file.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def build_packages(work):
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve() / "debug"
    triple, = [line.split()[1] for line in run("rustc", "-vV", text=True, capture_output=True).stdout.splitlines()
               if line.startswith("host:")]
    sidecar = APP / "src-tauri/binaries" / ("cryptexd-" + triple + SUFFIX)
    sidecar.parent.mkdir(exist_ok=True)
    if sidecar.exists():
        raise RuntimeError("A staged sidecar already exists")
    pnpm = shutil.which("pnpm.cmd" if WINDOWS else "pnpm")
    protocol = ROOT / "services/cryptexd/src/compat.rs"
    original = protocol.read_bytes()
    declaration = b"pub const DAEMON_PROTOCOL_VERSION: u32 = 1;"
    if original.count(declaration) != 1:
        raise RuntimeError("Review the fixture against the current protocol")
    packages = []
    try:
        for version in ("0.14.1", "0.14.2", "0.14.3"):
            # Build two real wire versions in the private CI checkout. The third
            # installer pairs Desktop v2 with the retained protocol-v1 daemon.
            if version == "0.14.2":
                protocol.write_bytes(original.replace(declaration, declaration.replace(b"1;", b"2;")))
            if version != "0.14.3":
                run("cargo", "build", "--locked", "-p", "cryptexd", "--bin", "cryptexd",
                    "--features", "disposable-ipc")
                shutil.copy2(target / ("cryptexd" + SUFFIX), sidecar)
                if not packages:
                    shutil.copy2(sidecar, work / "baseline-daemon")
            else:
                shutil.copy2(work / "baseline-daemon", sidecar)
            config = {"productName": PRODUCT, "version": version, "identifier": "com.cryptex.m14.update-check",
                      "bundle": {"active": True, "externalBin": ["binaries/cryptexd"],
                                 "macOS": {"minimumSystemVersion": "26.0"},
                                 "windows": {"nsis": {"installMode": "currentUser"}}}}
            config_file = work / "update.conf.json"
            config_file.write_text(json.dumps(config), encoding="utf-8")
            run(pnpm, "--dir", APP, "exec", "tauri", "build", "--debug", "--features",
                "m14-installed-update-check", "--bundles", "nsis" if WINDOWS else "app",
                "--config", config_file)
            dest = work / version
            dest.mkdir()
            if WINDOWS:
                installer, = (target / "bundle/nsis").glob(f"*_{version}_*-setup.exe")
                package = dest / "setup.exe"
                shutil.copy2(installer, package)
            else:
                package = dest / (PRODUCT + ".app")
                run("ditto", target / "bundle/macos" / package.name, package)
            packages.append((version, package, digest(sidecar)))
    finally:
        protocol.write_bytes(original)
        sidecar.unlink(missing_ok=True)
    return packages


def install(package, destination):
    version, artifact, expected_daemon = package
    if WINDOWS:
        run(artifact, "/S", "/D=" + str(destination), timeout=180)
        desktop = destination / "cryptex-desktop.exe"
    else:
        installed = destination / artifact.name
        with tempfile.TemporaryDirectory(dir=destination) as staging:
            staged = Path(staging) / artifact.name
            run("ditto", artifact, staged)
            if installed.exists():
                installed.rename(Path(staging) / "previous.app")
            staged.rename(installed)
        with (installed / "Contents/Info.plist").open("rb") as stream:
            info = plistlib.load(stream)
        if info["CFBundleShortVersionString"] != version:
            raise RuntimeError("Installed .app has the wrong version")
        desktop = installed / "Contents/MacOS" / info["CFBundleExecutable"]
    daemon = desktop.with_name("cryptexd" + SUFFIX)
    if digest(daemon) != expected_daemon:
        raise RuntimeError(f"Installed daemon differs from the {version} package")
    return desktop, (digest(desktop), expected_daemon)


def wait_for_desktop_exit(pid):
    if WINDOWS:
        import _winapi
        try:
            handle = _winapi.OpenProcess(0x00100000, False, pid)  # SYNCHRONIZE
        except OSError as error:
            if error.winerror == 87:  # This PID already exited.
                return
            raise
        try:
            if _winapi.WaitForSingleObject(handle, 15000) == _winapi.WAIT_OBJECT_0:
                return
        finally:
            _winapi.CloseHandle(handle)
    else:
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                return
            time.sleep(0.1)
    raise TimeoutError("The disposable Desktop did not exit after recording its connection")


def exercise(packages, work, runtime, env):
    user_root = Path(os.environ["LOCALAPPDATA"]) / "Programs" if WINDOWS else Path.home() / "Applications"
    user_root.mkdir(parents=True, exist_ok=True)
    active = user_root / ("Cryptex" if WINDOWS else "Cryptex.app")
    pending = active.with_name(active.name + ".pending")
    if any(path.exists() for path in (active, pending, active.with_name(active.name + ".previous"))):
        raise RuntimeError("Expected a fresh installation in this disposable account")

    # Prepare complete native payloads before installing the baseline. No test
    # code installs, selects or launches the fallback during the transactions.
    images = []
    for version, artifact, daemon_hash in packages[1:]:
        directory = work / version
        directory.mkdir()
        desktop, hashes = install((version, artifact, daemon_hash), directory)
        images.append((directory if WINDOWS else directory / artifact.name, hashes))

    initial_dir = active if WINDOWS else work / "bootstrap"
    initial_dir.mkdir()
    desktop, initial_hashes = install(packages[0], initial_dir)
    if not WINDOWS:
        (initial_dir / packages[0][1].name).rename(active)
    desktop = active / ("cryptex-desktop.exe" if WINDOWS else "Contents/MacOS/" + desktop.name)
    daemon = desktop.with_name("cryptexd" + SUFFIX)
    tool = work / ("cryptex-updater" + SUFFIX)
    shutil.copy2(packages[0][1].parent.parent / tool.name, tool)
    receipts = []
    previous = active.with_name(active.name + ".previous")
    for index, source in enumerate((images[0][0], images[1][0], images[1][0])):
        if WINDOWS:
            shutil.copytree(source, pending)
        else:
            run("ditto", source, pending)
        receipt = runtime / "m14-update-result.json"
        receipt.unlink(missing_ok=True)
        approved = run(tool, "fingerprint", source, text=True, capture_output=True).stdout.strip()
        if index == 2:
            # Simulate losing the coordinator immediately after package selection.
            # The restarted updater must recover the last confirmed version.
            state = (Path(os.environ["LOCALAPPDATA"]) if WINDOWS else Path.home() / "Library/Application Support") / "Cryptex-Updater"
            working = run(tool, "fingerprint", active, text=True, capture_output=True).stdout.strip()
            with (state / "package-transition.json").open("x", encoding="utf-8") as stream:
                json.dump({"working": working, "staged": approved}, stream)
            active.rename(previous)
            pending.rename(active)
        else:
            wrong = subprocess.run([tool, "apply", "0" * 64], env=env, capture_output=True, timeout=30)
            if wrong.returncode == 0 or b"approved digest" not in wrong.stderr:
                raise RuntimeError("Unexpected outcome for an unapproved staged image")
        process = run(tool, "apply", approved, env=env, text=True, stdout=subprocess.PIPE, timeout=150)
        report = json.loads(process.stdout)
        expected = ([(1, 1), (2, 2)], [(2, 2), (2, 1), (2, 2)], [(2, 2), (2, 2)])[index]
        checks = report["checks"]
        if [(c["desktop_protocol"], c["daemon_protocol"]) for c in checks] != expected:
            raise RuntimeError(f"Unexpected IPC handshake sequence: {checks}")
        if [c["status"] for c in checks] != ["incompatible" if a != b else "connected" for a, b in expected]:
            raise RuntimeError("Unexpected update decision evidence")
        if report["outcome"] != ("updated", "rolled_back", "recovered")[index]:
            raise RuntimeError(f"Unexpected updater outcome: {report}")
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            if receipt.is_file() and receipt.stat().st_size:
                break
            time.sleep(0.1)
        reopened = json.loads(receipt.read_bytes())
        if (reopened["desktop_pid"], reopened["version"], reopened["status"], reopened["daemon_protocol"]) != (
                report["reopened_pid"], "0.14.2", "connected", 2):
            raise RuntimeError(f"Reopened Desktop did not reconnect: {reopened}")
        if (digest(desktop), digest(daemon)) != images[0][1] or any(a == b for a, b in zip(initial_hashes, images[0][1])):
            raise RuntimeError("Active Desktop or daemon differs from the working upgrade")
        # The GUI writes its receipt just before exiting in the disposable build.
        # Wait for the native session lock via the process handle below.
        wait_for_desktop_exit(reopened["desktop_pid"])
        receipts.append({"updater": report, "reopened": reopened})
        # Keep the displaced package as evidence and free the staging name.
        pending.rename(user_root / ("Cryptex-displaced-" + str(index)))
    return {"platform": sys.platform, "scope": "installed-product-updater", "transactions": receipts,
            "restored_pair_byte_identical": True}


def main():
    if (os.environ.get("GITHUB_ACTIONS") != "true" or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted"
            or sys.platform not in ("win32", "darwin")):
        raise RuntimeError("Use an ephemeral GitHub-hosted native runner")
    action, packages_dir = sys.argv[1:]
    packages_dir = Path(packages_dir).resolve()
    if action == "build" and getpass.getuser().lower() in ("runneradmin", "runner"):
        packages_dir.mkdir()
        run("cargo", "build", "--locked", "-p", "cryptex-updater")
        target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve()
        shutil.copy2(target / "debug" / ("cryptex-updater" + SUFFIX), packages_dir)
        with tarfile.open(packages_dir / "cryptex-local-updater.tar.gz", "w:gz") as archive:
            archive.add(packages_dir / ("cryptex-updater" + SUFFIX), arcname="cryptex-updater" + SUFFIX)
        packages = build_packages(packages_dir)
        for _, artifact, daemon_hash in packages:
            (artifact.parent / "daemon.sha256").write_text(daemon_hash, encoding="ascii")
        return
    user = os.environ.get("USERNAME" if WINDOWS else "USER", "").lower()
    if action != "exercise" or not (re.fullmatch(r"cpxci[0-9a-f]{12}", user) if WINDOWS else user == "runner"):
        raise RuntimeError("Use the disposable exercise account")
    packages = []
    for version in ("0.14.1", "0.14.2", "0.14.3"):
        folder = packages_dir / version
        packages.append((version, folder / ("setup.exe" if WINDOWS else PRODUCT + ".app"),
                         (folder / "daemon.sha256").read_text(encoding="ascii")))
    base = Path(os.environ["LOCALAPPDATA"]) if WINDOWS else Path.home() / "Library/Application Support"
    runtime = base / "Cryptex-Dev/p0-2/compare/mode-b-desktop"
    runtime.mkdir(parents=True, exist_ok=False)
    token = str(uuid.uuid4())
    (runtime / "m14-update-owner").write_text(token, encoding="utf-8")
    with tempfile.TemporaryDirectory(dir=os.environ["RUNNER_TEMP"], prefix="cryptex-m14-update-") as directory:
        work = Path(directory)
        report = exercise(packages, work, runtime, dict(os.environ, CRYPTEX_FILE_FIXTURE_SOCKET=token))
        print(json.dumps(report), flush=True)
        (Path(os.environ["RUNNER_TEMP"]) / "m14-installed-update.json").write_text(json.dumps(report, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
