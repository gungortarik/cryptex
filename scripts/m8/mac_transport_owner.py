#!/usr/bin/python3
"""One-click local native pairing/transport fixture; aggregate-only receipt."""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import uuid
import zipfile

from mac_identity_preflight import PreflightError, run_bounded
from mac_transport_check import IDENTITY_MARKERS, MARKERS, MUTATION_BATCH_MARKERS, check_identity, check_native_pairing, check_parent_loss


# Closed diagnostic inventory. Never export raw output, arguments, paths,
# identities, Keychain values or exception text from native processes.
SAFE_CODES = frozenset("""
candidate_transport_unavailable candidate_transport_shape
candidate_pairing_brain_mismatch candidate_offer_refused candidate_offer_missing
candidate_proof_refused candidate_proof_missing candidate_initiator_refused
candidate_responder_refused candidate_approval_missing
candidate_transport_identity_collision candidate_pairing_not_recorded
candidate_pairing_history candidate_pairing_reopen_changed_history
candidate_worker candidate_worker_cleanup candidate_worker_cleanup_failed
candidate_snapshot candidate_receipt_write candidate_path
existing_profile_source existing_profile_incoming existing_profile_false_protection
existing_profile_publication existing_profile_publication_mismatch
existing_profile_live_restriction existing_profile_restriction_not_reconciled
existing_profile_old_handle_accepted existing_profile_restart_changed_events
existing_profile_live_pairing existing_profile_missing
incoming_hint_invalid incoming_queue_busy incoming_runtime_mismatch
incoming_transport_binding_unavailable incoming_publication_failed
""".split()) | frozenset("local_signed_startup_" + name for name in (
    "Arguments", "Inventory", "Image", "Channel", "Rendezvous", "Instance",
    "Deadline", "Spawn", "Readiness", "Cleanup", "StaleAccepted",
)) | frozenset("existing_profile_incoming_" + name for name in (
    "Idle", "WaitingForTransport", "Deferred", "Published", "Rejected", "Held", "Unavailable",
))
SAFE_MARKERS = frozenset(IDENTITY_MARKERS + MARKERS + tuple(MUTATION_BATCH_MARKERS.values()))
SAFE_STARTUP_STAGES = frozenset((
    "desktop_inventory", "daemon_image", "verification_hint", "native_accept",
    "startup_hello", "startup_ack", "runtime_ready", "ready_file",
    "child_inventory", "child_verification_hint",
))


def summarize_output(output):
    """Diagnostic hints only; acceptance still requires the original checks."""
    bounded = output[:65536]
    tokens = set(re.findall(r"[A-Za-z_][A-Za-z_0-9]*", bounded))
    stages = []
    for line in bounded.splitlines():
        if not line.startswith("{") or len(line) > 2048:
            continue
        try:
            value = json.loads(line)
        except (ValueError, RecursionError):
            continue
        if not isinstance(value, dict):
            continue
        name, error, elapsed = value.get("m8LocalStage"), value.get("error"), value.get("elapsedMs")
        if (isinstance(name, str) and name in SAFE_STARTUP_STAGES
                and isinstance(error, str) and error in SAFE_CODES
                and type(elapsed) is int and 0 <= elapsed <= 300000 and len(stages) < 16):
            stages.append({"stage": name, "error": error, "elapsed_ms": elapsed})
    return {"codes": sorted(tokens & SAFE_CODES),
            "markers": sorted(set(bounded.splitlines()) & SAFE_MARKERS),
            "startup_failures": stages}


def make_runner(report):
    lock = threading.Lock()
    report["diagnostic_schema"] = 1
    report["command_diagnostics"] = []

    def run(args, stage, timeout=30):
        started = time.monotonic()
        result = run_bounded([str(arg) for arg in args], timeout)
        receipt = {"stage": stage, "exit_code": result.code,
                   "timed_out": result.timed_out, "output_limited": result.limited,
                   "pipe_failed": result.pipe_failed,
                   "elapsed_ms": int((time.monotonic() - started) * 1000),
                   "timeout_seconds": timeout, **summarize_output(result.output)}
        with lock:
            if len(report["command_diagnostics"]) < 64:
                report["command_diagnostics"].append(receipt)
            else:
                report["command_diagnostics_truncated"] = True
        if result.code or result.timed_out or result.limited or result.pipe_failed:
            raise PreflightError(stage + "_failed")
        return result.output

    return run


def main():
    os.umask(0o077)
    package = Path(__file__).resolve().parent
    meta = json.loads((package / "fixture.json").read_text(encoding="utf-8"))
    report = {"source": meta["source"], "outcome": "incomplete",
              "collector_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "scope": "isolated real local engines and signed native pairing; not cross-device sync"}
    print("Mac aktarım kimliği ve eşleştirme testi başladı. Pencereyi açık bırak.\n"
          "Test kendi geçici profillerini kullanır; kişisel verileri aktarmaz.", flush=True)
    if sys.platform != "darwin" or platform.machine() != meta["architecture"]:
        report["outcome"] = "platform_mismatch"
    elif datetime.now(timezone.utc) >= datetime.fromisoformat(meta["expires_utc"]):
        report["outcome"] = "fixture_expired"
    else:
        temp = Path(tempfile.mkdtemp(prefix="cryptex-owner-transport-")).resolve()

        run = make_runner(report)

        try:
            binary = package / "positive.app/Contents/MacOS"
            check_identity(binary / "cryptexd", temp / "identity", run)
            report["identity_restart_and_refusals_verified"] = True
            check_native_pairing(binary / "cryptex-desktop", temp / "pairing", run)
            report["native_pairing_and_reopen_verified"] = True
            check_parent_loss(binary / "cryptexd", temp / "loss", run)
            report["daemon_loss_and_unresolved_refusal_verified"] = True
            report["credential_cleanup_verified"] = True
            shutil.rmtree(temp)
            report["runtime_cleanup_verified"] = True
            report["outcome"] = "passed"
        except PreflightError as error:
            report["outcome"] = "failed"
            report["diagnostic"] = str(error)
        except (OSError, ValueError, TypeError, KeyError, subprocess.SubprocessError):
            report["outcome"] = "collector_incomplete"
        finally:
            if report.get("runtime_cleanup_verified") is not True:
                print(f"Test kaydı inceleme için korundu: {temp}", flush=True)
    output = package / ("MAC-AKTARIM-SONUC-" + uuid.uuid4().hex[:8] + ".zip")
    with zipfile.ZipFile(output, "x") as archive:
        archive.writestr("result.json", json.dumps(report, indent=2) + "\n")
    print(f"Sonuç: {report['outcome']}\nPaylaşılacak dosya: {output}", flush=True)
    return 0 if report["outcome"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
