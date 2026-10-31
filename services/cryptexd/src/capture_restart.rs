//! M5 Slice 2: cross-process capture restart / lost-response proofs.
//!
//! Child processes use fresh process-local state and a file-backed test
//! SecureStore.
//!
//! Two families are covered:
//! - **Error/restart:** child returns a typed capture outcome; parent reopens.
//! - **Abrupt interruption:** child writes a handshake then exits at a durability
//!   boundary before normal capture cleanup returns. Not power-loss proof.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use cryptex_core::BrainIdentity;
use cryptex_core::ids::DeviceId;
use cryptex_events::Classification;

use crate::canonical_writer::{CanonicalWriterConfig, load_verified_events};
use crate::capture::{
    CaptureOutcome, CaptureRejectCode, CaptureRequest, CaptureUncertainReason, capture_memory_note,
    resolve_capture,
};
use crate::capture_receipt::{load_receipt, receipt_path};
use crate::identity_runtime::write_brain_identity;
use crate::local_device_identity::{enroll_local_device, load_local_device_identity};
use crate::secure_store::{FileBackedSecureStore, SecureStoreEnvironment};
use crate::test_interrupt::{
    INTERRUPT_ENV, INTERRUPT_EXIT_CODE, READY_NAME, read_ready, wait_child_bounded, wait_for_path,
};
use crate::test_temp::exclusive_temp_dir;

const BRAIN: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const WORKER_ENV: &str = "CRYPTEX_TEST_CAPTURE_XPROC_WORKER";
const ROOT_ENV: &str = "CRYPTEX_TEST_CAPTURE_XPROC_ROOT";
const CAPTURE_ID_ENV: &str = "CRYPTEX_TEST_CAPTURE_XPROC_CAPTURE_ID";
const TEXT_ENV: &str = "CRYPTEX_TEST_CAPTURE_XPROC_TEXT";
const RESULT_ENV: &str = "CRYPTEX_TEST_CAPTURE_XPROC_RESULT";
const FAULT_ENV: &str = "CRYPTEX_TEST_CAPTURE_FAULT";
const TEST_NAME: &str = "capture_restart::xproc_worker_entry";

fn store_open(runtime_root: &Path) -> FileBackedSecureStore {
    FileBackedSecureStore::open(runtime_root.join("secure_store")).expect("open file-backed store")
}

fn ensure_fixture(runtime_root: &Path) -> DeviceId {
    let store = store_open(runtime_root);
    let brain = BrainIdentity::new(BRAIN, "2026-10-10T00:00:00Z").unwrap();
    let _ = write_brain_identity(runtime_root, &brain);
    let identity =
        enroll_local_device(runtime_root, &store, SecureStoreEnvironment::Dev).expect("enroll");
    DeviceId::new(identity.device_id).unwrap()
}

fn device_id(runtime_root: &Path) -> DeviceId {
    let local = load_local_device_identity(runtime_root, BRAIN).expect("local identity");
    DeviceId::new(local.device_id).unwrap()
}

fn outcome_line(outcome: &CaptureOutcome) -> String {
    match outcome {
        CaptureOutcome::Committed(c) => format!("COMMITTED|{}|{}", c.event_id, c.device_seq),
        CaptureOutcome::Rejected { code, .. } => {
            let code = match code {
                CaptureRejectCode::InvalidRequest => "invalid_request",
                CaptureRejectCode::EnrollmentRequired => "enrollment_required",
                CaptureRejectCode::IdentityUnavailable => "identity_unavailable",
                CaptureRejectCode::CaptureIdConflict => "capture_id_conflict",
                CaptureRejectCode::NotCommitted => "not_committed",
            };
            format!("REJECTED|{code}")
        }
        CaptureOutcome::Uncertain { reason, .. } => {
            let reason = match reason {
                CaptureUncertainReason::WriterFailure => "writer_failure",
                CaptureUncertainReason::DurabilityUnconfirmed => "durability_unconfirmed",
                CaptureUncertainReason::ReceiptMissing => "receipt_missing",
                CaptureUncertainReason::ReceiptPersistFailed => "receipt_persist_failed",
                CaptureUncertainReason::VerificationUnavailable => "verification_unavailable",
                CaptureUncertainReason::FingerprintMismatch => "fingerprint_mismatch",
                CaptureUncertainReason::LockFailed => "lock_failed",
            };
            format!("UNCERTAIN|{reason}")
        }
    }
}

