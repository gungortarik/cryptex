//! Test-only binary update / rollback harness for M0 P0-2.
//!
//! Stages synthetic daemon binaries under a Cryptex-Dev runtime root, activates
//! replacements, detects failed starts, and rolls back to last-known-good.
//!
//! This does **not** select ADR-0012 updater orchestration and is not a
//! production installer.

use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::compat::{ComponentVersions, require_compatible};
use crate::error::{LifecycleError, LifecycleResult};
use crate::paths::{assert_outside_repository, marker_path};
use crate::supervisor::DesktopSupervisor;

const UPDATE_BIN: &str = "update-bin";
const ACTIVE: &str = "active";
const PREVIOUS: &str = "previous";
const STAGING: &str = "staging";
const FAILED: &str = "failed";
const IDENTITY_FILE: &str = "identity";
const BIN_NAME: &str = "cryptexd-fixture";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateLayout {
    pub root: PathBuf,
}

impl UpdateLayout {
    pub fn new(runtime_root: &Path) -> LifecycleResult<Self> {
        assert_outside_repository(runtime_root)?;
        let root = runtime_root.join(UPDATE_BIN);
        fs::create_dir_all(root.join(ACTIVE))?;
        fs::create_dir_all(root.join(PREVIOUS))?;
        fs::create_dir_all(root.join(STAGING))?;
        fs::create_dir_all(root.join(FAILED))?;
        Ok(Self { root })
    }

    pub fn active_bin(&self) -> PathBuf {
        self.root.join(ACTIVE).join(BIN_NAME)
    }

    pub fn previous_bin(&self) -> PathBuf {
        self.root.join(PREVIOUS).join(BIN_NAME)
    }

    pub fn staging_bin(&self) -> PathBuf {
        self.root.join(STAGING).join(BIN_NAME)
    }

    pub fn failed_bin(&self) -> PathBuf {
        self.root.join(FAILED).join(BIN_NAME)
    }

    fn identity_path(dir: &Path) -> PathBuf {
        dir.join(IDENTITY_FILE)
    }

    pub fn active_identity(&self) -> LifecycleResult<String> {
        read_identity(&Self::identity_path(&self.root.join(ACTIVE)))
    }

    pub fn previous_identity(&self) -> LifecycleResult<Option<String>> {
        let path = Self::identity_path(&self.root.join(PREVIOUS));
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(read_identity(&path)?))
    }
}

fn read_identity(path: &Path) -> LifecycleResult<String> {
    let raw = fs::read_to_string(path).map_err(|error| {
        LifecycleError::Attention(format!(
            "unable to read update identity {}: {error}",
            path.display()
        ))
    })?;
    let identity = raw.trim().to_string();
    if identity.is_empty() {
        return Err(LifecycleError::Attention(
            "update identity file is empty".into(),
        ));
    }
    Ok(identity)
}

fn write_identity(dir: &Path, identity: &str) -> LifecycleResult<()> {
    validate_identity(identity)?;
    fs::create_dir_all(dir)?;
    fs::write(UpdateLayout::identity_path(dir), format!("{identity}\n"))?;
    Ok(())
}

fn validate_identity(identity: &str) -> LifecycleResult<()> {
    if identity.is_empty()
        || !identity
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(LifecycleError::Usage(format!(
            "invalid fixture identity '{identity}'"
        )));
    }
    Ok(())
}

fn clear_dir(dir: &Path) -> LifecycleResult<()> {
    if dir.exists() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                fs::remove_dir_all(&path)?;
            } else {
                fs::remove_file(&path)?;
            }
        }
    } else {
        fs::create_dir_all(dir)?;
    }
    Ok(())
}

fn copy_file(src: &Path, dst: &Path) -> LifecycleResult<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(src, dst).map_err(|error| {
        LifecycleError::Io(format!(
            "copy {} -> {}: {error}",
            src.display(),
            dst.display()
        ))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(dst)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(dst, perms)?;
    }
    Ok(())
}

