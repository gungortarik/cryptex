"""The same bounded signed-profile checks for CI and the owner Mac package."""
from collections import Counter
import json
import uuid

from mac_identity_preflight import PreflightError, run_bounded

MARKERS = (
    "native_keychain_same_profile_restart=passed",
    "native_keychain_confirmed_history_preserved=passed",
    "native_keychain_old_local_handle_refused=passed",
    "native_keychain_cross_daemon_wire_replay_refused=passed",
)


def check_session_journal(path, expected):
    transactions = json.loads(path.read_text(encoding="utf-8"))["transactions"]
    if (Counter(item["state"] for item in transactions) != Counter(expected)
            or any(len(item["append_attempts"]) != (1 if item["state"] == "anchored" else 0)
                   for item in transactions)):
        raise PreflightError("native_session_journal_outcome_mismatch")


def profile_bytes(root):
    return {path.relative_to(root): path.read_bytes()
            for folder in ("identity", "canonical")
            for path in (root / folder).rglob("*") if path.is_file()}


def check_durable_profile(binary, root, report, runner=run_bounded):
    report["credential_cleanup_verified"] = False
    try:
        result = runner([str(binary), "--cryptex-m8-local-content-durable-check", str(root)], 120)
        if result.code or result.timed_out or result.limited or result.pipe_failed:
            raise PreflightError("native_keychain_restart_failed")
        if not all(marker in result.output.splitlines() for marker in MARKERS):
            raise PreflightError("native_keychain_restart_marker_missing")
        check_session_journal(root / "bootstrap/pending.json",
                              ("owner_pending", "cancelled", "owner_pending", "anchored"))
        receipt = json.loads((root / "ui-receipt.json").read_text(encoding="utf-8"))
        if receipt.get("eventCount") != 2 or receipt.get("prefixPreserved") is not True:
            raise PreflightError("native_keychain_restart_receipt_mismatch")
        report["restart_and_confirmation_verified"] = True
        report["wire_replay_refusal_verified"] = True
    finally:
        identity = root / "identity/local_device.json"
        if identity.exists():
            device = str(uuid.UUID(json.loads(identity.read_text(encoding="utf-8"))["device_id"]))
            delete = ["/usr/bin/security", "delete-generic-password", "-s",
                      "Cryptex-Dev-DeviceCredentials", "-a", "device-event-signing-seed:" + device]
            cleanup = runner(delete, 15)
            if cleanup.code or cleanup.timed_out or cleanup.limited or cleanup.pipe_failed:
                raise PreflightError("native_restart_fixture_key_cleanup_failed")
            report["credential_cleanup_verified"] = True
        else:
            raise PreflightError("native_restart_fixture_identity_unavailable")
    before = profile_bytes(root)
    missing = runner([str(binary), "--cryptex-m8-local-content-durable-resume-check", str(root)], 30)
    missing_marker = any(line in (
        "native_keychain_seed_missing",
        "cryptexd: peer approval recovery conflict: canonical history verification failed: secure_store_missing_item",
    ) for line in missing.output.splitlines())
    absent = runner(delete, 10)
    report["credential_cleanup_verified"] = (
        absent.code == 44 and not absent.timed_out and not absent.limited and not absent.pipe_failed
    )
    if (missing.code != 2 or missing.timed_out or missing.limited or missing.pipe_failed
            or not missing_marker or before != profile_bytes(root)):
        raise PreflightError("native_keychain_missing_key_refusal_failed")
    if not report["credential_cleanup_verified"]:
        raise PreflightError("native_keychain_missing_key_recreated")
    report["missing_key_refusal_verified"] = True