fn run_worker_body() {
    let role = std::env::var(WORKER_ENV).expect("worker role");
    let root = PathBuf::from(std::env::var(ROOT_ENV).expect("root"));
    let store = store_open(&root);
    let capture_id = std::env::var(CAPTURE_ID_ENV).unwrap_or_default();
    let text = std::env::var(TEXT_ENV).unwrap_or_else(|_| "xproc note".into());
    let result_path = PathBuf::from(std::env::var(RESULT_ENV).expect("result path"));
    let cfg = CanonicalWriterConfig::default();

    match role.as_str() {
        "enroll" => {
            let _ = ensure_fixture(&root);
            fs::write(&result_path, "ENROLLED").expect("write enrolled");
            std::process::exit(0);
        }
        "capture" => {
            let outcome = capture_memory_note(
                &store,
                SecureStoreEnvironment::Dev,
                &root,
                CaptureRequest {
                    capture_id,
                    classification: Classification::Internal,
                    text,
                },
                &cfg,
            );
            fs::write(&result_path, outcome_line(&outcome)).expect("write capture result");
            std::process::exit(0);
        }
        "resolve" => {
            let outcome = resolve_capture(&store, SecureStoreEnvironment::Dev, &root, &capture_id);
            fs::write(&result_path, outcome_line(&outcome)).expect("write resolve result");
            std::process::exit(0);
        }
        other => {
            fs::write(&result_path, format!("BAD_ROLE|{other}")).ok();
            std::process::exit(2);
        }
    }
}

/// Shared entrypoint: when worker env is set, any test that calls this will act as worker.
/// Each parent test calls `dispatch_worker_if_requested()` first.
fn dispatch_worker_if_requested() {
    if std::env::var_os(WORKER_ENV).is_some() {
        run_worker_body();
    }
}

/// Exact-name entry used by Command args so cargo routes the child into this crate test.
#[test]
fn xproc_worker_entry() {
    dispatch_worker_if_requested();
}