/// Install the initial known-good fixture into `active/`.
pub fn install_known_good(
    runtime_root: &Path,
    source_bin: &Path,
    identity: &str,
) -> LifecycleResult<UpdateLayout> {
    let layout = UpdateLayout::new(runtime_root)?;
    if !source_bin.is_file() {
        return Err(LifecycleError::Io(format!(
            "known-good binary missing: {}",
            source_bin.display()
        )));
    }
    clear_dir(&layout.root.join(ACTIVE))?;
    clear_dir(&layout.root.join(PREVIOUS))?;
    clear_dir(&layout.root.join(STAGING))?;
    clear_dir(&layout.root.join(FAILED))?;
    copy_file(source_bin, &layout.active_bin())?;
    write_identity(&layout.root.join(ACTIVE), identity)?;
    Ok(layout)
}

/// Stage a candidate replacement without activating it.
pub fn stage_candidate(
    runtime_root: &Path,
    source_bin: &Path,
    identity: &str,
) -> LifecycleResult<UpdateLayout> {
    let layout = UpdateLayout::new(runtime_root)?;
    if !source_bin.is_file() {
        return Err(LifecycleError::Io(format!(
            "candidate binary missing: {}",
            source_bin.display()
        )));
    }
    clear_dir(&layout.root.join(STAGING))?;
    copy_file(source_bin, &layout.staging_bin())?;
    write_identity(&layout.root.join(STAGING), identity)?;
    Ok(layout)
}

/// Validate the synthetic Desktop/daemon protocol pair before staging the
/// daemon fixture. This is evidence scaffolding, not package installation.
pub fn stage_compatible_candidate(
    runtime_root: &Path,
    source_bin: &Path,
    identity: &str,
    versions: ComponentVersions,
) -> LifecycleResult<UpdateLayout> {
    require_compatible(versions)?;
    stage_candidate(runtime_root, source_bin, identity)
}

/// Write an intentionally non-starting fixture into staging (failed-update case).
pub fn stage_failing_candidate(
    runtime_root: &Path,
    identity: &str,
) -> LifecycleResult<UpdateLayout> {
    let layout = UpdateLayout::new(runtime_root)?;
    clear_dir(&layout.root.join(STAGING))?;
    let staging = layout.staging_bin();
    #[cfg(unix)]
    {
        fs::write(
            &staging,
            "#!/bin/sh\necho \"cryptexd-fixture: incompatible/failing update fixture\" >&2\nexit 3\n",
        )?;
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&staging)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&staging, perms)?;
    }
    #[cfg(not(unix))]
    {
        // Windows: a `.cmd` that exits non-zero. Keep the fixture name stable.
        let cmd_path = layout.root.join(STAGING).join(format!("{BIN_NAME}.cmd"));
        fs::write(
            &cmd_path,
            "@echo off\r\necho cryptexd-fixture: incompatible/failing update fixture 1>&2\r\nexit /b 3\r\n",
        )?;
        // Also place a copy named without extension for path stability in tests that expect BIN_NAME.
        fs::copy(&cmd_path, &staging)?;
    }
    write_identity(&layout.root.join(STAGING), identity)?;
    Ok(layout)
}

/// Promote staging → active and preserve previous known-good.
pub fn activate_staged(runtime_root: &Path) -> LifecycleResult<UpdateLayout> {
    let layout = UpdateLayout::new(runtime_root)?;
    if !layout.staging_bin().is_file() {
        return Err(LifecycleError::Attention(
            "no staged candidate to activate".into(),
        ));
    }
    if !layout.active_bin().is_file() {
        return Err(LifecycleError::Attention(
            "refusing activate without an active known-good binary".into(),
        ));
    }

    clear_dir(&layout.root.join(PREVIOUS))?;
    copy_file(&layout.active_bin(), &layout.previous_bin())?;
    write_identity(
        &layout.root.join(PREVIOUS),
        &read_identity(&UpdateLayout::identity_path(&layout.root.join(ACTIVE)))?,
    )?;

    clear_dir(&layout.root.join(ACTIVE))?;
    copy_file(&layout.staging_bin(), &layout.active_bin())?;
    write_identity(
        &layout.root.join(ACTIVE),
        &read_identity(&UpdateLayout::identity_path(&layout.root.join(STAGING)))?,
    )?;
    clear_dir(&layout.root.join(STAGING))?;
    Ok(layout)
}

