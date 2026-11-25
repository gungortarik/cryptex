"""Read a previous daemon's signed note with the installed candidate as a standard user."""
import os
from pathlib import Path
import subprocess
import sys
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "m14"))
from installed_update import verify_note


def main(previous, installed):
    if sys.platform != "win32" or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted":
        raise RuntimeError("Disposable Windows runner required")
    # The standard-user runner removes this private profile with its account.
    root = Path(os.environ["LOCALAPPDATA"]) / str(uuid.uuid4())
    root.mkdir()
    (root / "pilot-runtime.marker").write_bytes(b"m0-native-desktop-runtime-v1\n")
    subprocess.run([previous, "pilot-fixture", "signed-transfer-v1", "--runtime-root", root,
                    "--confirm-isolated-pilot"], check=True, timeout=45)
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
