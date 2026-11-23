"""Exercise a real NSIS installation or installed .app in one disposable OS account."""
import getpass
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import tarfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts/m17"))
import update_fixture

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
            if WINDOWS:
                update_fixture.image(sidecar, "com.cryptex.daemon")
                config["bundle"]["windows"].update(certificateThumbprint=os.environ["CX_UPDATE_CI_SIGNER"], digestAlgorithm="sha256")
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
            if WINDOWS:
                with tempfile.TemporaryDirectory() as tmp:
                    installed = Path(tmp) / "Cryptex"
                    run(package, "/S", "/D=" + str(installed), timeout=180)
                    daemon_hash = digest(installed / "cryptexd.exe")
                    update_fixture.receipt(work / "cryptex-updater.exe", installed, dest / "release.cms")
                    if not packages:
                        update_fixture.receipt(work / "cryptex-updater.exe", installed, dest / "wrong.cms", wrong=True)
                    # Remove the bootstrap installation registration before packaging the next version.
                    run(installed / "uninstall.exe", "/S", "_?=" + str(installed), timeout=180)
            else:
                daemon = package / "Contents/MacOS/cryptexd"
                update_fixture.image(daemon, "com.cryptex.daemon")
                update_fixture.image(package, "com.cryptex.desktop")
                daemon_hash = digest(daemon)
                update_fixture.receipt(work / "cryptex-updater", package, dest / "release.cms")
                if not packages:
                    update_fixture.receipt(work / "cryptex-updater", package, dest / "wrong.cms", wrong=True)
            packages.append((version, package, daemon_hash))
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


def interrupt_after_selection(tool, approved, working, active, runtime, journal, env):
    armed = runtime / "m14-interrupt-probe"
    armed.touch(exist_ok=False)
    probe_pid = None
    with subprocess.Popen([tool, "apply", approved], env=env, stdin=subprocess.DEVNULL) as updater:
        try:
            deadline = time.monotonic() + 45
            while time.monotonic() < deadline:
                markers = list(runtime.glob("m14-held-probe-*"))
                if markers:
                    marker, = markers
                    probe_pid = int(marker.name.removeprefix("m14-held-probe-"))
                    break
                if updater.poll() is not None:
                    raise RuntimeError(f"Updater exited before interruption: {updater.returncode}")
                time.sleep(0.025)
            if probe_pid is None:
                raise TimeoutError("Candidate did not reach the interrupted probe")
            intent = journal.read_bytes()
            if json.loads(intent) != {"working": working, "staged": approved}:
                raise RuntimeError("Updater did not journal the two approved images")
            updater.kill()
            exit_code = updater.wait(timeout=10)
            if exit_code != (1 if WINDOWS else -signal.SIGKILL):
                raise RuntimeError(f"Unexpected updater termination: {exit_code}")
            busy = subprocess.run([tool, "apply", approved], env=env, capture_output=True, timeout=5)
            if busy.returncode == 0 or b"quit Cryptex before updating" not in busy.stderr:
                raise RuntimeError("Recovery did not respect the surviving probe's session lock")
            if journal.read_bytes() != intent or active.with_name(active.name + ".pending").exists():
                raise RuntimeError("Busy recovery changed the interrupted selection")
        finally:
            armed.unlink()
            if updater.poll() is None:
                updater.kill()
                updater.wait(timeout=10)
            if probe_pid is not None:
                wait_for_desktop_exit(probe_pid)
    previous = active.with_name(active.name + ".previous")
    for path, expected in ((active, approved), (previous, working)):
        if run(tool, "fingerprint", path, text=True, capture_output=True).stdout.strip() != expected:
            raise RuntimeError("Interrupted package fingerprint differs from journal")
    return {"updater_pid": updater.pid, "exit_code": exit_code, "probe_pid": probe_pid,
            "busy_recovery_refused": True, "journal_and_images_verified": True}