/// Restore previous known-good after a failed activation/start.
pub fn rollback_to_previous(runtime_root: &Path) -> LifecycleResult<UpdateLayout> {
    let layout = UpdateLayout::new(runtime_root)?;
    if !layout.previous_bin().is_file() {
        return Err(LifecycleError::Attention(
            "no previous known-good binary available for rollback".into(),
        ));
    }

    clear_dir(&layout.root.join(FAILED))?;
    if layout.active_bin().is_file() {
        copy_file(&layout.active_bin(), &layout.failed_bin())?;
        if let Ok(identity) = read_identity(&UpdateLayout::identity_path(&layout.root.join(ACTIVE)))
        {
            write_identity(&layout.root.join(FAILED), &identity)?;
        }
    }

    clear_dir(&layout.root.join(ACTIVE))?;
    copy_file(&layout.previous_bin(), &layout.active_bin())?;
    write_identity(
        &layout.root.join(ACTIVE),
        &read_identity(&UpdateLayout::identity_path(&layout.root.join(PREVIOUS)))?,
    )?;
    Ok(layout)
}

/// Synthetic non-cryptographic digest of the marker contents for evidence.
pub fn marker_digest(runtime_root: &Path) -> LifecycleResult<String> {
    let contents = fs::read_to_string(marker_path(runtime_root))
        .map_err(|error| LifecycleError::Attention(format!("synthetic marker missing: {error}")))?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    contents.hash(&mut hasher);
    Ok(format!("{:016x}", hasher.finish()))
}

pub fn marker_contents(runtime_root: &Path) -> LifecycleResult<String> {
    Ok(fs::read_to_string(marker_path(runtime_root))?)
}

#[derive(Debug, Clone)]
pub struct UpdateProofReport {
    pub experiment_id: String,
    pub runtime_root: PathBuf,
    pub initial_identity: String,
    pub initial_pid: u32,
    pub initial_marker_digest: String,
    pub incompatible_pair_rejected: bool,
    pub active_identity_after_incompatible_pair: String,
    pub compatible_pair_accepted: bool,
    pub compatible_identity: String,
    pub compatible_pid: u32,
    pub compatible_restart_ms: u128,
    pub compatible_marker_digest: String,
    pub failed_identity: String,
    pub failed_detected: bool,
    pub failed_marker_digest: String,
    pub rollback_identity: String,
    pub rollback_pid: u32,
    pub rollback_marker_digest: String,
    pub final_instance_count: u32,
}

impl UpdateProofReport {
    pub fn render_kv(&self) -> String {
        format!(
            "experiment_id={}\n\
runtime_root={}\n\
supervisor_mode=desktop_supervisor\n\
initial_identity={}\n\
initial_pid={}\n\
initial_marker_digest={}\n\
incompatible_pair_rejected={}\n\
active_identity_after_incompatible_pair={}\n\
compatible_pair_accepted={}\n\
compatible_identity={}\n\
compatible_pid={}\n\
compatible_restart_ms={}\n\
compatible_marker_digest={}\n\
compatible_marker_unchanged={}\n\
failed_identity={}\n\
failed_detected={}\n\
failed_marker_digest={}\n\
failed_marker_unchanged={}\n\
rollback_identity={}\n\
rollback_pid={}\n\
rollback_marker_digest={}\n\
rollback_marker_unchanged={}\n\
final_instance_count={}\n\
scope=isolated_p0_2_update_rollback_proof\n",
            self.experiment_id,
            self.runtime_root.display(),
            self.initial_identity,
            self.initial_pid,
            self.initial_marker_digest,
            self.incompatible_pair_rejected,
            self.active_identity_after_incompatible_pair,
            self.compatible_pair_accepted,
            self.compatible_identity,
            self.compatible_pid,
            self.compatible_restart_ms,
            self.compatible_marker_digest,
            self.compatible_marker_digest == self.initial_marker_digest,
            self.failed_identity,
            self.failed_detected,
            self.failed_marker_digest,
            self.failed_marker_digest == self.initial_marker_digest,
            self.rollback_identity,
            self.rollback_pid,
            self.rollback_marker_digest,
            self.rollback_marker_digest == self.initial_marker_digest,
            self.final_instance_count,
        )
    }
}

