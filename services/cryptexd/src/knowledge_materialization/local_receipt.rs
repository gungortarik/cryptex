//! Offline, run-bound local evidence. Not a signed remote receipt, transport
//! authorization, recovery operation or Protected-state assertion.
use super::*;
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    Arguments,
    UnsafeRuntime,
    RunningOrUnavailable,
    RecoveryRequired,
    IdentityMismatch,
    KeyUnavailable,
    HistoryUnavailable,
    PeerUnavailable,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "local_verification_{}",
            match self {
                Self::Arguments => "arguments",
                Self::UnsafeRuntime => "unsafe_runtime",
                Self::RunningOrUnavailable => "running_or_unavailable",
                Self::RecoveryRequired => "recovery_required",
                Self::IdentityMismatch => "identity_mismatch",
                Self::KeyUnavailable => "key_unavailable",
                Self::HistoryUnavailable => "history_unavailable",
                Self::PeerUnavailable => "peer_unavailable",
            }
        )
    }
}
impl std::error::Error for Error {}

pub struct Request<'a> {
    pub run_id: &'a str,
    pub brain_id: &'a str,
    pub local_device_id: &'a str,
    pub peer_device_id: &'a str,
}

#[derive(Debug, Serialize)]
pub struct Receipt {
    pub schema: u32,
    pub purpose: &'static str,
    pub run_id: String,
    pub brain_id: String,
    pub local_device_id: String,
    pub peer_device_id: String,
    pub canonical_set_sha256: String,
    pub canonical_records: usize,
    pub canonical_devices: usize,
    pub signatures_and_chains_verified: bool,
    pub remote_durability_verified: bool,
    pub network_delivery_verified: bool,
    pub derived_results_verified: bool,
}

/// Requires normal application shutdown. Pins the existing runtime and obtains
/// the same process exclusion as the daemon before touching OS key storage.
/// Only instance/coordination locks may be created/updated; canonical packs,
/// journals, projections, settings and credentials are never repaired or changed.
pub fn inspect<S: SecureStore + ?Sized>(
    root: &Path,
    store: &S,
    environment: SecureStoreEnvironment,
    request: &Request<'_>,
) -> Result<Receipt, Error> {
    for id in [
        request.run_id,
        request.brain_id,
        request.local_device_id,
        request.peer_device_id,
    ] {
        let parsed = cryptex_core::validate::parse_uuid_str(id).map_err(|_| Error::Arguments)?;
        if parsed != id {
            return Err(Error::Arguments);
        }
    }
    if request.local_device_id == request.peer_device_id {
        return Err(Error::Arguments);
    }
    let _root_pins = crate::import_staging::ImportStagingSession::pin_existing_runtime(root)
        .map_err(|_| Error::UnsafeRuntime)?;
    let _instance =
        crate::instance::try_acquire_instance(root).map_err(|_| Error::RunningOrUnavailable)?;
    let _import_pins = crate::import_staging::ImportStagingSession::pin_clean_import_state(root)
        .map_err(|_| Error::RecoveryRequired)?;
    let brain =
        crate::identity_runtime::load_brain_identity(root).map_err(|_| Error::IdentityMismatch)?;
    let local =
        load_local_device_identity(root, &brain.brain_id).map_err(|_| Error::IdentityMismatch)?;
    if brain.brain_id != request.brain_id || local.device_id != request.local_device_id {
        return Err(Error::IdentityMismatch);
    }
    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        root,
        &brain.brain_id,
        store,
        environment,
    )
    .map_err(|_| Error::KeyUnavailable)?;
    let anchor = resolver.local_trust_anchor().ok_or(Error::KeyUnavailable)?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let snapshot = scan_verified_canonical_from_local_anchor_until(
        root,
        &brain.brain_id,
        &anchor,
        Some(deadline),
    )
    .map_err(|_| Error::HistoryUnavailable)?;
    let (history, trust) = snapshot
        .verified_trust_context()
        .ok_or(Error::HistoryUnavailable)?;
    let peer = DeviceId::new(request.peer_device_id).map_err(|_| Error::Arguments)?;
    if !snapshot.devices.contains(&peer)
        || !history.directly_approved_peers.contains(&peer)
        || !trust.trusted_devices.contains(&peer)
        || !snapshot
            .security_history
            .records()
            .iter()
            .any(|record| record.device_id() == peer.as_str())
    {
        return Err(Error::PeerUnavailable);
    }
    // Hash every authenticated physical record, including security controls and
    // retained beyond-cutoff history. Sorting fixed-size SHA-256 hashes makes
    // this independent of pack boundaries, .open/.seg names and filesystem order.
    // It is a diagnostic checksum, never a new signature/authentication scheme.
    let mut hashes = Vec::<[u8; 32]>::with_capacity(snapshot.security_frames.len());
    for frame in &snapshot.security_frames {
        if Instant::now() >= deadline {
            return Err(Error::HistoryUnavailable);
        }
        hashes.push(Sha256::digest(&frame.bytes).into());
    }
    hashes.sort_unstable();
    let mut digest = Sha256::new();
    digest.update(b"cryptex-local-canonical-set-v1\0");
    digest.update(brain.brain_id.as_bytes());
    digest.update((hashes.len() as u64).to_be_bytes());
    for hash in hashes {
        digest.update(hash);
    }
    if Instant::now() >= deadline {
        return Err(Error::HistoryUnavailable);
    }
    if !snapshot.locks_runtime(root) {
        return Err(Error::UnsafeRuntime);
    }
    Ok(Receipt {
        schema: 1,
        purpose: "cryptex-local-canonical-verification",
        run_id: request.run_id.into(),
        brain_id: brain.brain_id,
        local_device_id: local.device_id,
        peer_device_id: request.peer_device_id.into(),
        canonical_set_sha256: format!("{:x}", digest.finalize()),
        canonical_records: snapshot.security_frames.len(),
        canonical_devices: snapshot.devices.len(),
        signatures_and_chains_verified: true,
        remote_durability_verified: false,
        network_delivery_verified: false,
        derived_results_verified: false,
    })
}

#[cfg(test)]
mod tests;
