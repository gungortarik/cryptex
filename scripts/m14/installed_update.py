"""Exercise a real NSIS installation or installed .app in one disposable OS account."""
import getpass
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
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
                package = dest / installer.name
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


def trial(package, destination, runtime, env):
    desktop, fingerprints = install(package, destination)
    result = runtime / "m14-update-result.json"
    result.unlink(missing_ok=True)
    with subprocess.Popen([desktop], env=env) as child:
        try:
            code = child.wait(timeout=75)
        except subprocess.TimeoutExpired:
            child.kill()
            raise
    if code:
        raise RuntimeError(f"Installed Desktop {package[0]} exited with {code}")
    reply = json.loads(result.read_bytes())
    if reply["version"] != package[0] or reply["desktop_pid"] != child.pid or reply["daemon_pid"] <= 0:
        raise RuntimeError(f"Unexpected installed identity: {reply}")
    return reply, fingerprints


def exercise(packages, work, runtime, env):
    destination = work / "Applications"
    destination.mkdir()
    receipts = []

    def launch(package):
        reply, images = trial(package, destination, runtime, env)
        receipts.append(reply)
        return reply, images

    def update(known_good, replacement):
        candidate_reply, images = launch(replacement)
        if candidate_reply["status"] == "connected":
            return replacement, images
        if candidate_reply["status"] != "incompatible":
            raise RuntimeError("Missing update health decision")
        restored_reply, restored_images = launch(known_good)
        if restored_reply["status"] != "connected":
            raise RuntimeError("Reinstalled fallback did not reconnect")
        return known_good, restored_images

    baseline, candidate, incompatible = packages
    first, old_images = launch(baseline)
    if (first["status"], first["desktop_protocol"], first["daemon_protocol"]) != ("connected", 1, 1):
        raise RuntimeError("Baseline IPC connection failed")
    active, good_images = update(baseline, candidate)
    second = receipts[-1]
    if active != candidate or (second["status"], second["desktop_protocol"], second["daemon_protocol"]) != ("connected", 2, 2):
        raise RuntimeError("Updated pair failed to reconnect")
    if any(old == new for old, new in zip(old_images, good_images)):
        raise RuntimeError("Compatible update did not replace both installed components")
    active, restored_images = update(active, incompatible)
    if active != candidate or len(receipts) != 4:
        raise RuntimeError("Incompatible update did not restore the last working version")
    failed, restored = receipts[-2:]
    if (failed["status"], failed["desktop_protocol"], failed["daemon_protocol"]) != ("incompatible", 2, 1):
        raise RuntimeError("Installed pair did not return a correlated protocol rejection")
    if (restored["status"], restored["desktop_protocol"], restored["daemon_protocol"]) != ("connected", 2, 2) or restored_images != good_images:
        raise RuntimeError("Rollback did not restore and reconnect the last working pair")
    return {"platform": sys.platform, "scope": "installed-rehearsal", "launches": receipts,
            "restored_pair_byte_identical": True}


def main():
    if (os.environ.get("GITHUB_ACTIONS") != "true" or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted"
            or sys.platform not in ("win32", "darwin") or getpass.getuser().lower() not in ("runneradmin", "runner")):
        raise RuntimeError("Use an ephemeral GitHub-hosted native runner")
    base = Path(os.environ["LOCALAPPDATA"]) if WINDOWS else Path.home() / "Library/Application Support"
    runtime = base / "Cryptex-Dev/p0-2/compare/mode-b-desktop"
    runtime.mkdir(parents=True, exist_ok=False)
    token = str(uuid.uuid4())
    (runtime / "m14-update-owner").write_text(token, encoding="utf-8")
    with tempfile.TemporaryDirectory(dir=os.environ["RUNNER_TEMP"], prefix="cryptex-m14-update-") as directory:
        work = Path(directory)
        report = exercise(build_packages(work), work, runtime, dict(os.environ, CRYPTEX_FILE_FIXTURE_SOCKET=token))
        print(json.dumps(report), flush=True)
        (Path(os.environ["RUNNER_TEMP"]) / "m14-installed-update.json").write_text(json.dumps(report, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