/// Run the deterministic compatible-update / failed-update / rollback proof.
pub fn run_update_rollback_proof(
    runtime_root: &Path,
    base_bin: &Path,
    experiment_id: &str,
) -> LifecycleResult<UpdateProofReport> {
    assert_outside_repository(runtime_root)?;
    fs::create_dir_all(runtime_root)?;
    crate::compat::write_synthetic_marker(runtime_root, "canonical-v1")?;
    let initial_marker = marker_digest(runtime_root)?;

    let layout = install_known_good(runtime_root, base_bin, "v1")?;
    let mut supervisor = DesktopSupervisor::new(runtime_root, &layout.active_bin())?;
    let initial_pid = supervisor.start_daemon()?;
    let initial_identity = layout.active_identity()?;
    if initial_identity != "v1" {
        return Err(LifecycleError::Attention(format!(
            "expected active identity v1, observed {initial_identity}"
        )));
    }
    assert_single_instance(runtime_root)?;

    // An incompatible candidate pair must fail before any staged daemon or
    // active identity changes. The protocol numbers stand in for package
    // metadata; this does not replace or execute a Desktop package.
    let incompatible_pair_rejected = match stage_compatible_candidate(
        runtime_root,
        base_bin,
        "v2",
        ComponentVersions {
            desktop_protocol: 2,
            daemon_protocol: 1,
        },
    ) {
        Err(LifecycleError::Incompatible(_)) => true,
        Err(error) => return Err(error),
        Ok(_) => false,
    };
    if !incompatible_pair_rejected {
        return Err(LifecycleError::Attention(
            "incompatible Desktop/daemon update pair was not rejected".into(),
        ));
    }
    let active_identity_after_incompatible_pair = layout.active_identity()?;
    if active_identity_after_incompatible_pair != "v1" || layout.staging_bin().exists() {
        return Err(LifecycleError::Attention(
            "incompatible update modified active identity or staged a daemon".into(),
        ));
    }
    assert_single_instance(runtime_root)?;
    if marker_digest(runtime_root)? != initial_marker {
        return Err(LifecycleError::Attention(
            "synthetic marker changed after incompatible update rejection".into(),
        ));
    }

    // Compatible replacement v2.
    supervisor.clean_stop()?;
    let _compatible_layout = stage_compatible_candidate(
        runtime_root,
        base_bin,
        "v2",
        ComponentVersions {
            desktop_protocol: 2,
            daemon_protocol: 2,
        },
    )?;
    let compatible_pair_accepted = true;
    activate_staged(runtime_root)?;
    let layout = UpdateLayout::new(runtime_root)?;
    supervisor.cryptexd_bin = layout.active_bin();
    let started = Instant::now();
    let compatible_pid = supervisor.start_daemon()?;
    let compatible_restart_ms = started.elapsed().as_millis();
    let compatible_identity = layout.active_identity()?;
    if compatible_identity != "v2" {
        return Err(LifecycleError::Attention(format!(
            "expected active identity v2, observed {compatible_identity}"
        )));
    }
    assert_single_instance(runtime_root)?;
    let compatible_marker = marker_digest(runtime_root)?;
    if compatible_marker != initial_marker {
        return Err(LifecycleError::Attention(
            "synthetic marker changed during compatible update".into(),
        ));
    }

    // Failed update: stage non-starting fixture, activate, expect start failure, rollback.
    supervisor.clean_stop()?;
    stage_failing_candidate(runtime_root, "v_bad")?;
    activate_staged(runtime_root)?;
    let layout = UpdateLayout::new(runtime_root)?;
    supervisor.cryptexd_bin = layout.active_bin();
    let failed_start = supervisor.start_daemon();
    let failed_detected = failed_start.is_err();
    if !failed_detected {
        let _ = supervisor.clean_stop();
        return Err(LifecycleError::Attention(
            "failing update fixture unexpectedly became ready".into(),
        ));
    }
    // Ensure no child remains.
    let _ = supervisor.force_kill_daemon();
    assert_no_lock(runtime_root)?;
    let failed_marker = marker_digest(runtime_root)?;
    if failed_marker != initial_marker {
        return Err(LifecycleError::Attention(
            "synthetic marker changed during failed update".into(),
        ));
    }
    let failed_identity = layout.active_identity().unwrap_or_else(|_| "v_bad".into());

    rollback_to_previous(runtime_root)?;
    let layout = UpdateLayout::new(runtime_root)?;
    supervisor.cryptexd_bin = layout.active_bin();
    let rollback_pid = supervisor.start_daemon()?;
    let rollback_identity = layout.active_identity()?;
    if rollback_identity != "v2" {
        return Err(LifecycleError::Attention(format!(
            "expected rollback identity v2 (last known good), observed {rollback_identity}"
        )));
    }
    assert_single_instance(runtime_root)?;
    let rollback_marker = marker_digest(runtime_root)?;
    if rollback_marker != initial_marker {
        return Err(LifecycleError::Attention(
            "synthetic marker changed during rollback".into(),
        ));
    }

    supervisor.clean_stop()?;
    assert_no_lock(runtime_root)?;

    Ok(UpdateProofReport {
        experiment_id: experiment_id.to_string(),
        runtime_root: runtime_root.to_path_buf(),
        initial_identity,
        initial_pid,
        initial_marker_digest: initial_marker,
        incompatible_pair_rejected,
        active_identity_after_incompatible_pair,
        compatible_pair_accepted,
        compatible_identity,
        compatible_pid,
        compatible_restart_ms,
        compatible_marker_digest: compatible_marker,
        failed_identity,
        failed_detected,
        failed_marker_digest: failed_marker,
        rollback_identity,
        rollback_pid,
        rollback_marker_digest: rollback_marker,
        final_instance_count: 0,
    })
}

