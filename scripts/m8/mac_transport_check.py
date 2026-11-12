"""Default-off signed native pairing fixture, with exact fixture-key cleanup."""
import json
import re
import stat
import uuid

from mac_identity_preflight import PreflightError

MARKERS = ("native_managed_transport_pairing=passed",
           "native_managed_pairing_restart_idempotent=passed")


def _metadata(path):
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1 or metadata.st_size > 8192:
        raise PreflightError("native_pairing_cleanup_metadata_unsafe")
    return json.loads(path.read_text(encoding="utf-8"))


def check_native_pairing(binary, root, run):
    try:
        output = run([binary, "--cryptex-m8-local-content-transport-check", root],
                     "real_native_managed_pairing", 240)
        if not all(marker in output.splitlines() for marker in MARKERS):
            raise PreflightError("native_managed_pairing_marker_missing")
    finally:
        # These are fresh CI-only roots. Never enumerate/adopt the login user's
        # other credentials, and never export keys or full transcripts.
        for role in ("initiator", "responder"):
            profile = root / role
            identity = profile / "identity/local_device.json"
            if identity.exists():
                device = str(uuid.UUID(_metadata(identity)["device_id"]))
                run(["/usr/bin/security", "delete-generic-password", "-s",
                     "Cryptex-Dev-DeviceCredentials", "-a", "device-event-signing-seed:" + device],
                    "cleanup_native_pairing_device_key")
            metadata = profile / "managed-transport/00000000000000000000000000000001/profile.json"
            if metadata.exists():
                credential = _metadata(metadata)["credential_id"]
                if not isinstance(credential, str) or not re.fullmatch(r"[0-9a-f]{32}", credential):
                    raise PreflightError("native_pairing_cleanup_credential_invalid")
                run(["/usr/bin/security", "delete-generic-password", "-s",
                     "Cryptex-Dev-ManagedSyncthing", "-a", "api:" + credential],
                    "cleanup_native_pairing_api_key")
    for marker in MARKERS:
        print(marker)
