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
from mac_transport_check import IDENTITY_MARKERS, MARKERS, MUTATION_BATCH_MARKERS, _metadata, check_identity, check_parent_loss
from mac_process_inventory import compile_probe, check_observed_pairing


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
VALIDATION_REASONS = {
    "retrieval safety gate is held:": "retrieval_safety_held",
    "CanonicalVerificationUnavailable": "canonical_verification_unavailable",
    "MaterializationStateMissingOrStale": "materialization_missing_or_stale",
    "ProjectionDoesNotMatchCanonical": "projection_canonical_mismatch",
    "UnsupportedSemantics": "unsupported_semantics",
    "V2SemanticsNotIntegrated": "v2_semantics_not_integrated",
    "UnresolvedDependency": "unresolved_dependency",
    "OperationDeadlineExceeded": "operation_deadline",
    "import staging I/O:": "staging_io",
    "import staging rejected:": "staging_invalid",
    "import staging limit:": "staging_limit",
    "unsafe or changed import filesystem object": "filesystem_unsafe_or_changed",
    "import filesystem open_fd:": "filesystem_open_fd",
    "import filesystem stat_at:": "filesystem_stat_at",
    "import filesystem lock_parent_open:": "filesystem_lock_parent_open",
    "import filesystem lock_leaf_open:": "filesystem_lock_leaf_open",
    "import filesystem existing_lock_lookup:": "filesystem_existing_lock_lookup",
    "import filesystem create_existing_ancestor:": "filesystem_create_ancestor",
    "import filesystem create_directory:": "filesystem_create_directory",
    "import filesystem validate_created_directory:": "filesystem_validate_created_directory",
    "incoming candidate aliases an entry under canonical/events": "canonical_alias",
    "candidate identity or metadata changed during snapshot": "source_metadata_changed",
    "candidate changed length during snapshot:": "source_length_changed",
    "staged bytes did not match the copied source snapshot": "snapshot_readback_mismatch",
    "staged segment ends with an incomplete": "incomplete_frame",
    "staged segment uses an unsupported frame version": "unsupported_frame_version",
    "event frame is not valid JSON": "invalid_frame_json",
    "Ed25519 signature verification failed": "signature_verification_failed",
    "content_hash mismatch:": "content_hash_mismatch",
    "JSON duplicate-key check failed:": "duplicate_json_key",
    "event device_id does not match the staged segment": "event_device_mismatch",
    "event brain_id does not match the selected Brain": "event_brain_mismatch",
    "no trusted daemon signing key is bound to the event": "untrusted_signing_key",
    "event prev_content_hash does not match the verified stream tip": "stream_hash_mismatch",
    "event device_seq is not strictly increasing": "stream_sequence_order",
    "staged segment contains a duplicate event_id": "duplicate_event_id",
    "staged segment is empty or its digest changed before validation": "empty_or_changed_segment",
    "import candidate device is outside the locally approved trust set": "device_outside_trust",
    "import cannot extend a device with an active canonical segment": "active_canonical_segment",
    "complete local-anchor trust context is unavailable": "trust_context_unavailable",
    "post-import policy union contains duplicate canonical records": "duplicate_policy_record",
    "staging cleanup failed:": "validation_cleanup_failed",
}


def summarize_validation(output):
    bounded = output[:65536]
    return {"reason_codes": sorted({code for phrase, code in VALIDATION_REASONS.items() if phrase in bounded}),
            "os_error_numbers": sorted({int(code) for code in re.findall(r"\(os error ([0-9]{1,3})\)", bounded)
                                        if int(code) <= 255})[:8]}


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


def make_runner(report, expected_pairing_root=None):
    lock = threading.Lock()
    report["diagnostic_schema"] = 2
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
        if (stage == "real_native_managed_pairing" and result.code == 2
                and not (result.timed_out or result.limited or result.pipe_failed)
                and "existing_profile_incoming_Rejected" in receipt["codes"]
                and "post_rejection_validation" not in report
                and expected_pairing_root is not None and len(args) == 3
                and args[1] == "--cryptex-m8-local-content-transport-check"
                and Path(args[2]) == expected_pairing_root):
            # run_bounded returned only after reaping the exact native driver
            # and stopping its owned process group. Keys still exist until the
            # checker's finally cleanup. No old root discovery or adoption.
            try:
                metadata = _metadata(expected_pairing_root / "initiator/identity/local_device.json")
                device = str(uuid.UUID(metadata["device_id"]))
            except (PreflightError, OSError, ValueError, TypeError, KeyError):
                report["post_rejection_validation"] = {"outcome": "fixture_metadata_refused"}
            else:
                remaining = int(timeout - (time.monotonic() - started))
                if remaining < 1:
                    report["post_rejection_validation"] = {"outcome": "original_budget_exhausted"}
                else:
                    budget = min(45, remaining)
                    probe_started = time.monotonic()
                    relative = device.encode("ascii").hex() + "/00000000000000000000.seg"
                    # Existing daemon CLI: owns the instance, stages/verifies/
                    # discards. Never publishes or repairs canonical memory.
                    # Raw wait/OS errors MUST propagate: no typed stopped proof
                    # may reach the checker cleanup while this writer is unresolved.
                    report["post_rejection_validation"] = {"outcome": "writer_exit_unresolved", "validation_only": True}
                    probe = run_bounded([str(Path(args[0]).with_name("cryptexd")), "replication",
                        "validate-incoming", "--runtime-root", str(expected_pairing_root / "responder"),
                        "--environment", "dev", relative], budget)
                    report["post_rejection_validation"] = {
                        "outcome": "validation_only_after_failure", "validation_only": True,
                        "exit_code": probe.code, "timed_out": probe.timed_out,
                        "output_limited": probe.limited, "pipe_failed": probe.pipe_failed,
                        "timeout_seconds": budget,
                        "elapsed_ms": int((time.monotonic() - probe_started) * 1000),
                        **summarize_validation(probe.output)}
                    # This later check never turns the original native failure
                    # into a pass, even when explicit validation exits zero.
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
        # Acceptance fixtures must exercise the common low descriptor budget,
        # including all native descendants; never raise the owner's limit.
        import resource
        soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
        ceiling = 256 if soft == resource.RLIM_INFINITY else min(256, soft)
        resource.setrlimit(resource.RLIMIT_NOFILE, (ceiling, hard))
        report["descriptor_soft_limit"] = ceiling
        temp = Path(tempfile.mkdtemp(prefix="cryptex-owner-transport-")).resolve()

        run = make_runner(report, temp / "pairing")

        try:
            binary = package / "positive.app/Contents/MacOS"
            check_identity(binary / "cryptexd", temp / "identity", run)
            report["identity_restart_and_refusals_verified"] = True
            probe = compile_probe(temp)
            check_observed_pairing(binary / "cryptex-desktop", temp / "pairing", probe, run, report)
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