fn assert_single_instance(runtime_root: &Path) -> LifecycleResult<()> {
    let obs = crate::instance::observe_instance(runtime_root)?;
    if !obs.lock_held {
        return Err(LifecycleError::Attention(
            "expected single daemon instance lock to be held".into(),
        ));
    }
    Ok(())
}

fn assert_no_lock(runtime_root: &Path) -> LifecycleResult<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        let obs = crate::instance::observe_instance(runtime_root)?;
        if !obs.lock_held {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(LifecycleError::Attention(
        "instance lock still held after failed update / stop".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(label: &str) -> PathBuf {
        crate::test_temp::exclusive_temp_dir(&format!("cryptex-update-{label}"))
    }

    fn fake_bin(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        #[cfg(unix)]
        {
            fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).unwrap();
        }
        #[cfg(not(unix))]
        {
            fs::write(&path, "@echo off\r\nexit /b 0\r\n").unwrap();
        }
        path
    }

    #[test]
    fn stage_activate_rollback_preserves_identities() {
        let root = temp_root("stage");
        let good = fake_bin(&root, "good.bin");
        install_known_good(&root, &good, "v1").expect("install");
        assert_eq!(
            UpdateLayout::new(&root).unwrap().active_identity().unwrap(),
            "v1"
        );
        stage_candidate(&root, &good, "v2").expect("stage");
        activate_staged(&root).expect("activate");
        let layout = UpdateLayout::new(&root).unwrap();
        assert_eq!(layout.active_identity().unwrap(), "v2");
        assert_eq!(layout.previous_identity().unwrap().as_deref(), Some("v1"));
        stage_failing_candidate(&root, "v_bad").expect("bad");
        activate_staged(&root).expect("activate bad");
        assert_eq!(
            UpdateLayout::new(&root).unwrap().active_identity().unwrap(),
            "v_bad"
        );
        rollback_to_previous(&root).expect("rollback");
        assert_eq!(
            UpdateLayout::new(&root).unwrap().active_identity().unwrap(),
            "v2"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn incompatible_pair_is_rejected_before_staging_or_replacing_active() {
        let root = temp_root("incompatible-pair");
        let good = fake_bin(&root, "good.bin");
        let layout = install_known_good(&root, &good, "v1").expect("install");
        let error = stage_compatible_candidate(
            &root,
            &good,
            "v2",
            ComponentVersions {
                desktop_protocol: 2,
                daemon_protocol: 1,
            },
        )
        .expect_err("mismatched candidate must be rejected");
        assert!(matches!(error, LifecycleError::Incompatible(_)));
        assert_eq!(layout.active_identity().expect("active identity"), "v1");
        assert!(!layout.staging_bin().exists());
        assert_eq!(layout.previous_identity().expect("previous"), None);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn marker_digest_stable_for_same_contents() {
        let root = temp_root("marker");
        crate::compat::write_synthetic_marker(&root, "canonical-v1").unwrap();
        let a = marker_digest(&root).unwrap();
        let b = marker_digest(&root).unwrap();
        assert_eq!(a, b);
        let _ = fs::remove_dir_all(&root);
    }
}