fn spawn_worker(
    root: &Path,
    role: &str,
    capture_id: &str,
    text: &str,
    result_path: &Path,
    fault: Option<&str>,
    interrupt: Option<&str>,
) -> std::process::Child {
    let exe = std::env::current_exe().expect("current_exe");
    let mut cmd = Command::new(&exe);
    cmd.env(WORKER_ENV, role)
        .env(ROOT_ENV, root)
        .env(CAPTURE_ID_ENV, capture_id)
        .env(TEXT_ENV, text)
        .env(RESULT_ENV, result_path)
        .env("RUST_TEST_THREADS", "1")
        .args(["--exact", TEST_NAME, "--nocapture"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(fault) = fault {
        cmd.env(FAULT_ENV, fault);
    } else {
        cmd.env_remove(FAULT_ENV);
    }
    if let Some(boundary) = interrupt {
        cmd.env(INTERRUPT_ENV, boundary);
    } else {
        cmd.env_remove(INTERRUPT_ENV);
    }
    cmd.spawn().expect("spawn capture xproc worker")
}

fn run_worker(
    root: &Path,
    role: &str,
    capture_id: &str,
    text: &str,
    fault: Option<&str>,
) -> String {
    let result_path = root.join(format!("result-{}-{}.txt", role, std::process::id()));
    let _ = fs::remove_file(&result_path);
    let mut child = spawn_worker(root, role, capture_id, text, &result_path, fault, None);
    let status = child.wait().expect("wait worker");
    assert!(status.success(), "worker {role} failed: {status:?}");
    fs::read_to_string(&result_path).unwrap_or_else(|e| panic!("missing result for {role}: {e}"))
}

/// Run a child that must abruptly exit at `interrupt` after writing the ready handshake.
fn run_interrupt_worker(
    root: &Path,
    capture_id: &str,
    text: &str,
    interrupt: &str,
) -> (String, String) {
    let ready = root.join(READY_NAME);
    let _ = fs::remove_file(&ready);
    let result_path = root.join(format!("interrupt-result-{}.txt", std::process::id()));
    let _ = fs::remove_file(&result_path);
    let mut child = spawn_worker(
        root,
        "capture",
        capture_id,
        text,
        &result_path,
        None,
        Some(interrupt),
    );
    assert!(
        wait_for_path(&ready, Duration::from_secs(30)),
        "interrupt handshake not observed for {interrupt}"
    );
    let status =
        wait_child_bounded(&mut child, Duration::from_secs(15)).expect("wait interrupt child");
    assert_eq!(
        status.code(),
        Some(INTERRUPT_EXIT_CODE),
        "expected abrupt exit {INTERRUPT_EXIT_CODE}, got {status:?}"
    );
    // Child must not have completed normal response delivery.
    assert!(
        !result_path.exists() || fs::metadata(&result_path).map(|m| m.len()).unwrap_or(0) == 0,
        "abrupt interrupt must not publish a normal capture result file"
    );
    read_ready(root)
}

fn event_count(root: &Path) -> usize {
    let store = store_open(root);
    let device = device_id(root);
    load_verified_events(&store, SecureStoreEnvironment::Dev, root, &device)
        .expect("load events")
        .len()
}

fn pending_event_id(root: &Path, capture_id: &str) -> String {
    let device = device_id(root);
    let receipt = load_receipt(root, BRAIN, &device, capture_id)
        .expect("load receipt")
        .expect("receipt present");
    assert_eq!(receipt.state, crate::capture_receipt::ReceiptState::Pending);
    receipt.event_id
}

#[test]
fn xproc_pending_before_append_restart_resolves_absent_then_resumes_once() {
    dispatch_worker_if_requested();
    let root = exclusive_temp_dir("cryptex-m5-s2-pending");
    assert_eq!(run_worker(&root, "enroll", "", "", None), "ENROLLED");
    let capture_id = "11111111-1111-4111-8111-111111111111";
    let text = "pending before append";
    let first = run_worker(
        &root,
        "capture",
        capture_id,
        text,
        Some("after_pending_before_append"),
    );
    assert!(
        first.starts_with("UNCERTAIN|"),
        "expected uncertain after PENDING: {first}"
    );
    let event_id = pending_event_id(&root, capture_id);
    assert_eq!(event_count(&root), 0);

    let resolved = run_worker(&root, "resolve", capture_id, text, None);
    assert_eq!(resolved, "REJECTED|not_committed");

    let second = run_worker(&root, "capture", capture_id, text, None);
    assert!(
        second.starts_with(&format!("COMMITTED|{event_id}|")),
        "resume must keep event_id {event_id}, got {second}"
    );
    assert_eq!(event_count(&root), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn xproc_append_before_tip_restart_reconciles_without_second_event() {
    dispatch_worker_if_requested();
    let root = exclusive_temp_dir("cryptex-m5-s2-tip");
    assert_eq!(run_worker(&root, "enroll", "", "", None), "ENROLLED");
    let capture_id = "22222222-2222-4222-8222-222222222222";
    let text = "append before tip";
    let first = run_worker(
        &root,
        "capture",
        capture_id,
        text,
        Some("after_append_before_tip"),
    );
    assert!(first.starts_with("UNCERTAIN|"), "got {first}");
    assert_eq!(event_count(&root), 1, "append durable before tip failure");
    let event_id = pending_event_id(&root, capture_id);

    let reconciled = run_worker(&root, "capture", capture_id, text, None);
    assert!(
        reconciled.starts_with(&format!("COMMITTED|{event_id}|")),
        "got {reconciled}"
    );
    assert_eq!(event_count(&root), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn xproc_append_before_committed_receipt_restart_reconciles_once() {
    dispatch_worker_if_requested();
    let root = exclusive_temp_dir("cryptex-m5-s2-receipt");
    assert_eq!(run_worker(&root, "enroll", "", "", None), "ENROLLED");
    let capture_id = "33333333-3333-4333-8333-333333333333";
    let text = "append before committed receipt";
    let first = run_worker(
        &root,
        "capture",
        capture_id,
        text,
        Some("after_append_before_committed_receipt"),
    );
    assert!(first.starts_with("UNCERTAIN|"), "got {first}");
    assert_eq!(event_count(&root), 1);
    let event_id = pending_event_id(&root, capture_id);

    let reconciled = run_worker(&root, "resolve", capture_id, text, None);
    assert!(
        reconciled.starts_with(&format!("COMMITTED|{event_id}|")),
        "got {reconciled}"
    );
    assert_eq!(event_count(&root), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn xproc_committed_lost_response_replay_returns_original_event() {
    dispatch_worker_if_requested();
    let root = exclusive_temp_dir("cryptex-m5-s2-lost");
    assert_eq!(run_worker(&root, "enroll", "", "", None), "ENROLLED");
    let capture_id = "44444444-4444-4444-8444-444444444444";
    let text = "lost response after committed";
    let first = run_worker(&root, "capture", capture_id, text, None);
    assert!(first.starts_with("COMMITTED|"), "got {first}");
    let event_id = first.split('|').nth(1).unwrap().to_string();
    // Simulate lost response: discard the result file and use a fresh process.
    let replay = run_worker(&root, "capture", capture_id, text, None);
    assert_eq!(replay, first, "lost-response replay must match original");
    assert!(replay.contains(&event_id));
    assert_eq!(event_count(&root), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn xproc_partial_staged_receipt_is_not_false_committed() {
    dispatch_worker_if_requested();
    let root = exclusive_temp_dir("cryptex-m5-s2-stage");
    assert_eq!(run_worker(&root, "enroll", "", "", None), "ENROLLED");
    let capture_id = "55555555-5555-4555-8555-555555555555";
    let text = "partial staged receipt";
    let first = run_worker(
        &root,
        "capture",
        capture_id,
        text,
        Some("after_receipt_stage_before_place"),
    );
    assert!(first.starts_with("UNCERTAIN|"), "got {first}");
    let device = device_id(&root);
    let path = receipt_path(&root, BRAIN, &device, capture_id);
    assert!(
        !path.exists(),
        "final receipt must not exist after stage fail"
    );
    assert_eq!(event_count(&root), 0);

    let resolved = run_worker(&root, "resolve", capture_id, text, None);
    assert_eq!(resolved, "UNCERTAIN|receipt_missing");

    let second = run_worker(&root, "capture", capture_id, text, None);
    assert!(second.starts_with("COMMITTED|"), "got {second}");
    assert_eq!(event_count(&root), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn xproc_corrupt_receipt_fail_closed_after_restart() {
    dispatch_worker_if_requested();
    let root = exclusive_temp_dir("cryptex-m5-s2-corrupt");
    assert_eq!(run_worker(&root, "enroll", "", "", None), "ENROLLED");
    let capture_id = "66666666-6666-4666-8666-666666666666";
    let text = "corrupt receipt";
    let first = run_worker(
        &root,
        "capture",
        capture_id,
        text,
        Some("after_pending_before_append"),
    );
    assert!(first.starts_with("UNCERTAIN|"), "got {first}");
    let device = device_id(&root);
    let path = receipt_path(&root, BRAIN, &device, capture_id);
    fs::write(&path, "{not-json\n").expect("corrupt receipt");

    let resolved = run_worker(&root, "resolve", capture_id, text, None);
    assert!(
        resolved.starts_with("UNCERTAIN|"),
        "corrupt receipt must not commit: {resolved}"
    );
    let capture = run_worker(&root, "capture", capture_id, text, None);
    assert!(
        capture.starts_with("UNCERTAIN|"),
        "corrupt receipt must fail closed on capture: {capture}"
    );
    assert_eq!(event_count(&root), 0);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn xproc_concurrent_matching_capture_ids_single_event() {
    dispatch_worker_if_requested();
    let root = exclusive_temp_dir("cryptex-m5-s2-concurrent");
    assert_eq!(run_worker(&root, "enroll", "", "", None), "ENROLLED");
    let capture_id = "77777777-7777-4777-8777-777777777777";
    let text = "concurrent matching";
    let r1 = root.join("concurrent-a.txt");
    let r2 = root.join("concurrent-b.txt");
    let mut a = spawn_worker(&root, "capture", capture_id, text, &r1, None, None);
    let mut b = spawn_worker(&root, "capture", capture_id, text, &r2, None, None);
    let sa = a.wait().expect("wait a");
    let sb = b.wait().expect("wait b");
    assert!(sa.success() && sb.success(), "workers failed {sa:?} {sb:?}");
    let oa = fs::read_to_string(&r1).expect("result a");
    let ob = fs::read_to_string(&r2).expect("result b");
    assert!(
        oa.starts_with("COMMITTED|") || oa.starts_with("UNCERTAIN|"),
        "{oa}"
    );
    assert!(
        ob.starts_with("COMMITTED|") || ob.starts_with("UNCERTAIN|"),
        "{ob}"
    );
    // If either side was uncertain due to lock races, reconcile once.
    if !oa.starts_with("COMMITTED|") || !ob.starts_with("COMMITTED|") {
        let reconciled = run_worker(&root, "capture", capture_id, text, None);
        assert!(reconciled.starts_with("COMMITTED|"), "{reconciled}");
    }
    assert_eq!(event_count(&root), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn xproc_distinct_capture_ids_identical_text_remain_two_notes() {
    dispatch_worker_if_requested();
    let root = exclusive_temp_dir("cryptex-m5-s2-distinct");
    assert_eq!(run_worker(&root, "enroll", "", "", None), "ENROLLED");
    let text = "identical text distinct ids";
    let a = run_worker(
        &root,
        "capture",
        "88888888-8888-4888-8888-888888888888",
        text,
        None,
    );
    let b = run_worker(
        &root,
        "capture",
        "99999999-9999-4999-8999-999999999999",
        text,
        None,
    );
    assert!(
        a.starts_with("COMMITTED|") && b.starts_with("COMMITTED|"),
        "{a} / {b}"
    );
    let ea = a.split('|').nth(1).unwrap();
    let eb = b.split('|').nth(1).unwrap();
    assert_ne!(ea, eb);
    assert_eq!(event_count(&root), 2);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn xproc_durable_capture_recoverable_without_sqlite() {
    dispatch_worker_if_requested();
    let root = exclusive_temp_dir("cryptex-m5-s2-nosqlite");
    assert_eq!(run_worker(&root, "enroll", "", "", None), "ENROLLED");
    let capture_id = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
    let text = "no sqlite required";
    let first = run_worker(&root, "capture", capture_id, text, None);
    assert!(first.starts_with("COMMITTED|"), "{first}");
    // Ensure no projection DB is required for recovery via verified packs.
    let sqlite_candidates = [
        root.join("derived").join("projection.sqlite"),
        root.join("projection.sqlite"),
        root.join("derived").join("notes.sqlite"),
    ];
    for path in sqlite_candidates {
        assert!(!path.exists(), "slice 2 must not create projection db");
    }
    let replay = run_worker(&root, "resolve", capture_id, text, None);
    assert_eq!(replay, first);
    assert_eq!(event_count(&root), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn xproc_interrupt_pending_before_append_then_fresh_resume() {
    dispatch_worker_if_requested();
    let root = exclusive_temp_dir("cryptex-m5-s2-int-pending");
    assert_eq!(run_worker(&root, "enroll", "", "", None), "ENROLLED");
    let capture_id = "a0a0a0a0-a0a0-4a0a-8a0a-a0a0a0a0a0a0";
    let text = "interrupt after durable pending";
    let (boundary, detail) =
        run_interrupt_worker(&root, capture_id, text, "after_pending_before_append");
    assert_eq!(boundary, "after_pending_before_append");
    let event_id = pending_event_id(&root, capture_id);
    assert_eq!(detail, event_id);
    assert_eq!(event_count(&root), 0);

    let resolved = run_worker(&root, "resolve", capture_id, text, None);
    assert_eq!(resolved, "REJECTED|not_committed");
    let resumed = run_worker(&root, "capture", capture_id, text, None);
    assert!(
        resumed.starts_with(&format!("COMMITTED|{event_id}|")),
        "fresh resume must keep {event_id}, got {resumed}"
    );
    assert_eq!(event_count(&root), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn xproc_interrupt_append_before_tip_then_fresh_reconcile() {
    dispatch_worker_if_requested();
    let root = exclusive_temp_dir("cryptex-m5-s2-int-tip");
    assert_eq!(run_worker(&root, "enroll", "", "", None), "ENROLLED");
    let capture_id = "b1b1b1b1-b1b1-4b1b-8b1b-b1b1b1b1b1b1";
    let text = "interrupt after durable append before tip";
    let (boundary, detail) =
        run_interrupt_worker(&root, capture_id, text, "after_append_before_tip");
    assert_eq!(boundary, "after_append_before_tip");
    assert_eq!(event_count(&root), 1);
    let event_id = pending_event_id(&root, capture_id);
    assert_eq!(detail, event_id);

    let reconciled = run_worker(&root, "capture", capture_id, text, None);
    assert!(
        reconciled.starts_with(&format!("COMMITTED|{event_id}|")),
        "got {reconciled}"
    );
    assert_eq!(event_count(&root), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn xproc_interrupt_staged_receipt_before_place_not_committed() {
    dispatch_worker_if_requested();
    let root = exclusive_temp_dir("cryptex-m5-s2-int-stage");
    assert_eq!(run_worker(&root, "enroll", "", "", None), "ENROLLED");
    let capture_id = "c2c2c2c2-c2c2-4c2c-8c2c-c2c2c2c2c2c2";
    let text = "interrupt after staged receipt before place";
    let (boundary, detail) =
        run_interrupt_worker(&root, capture_id, text, "after_receipt_stage_before_place");
    assert_eq!(boundary, "after_receipt_stage_before_place");
    assert_eq!(detail, capture_id);
    let device = device_id(&root);
    let path = receipt_path(&root, BRAIN, &device, capture_id);
    assert!(
        !path.exists(),
        "final receipt must be absent after stage interrupt"
    );
    assert_eq!(event_count(&root), 0);

    let resolved = run_worker(&root, "resolve", capture_id, text, None);
    assert_eq!(resolved, "UNCERTAIN|receipt_missing");
    let committed = run_worker(&root, "capture", capture_id, text, None);
    assert!(committed.starts_with("COMMITTED|"), "{committed}");
    assert_eq!(event_count(&root), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn xproc_interrupt_committed_before_response_lost_delivery_replay() {
    dispatch_worker_if_requested();
    let root = exclusive_temp_dir("cryptex-m5-s2-int-committed");
    assert_eq!(run_worker(&root, "enroll", "", "", None), "ENROLLED");
    let capture_id = "d3d3d3d3-d3d3-4d3d-8d3d-d3d3d3d3d3d3";
    let text = "interrupt after durable committed before response";
    let (boundary, detail) =
        run_interrupt_worker(&root, capture_id, text, "after_committed_before_response");
    assert_eq!(boundary, "after_committed_before_response");
    assert_eq!(event_count(&root), 1);
    let device = device_id(&root);
    let receipt = load_receipt(&root, BRAIN, &device, capture_id)
        .expect("load")
        .expect("committed receipt present");
    assert_eq!(
        receipt.state,
        crate::capture_receipt::ReceiptState::Committed
    );
    assert_eq!(detail, receipt.event_id);

    let replay = run_worker(&root, "capture", capture_id, text, None);
    assert!(
        replay.starts_with(&format!("COMMITTED|{}|", receipt.event_id)),
        "lost-delivery replay must return original event, got {replay}"
    );
    assert_eq!(event_count(&root), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn xproc_interrupt_append_before_committed_receipt_then_fresh_reconcile() {
    dispatch_worker_if_requested();
    let root = exclusive_temp_dir("cryptex-m5-s2-int-committed-receipt");
    assert_eq!(run_worker(&root, "enroll", "", "", None), "ENROLLED");
    let capture_id = "e4e4e4e4-e4e4-4e4e-8e4e-e4e4e4e4e4e4";
    let text = "interrupt after durable append before committed receipt";
    let (boundary, detail) = run_interrupt_worker(
        &root,
        capture_id,
        text,
        "after_append_before_committed_receipt",
    );
    assert_eq!(boundary, "after_append_before_committed_receipt");
    assert_eq!(event_count(&root), 1);
    let event_id = pending_event_id(&root, capture_id);
    assert_eq!(detail, event_id);

    let reconciled = run_worker(&root, "resolve", capture_id, text, None);
    assert!(
        reconciled.starts_with(&format!("COMMITTED|{event_id}|")),
        "got {reconciled}"
    );
    assert_eq!(event_count(&root), 1);
    let _ = fs::remove_dir_all(&root);
}