def apply_with_live_timeout(tool, approved, runtime, env):
    armed = runtime / "m14-timeout-probe"
    armed.touch(exist_ok=False)
    try:
        with subprocess.Popen([sys.executable, "-c", "import time; time.sleep(180)"]) as unrelated:
            try:
                started = time.monotonic()
                result = run(tool, "apply", approved, env=env, text=True, stdout=subprocess.PIPE, timeout=110)
                elapsed = time.monotonic() - started
                marker, = runtime.glob("m14-held-live-*")
                probe_pid, daemon_pid = map(int, marker.name.removeprefix("m14-held-live-").split("-"))
                if min(probe_pid, daemon_pid) <= 0 or probe_pid == daemon_pid or elapsed < 30:
                    raise RuntimeError("The candidate did not exercise the native probe timeout")
                wait_for_desktop_exit(probe_pid)
                wait_for_desktop_exit(daemon_pid)
                if unrelated.poll() is not None:
                    raise RuntimeError("Probe cleanup stopped an unrelated process")
                return result, {"probe_pid": probe_pid, "daemon_pid": daemon_pid,
                                "probe_and_daemon_exited": True, "unrelated_process_survived": True,
                                "apply_elapsed_seconds": round(elapsed, 2)}
            finally:
                if unrelated.poll() is None:
                    unrelated.terminate()
                unrelated.wait(timeout=10)
    finally:
        armed.unlink()


