"""Read a previous daemon's signed note with the installed candidate as a standard user."""
import os
from pathlib import Path
import subprocess
import sys
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "m14"))
from installed_update import ROOT, digest, verify_note


def historical_profile(previous, root=None):
    # The standard-user runner removes this private profile with its account.
    if root is None:
        root = Path(os.environ["LOCALAPPDATA"]) / str(uuid.uuid4())
    root.mkdir(parents=True, exist_ok=False)
    (root / "pilot-runtime.marker").write_bytes(b"m0-native-desktop-runtime-v1\n")
    subprocess.run([previous, "pilot-fixture", "signed-transfer-v1", "--runtime-root", root,
                    "--confirm-isolated-pilot"], check=True, timeout=45)
    return root


def existing_enrollment(previous, installed):
    # Use the actual installed Desktop runtime in the disposable standard account.
    import msvcrt
    base = Path(os.environ["LOCALAPPDATA"])
    root = historical_profile(previous, base / "Cryptex-Dev/runtime/p0-2")
    before = verify_note(previous, root)
    subprocess.run([sys.executable, "-B", ROOT / "scripts/m8/prepare_native_test_package.py"],
                   cwd=ROOT, check=True, timeout=100)
    package = Path(os.environ["RUNNER_TEMP"]) / "exchange-package"
    enroll = ["transport", "enroll-local-dev", "--runtime-root", root, "--confirm-local-only"]
    subprocess.run([previous, *enroll, "--package", package], check=True, timeout=160)
    managed = root / "managed-transport"

    def inventory():
        return {path.relative_to(managed): digest(path)
                for path in managed.rglob("*") if path.is_file()}

    saved = inventory()
    profile = Path("0" * 31 + "1")
    required = [profile / name for name in
                ("profile.json", "native-generation-complete", "native/config.xml",
                 "native/cert.pem", "native/key.pem")]
    if (any(path not in saved for path in required)
            or saved.get(Path("package/syncthing.exe")) != digest(package / "syncthing.exe")):
        raise RuntimeError("Incomplete historical native enrollment")
    refused = subprocess.run([installed, *enroll], capture_output=True, text=True, timeout=45)
    if (refused.returncode != 2 or
            "cryptexd: attention: transport_enrollment_AlreadyExists" not in refused.stderr.splitlines()):
        raise RuntimeError("Expected the installed candidate to refuse fresh enrollment")
    if inventory() != saved or verify_note(previous, root) != before or verify_note(installed, root) != before:
        raise RuntimeError("Candidate re-enrollment changed the old transport or signed history")
    updater_state = base / "Cryptex-Updater"
    updater_state.mkdir(exist_ok=True)
    with (updater_state / "update.lock").open("a+b") as lease:
        lease.seek(0)
        msvcrt.locking(lease.fileno(), msvcrt.LK_NBLCK, 1)
        runtime_files = {path.relative_to(root): digest(path) for path in root.rglob("*") if path.is_file()}
        probe = subprocess.run([installed.with_name("cryptex-desktop.exe"), "--cryptex-update-probe"],
                               capture_output=True, text=True, timeout=45)
        if (probe.returncode != 2 or "enrolled transport requires cold migration" not in probe.stderr
                or probe.stdout.strip()):
            raise RuntimeError("Installed Desktop accepted an unmigrated transport or failed at the wrong boundary")
        if {path.relative_to(root): digest(path) for path in root.rglob("*") if path.is_file()} != runtime_files:
            raise RuntimeError("Candidate health probe changed the previous runtime")
    if verify_note(previous, root) != before:
        raise RuntimeError("Historical service cannot verify its original note after candidate refusal")
    print("installed_desktop_refused_unmigrated_215_before_daemon_start=passed")
    # The next release smoke creates a separate fresh profile at the fixed pathname.
    root.rename(root.with_name("preserved-previous-" + str(uuid.uuid4())))
    print("historically_enrolled_215_preserved_on_candidate_reenrollment_refusal=passed")


def main(previous, installed):
    if sys.platform != "win32" or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted":
        raise RuntimeError("Disposable Windows runner required")
    existing_enrollment(previous, installed)
    root = historical_profile(previous)
    before = verify_note(previous, root)
    identity = root / "identity"
    def public_files():
        return {file.relative_to(identity): file.read_bytes()
                for file in identity.rglob("*.json")}
    keys = public_files()
    if not any(file.name == "event_signing_key.json" for file in keys):
        raise RuntimeError("Missing original public signing identity")
    # Enrollment verifies that the existing signing identity can still be
    # resolved through this user's native credential store.
    subprocess.run([installed, "transport", "enroll-local-dev", "--runtime-root", root,
                    "--confirm-local-only"], check=True, timeout=160)
    if verify_note(installed, root) != before or public_files() != keys:
        raise RuntimeError("Historical signed note or identity changed")
    print("installed_102_verified_note_created_by_101_daemon=passed")


if __name__ == "__main__":
    main(*map(Path, sys.argv[1:]))