def verify_note(daemon, runtime):
    events = runtime / "canonical/events"
    packs = sorted(path for path in events.rglob("*") if path.suffix in (".seg", ".open"))
    segment, = [path for path in packs if path.suffix == ".seg"]
    descriptor = runtime / "signed-probe.json"
    verified = run(daemon, "pilot-fixture", "verify-signed-transfer-v1", "--segment", segment,
                   "--descriptor", descriptor, "--confirm-isolated-pilot", capture_output=True, timeout=30)
    report = json.loads(verified.stdout)
    if report["outcome"] != "signature_verified" or report["event_count"] != 1:
        raise RuntimeError("Expected one verified synthetic note")
    return {path.relative_to(runtime).as_posix(): digest(path) for path in [*packs, descriptor]}


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
    (runtime / "pilot-runtime.marker").write_bytes(b"m0-native-desktop-runtime-v1\n")
    create = [daemon, "pilot-fixture", "signed-transfer-v1", "--runtime-root", runtime, "--confirm-isolated-pilot"]
    if WINDOWS:
        run(*create, timeout=45)
    else:
        sys.path.insert(0, str(ROOT / "scripts/m8"))
        from mac_ci_credential_keychain import CredentialKeychain
        from mac_identity_preflight import run_bounded

        with CredentialKeychain(work, run_bounded) as scope:
            scope.fence.checked(create, "create_m14_signed_note", 45)
    original_note = verify_note(daemon, runtime)
    (runtime / "m14-update-owner").write_text(env["CRYPTEX_FILE_FIXTURE_SOCKET"], encoding="utf-8")
    tool = work / ("cryptex-updater" + SUFFIX)
    shutil.copy2(packages[0][1].parent.parent / tool.name, tool)
    receipts = []
    previous = active.with_name(active.name + ".previous")
    state = (Path(os.environ["LOCALAPPDATA"]) if WINDOWS else Path.home() / "Library/Application Support") / "Cryptex-Updater"
    journal = state / "package-transition.json"
    def refuse(command, reason):
        result = subprocess.run(command, env=env, capture_output=True, timeout=40)
        if result.returncode == 0 or reason not in result.stderr:
            raise RuntimeError(f"Unexpected signing decision: {result.returncode} {result.stderr!r}")

    baseline_dir = packages[0][1].parent
    refuse([tool, "admit", active, baseline_dir / "wrong.cms"], b"receipt verification failed")
    refuse([tool, "admit", active, packages[1][1].parent / "release.cms"], b"receipt verification failed")
    for path, (_, artifact, _) in zip([active, *(image[0] for image in images)], packages, strict=True):
        for _ in range(2):
            run(tool, "admit", path, artifact.parent / "release.cms", env=env, timeout=45)
    for index, source in enumerate([images[0][0]] + [images[1][0]] * 4):
        if WINDOWS:
            shutil.copytree(source, pending)
        else:
            run("ditto", source, pending)
        receipt = runtime / "m14-update-result.json"
        receipt.unlink(missing_ok=True)
        approved = run(tool, "fingerprint", source, text=True, capture_output=True).stdout.strip()
        if index == 0:
            candidate_receipt = state / "release-receipts" / approved
            saved_receipt = candidate_receipt.read_bytes()
            candidate_receipt.write_bytes((baseline_dir / "wrong.cms").read_bytes())
            try:
                refuse([tool, "apply", approved], b"receipt verification failed")
                if journal.exists() or previous.exists() or receipt.exists() or not pending.is_dir():
                    raise RuntimeError("Unauthenticated update changed the current installation")
            finally:
                candidate_receipt.write_bytes(saved_receipt)
        if index == 2:
            working = run(tool, "fingerprint", active, text=True, capture_output=True).stdout.strip()
            interruption = interrupt_after_selection(tool, approved, working, active, runtime, journal, env)
            retained_receipt = state / "release-receipts" / working
            saved_receipt = retained_receipt.read_bytes()
            retained_receipt.write_bytes(baseline_dir.joinpath("wrong.cms").read_bytes())
            try:
                refuse([tool, "apply", approved], b"receipt verification failed")
                if not journal.is_file() or not previous.is_dir() or receipt.exists():
                    raise RuntimeError("Unauthenticated recovery changed the interrupted selection")
            finally:
                retained_receipt.write_bytes(saved_receipt)
            if verify_note(daemon, runtime) != original_note:
                raise RuntimeError("Interrupted selection changed canonical note data")
        else:
            wrong = subprocess.run([tool, "apply", "0" * 64], env=env, capture_output=True, timeout=30)
            if wrong.returncode == 0 or b"approved digest" not in wrong.stderr:
                raise RuntimeError("Unexpected outcome for an unapproved staged image")
        if index == 3:
            process, timeout_evidence = apply_with_live_timeout(tool, approved, runtime, env)
            if run(tool, "fingerprint", pending, text=True, capture_output=True).stdout.strip() != approved:
                raise RuntimeError("Timeout rollback changed the staged candidate")
        elif index == 4:
            marker = runtime / "m17-fail-probe"
            marker.touch(exist_ok=False)
            try:
                process = run(tool, "apply", approved, env=env, text=True, stdout=subprocess.PIPE, timeout=150)
            finally:
                marker.unlink()
        else:
            process = run(tool, "apply", approved, env=env, text=True, stdout=subprocess.PIPE, timeout=150)
        report = json.loads(process.stdout)
        expected = ([(1, 1), (2, 2)], [(2, 2), (2, 1), (2, 2)], [(2, 2), (2, 2)], [(2, 2)] * 3, [(2, 2)] * 3)[index]
        checks = report["checks"]
        if [(c["desktop_protocol"], c["daemon_protocol"]) for c in checks] != expected:
            raise RuntimeError(f"Transaction {index} returned {report['outcome']} with unexpected IPC handshake sequence: {checks}")
        if [c["status"] for c in checks] != ["incompatible" if a != b else "connected" for a, b in expected]:
            raise RuntimeError("Unexpected update decision evidence")
        if report["outcome"] != ("updated", "rolled_back", "recovered", "rolled_back_timeout", "rolled_back_failed_probe")[index]:
            raise RuntimeError(f"Unexpected updater outcome: {report}")
        if journal.exists() or previous.exists():
            raise RuntimeError("Completed operation retained an unfinished transition")
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
        if verify_note(daemon, runtime) != original_note:
            raise RuntimeError("Installed package transaction changed canonical note data")
        transaction = {"updater": report, "reopened": reopened, "canonical_note_verified": True}
        if index == 2:
            transaction["interruption"] = interruption
        if index == 3:
            transaction["timeout"] = timeout_evidence
        receipts.append(transaction)
        # Keep the displaced package as evidence and free the staging name.
        pending.rename(user_root / ("Cryptex-displaced-" + str(index)))

    # An ordinary app removal and reinstall must leave the separate Brain readable.
    if WINDOWS:
        run(active / "uninstall.exe", "/S", timeout=120)
        deadline = time.monotonic() + 60
        while (desktop.exists() or daemon.exists()) and time.monotonic() < deadline:
            time.sleep(0.2)
        if desktop.exists() or daemon.exists():
            raise RuntimeError("NSIS left the application binaries installed")
        install(packages[1], active)
    else:
        # The normal packaged Mac command removes only the reviewed application
        # and its matching per-user next-login registration. Preserve a neighbor.
        agents = Path.home() / "Library/LaunchAgents"
        agents.mkdir(parents=True, exist_ok=True)
        login = agents / "Cryptex.plist"
        neighbor = agents / f"cryptex-ci-{uuid.uuid4()}.plist"
        if os.path.lexists(login):
            raise RuntimeError("Expected no existing Cryptex registration in the disposable account")
        foreign = plistlib.dumps({"Label": "unrelated", "ProgramArguments": ["/usr/bin/true"]})
        neighbor.write_bytes(foreign)
        login.write_bytes(foreign)
        fingerprint = run(tool, "fingerprint", active, text=True, capture_output=True).stdout.strip()
        removal = [tool, "remove-installed-app", fingerprint]
        refuse([tool, "remove-installed-app", "0" * 64], b"differs from its approved digest")
        refuse(removal, b"login entry does not belong to this installation")
        if not active.is_dir() or login.read_bytes() != foreign or neighbor.read_bytes() != foreign:
            raise RuntimeError("Refused Mac removal changed an application or login entry")
        login.write_bytes(plistlib.dumps({"Label": "Cryptex", "ProgramArguments": [str(desktop), "--cryptex-background"],
                                         "RunAtLoad": True}))
        import fcntl
        with (state / "desktop.lock").open("rb") as session:
            fcntl.flock(session, fcntl.LOCK_SH)
            refuse(removal, b"quit Cryptex before updating")
        with journal.open("xb"):
            try:
                refuse(removal, b"finish the pending update recovery")
            finally:
                journal.unlink()
        if not active.is_dir() or not login.is_file() or neighbor.read_bytes() != foreign:
            raise RuntimeError("Busy Mac removal changed the installation or startup entry")
        for has_login in (True, False):
            run(*removal, env=env, timeout=90)
            if os.path.lexists(active) or os.path.lexists(login) or neighbor.read_bytes() != foreign:
                raise RuntimeError(f"Mac removal left the wrong installation state (registered={has_login})")
            run("ditto", packages[1][1], active)
            if verify_note(daemon, runtime) != original_note:
                raise RuntimeError("Mac reinstall lost the verified signed note")
        print("mac_removal=verified with_and_without_login=true unrelated_entry_preserved=true "
              "signed_note_verified_after_reinstall=true", flush=True)
    run(tool, "admit", active, packages[1][1].parent / "release.cms", env=env, timeout=45)
    if verify_note(daemon, runtime) != original_note:
        raise RuntimeError("Reinstall changed the signed note or its verification evidence")
    return {"wrong_signer_refused": True, "foreign_receipt_refused": True, "unauthenticated_update_refused": True,
            "unauthenticated_rollback_refused": True,
            "platform": sys.platform, "scope": "installed-product-updater", "transactions": receipts,
            "restored_pair_byte_identical": True, "signed_note_verified_after_reinstall": True}


def main():
    if (os.environ.get("GITHUB_ACTIONS") != "true" or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted"
            or sys.platform not in ("win32", "darwin")):
        raise RuntimeError("Use an ephemeral GitHub-hosted native runner")
    action, packages_dir = sys.argv[1:]
    packages_dir = Path(packages_dir).resolve()
    if action == "build" and getpass.getuser().lower() in ("runneradmin", "runner"):
        packages_dir.mkdir()
        with update_fixture.signer():
            run("cargo", "build", "--release", "--locked", "-p", "cryptex-updater")
            target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve()
            shutil.copy2(target / "release" / ("cryptex-updater" + SUFFIX), packages_dir)
            update_fixture.image(packages_dir / ("cryptex-updater" + SUFFIX), "com.cryptex.updater")
            notices = packages_dir / "updater-notices.json"
            run("node", ROOT / "scripts/m17/updater_notices.mjs", notices)
            with tarfile.open(packages_dir / "cryptex-local-updater.tar.gz", "w:gz") as archive:
                archive.add(notices, arcname="notices/rust.json")
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
    with tempfile.TemporaryDirectory(dir=os.environ["RUNNER_TEMP"], prefix="cryptex-m14-update-") as directory:
        work = Path(directory)
        report = exercise(packages, work, runtime, dict(os.environ, CRYPTEX_FILE_FIXTURE_SOCKET=token))
        print(json.dumps(report), flush=True)
        (Path(os.environ["RUNNER_TEMP"]) / "m14-installed-update.json").write_text(json.dumps(report, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
