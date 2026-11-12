//! Explicit synthetic pilot only: signature verification is not trust admission.
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use cryptex_core::DeviceId;
use cryptex_events::{
    Classification, EventPayload, FRAME_HEADER_BYTES, validate_accepted_record_bytes,
};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};

use crate::canonical_writer::{AppendMemoryNoteArgs, CanonicalWriterConfig, append_memory_note};
use crate::secure_store::{SecureStore, SecureStoreEnvironment};

const NOTE: &str = "Synthetic signed transfer probe. No personal data.";
const SCOPE: &str = "synthetic_signature_only_not_trust";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeError {
    Io,
    Limit,
    Descriptor,
    Frame,
    Integrity,
    Binding,
    Setup,
    Enrollment,
    Append,
    Key,
    StorePlatform(i64),
    NotApproved,
    NonSynthetic,
}
impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Self::StorePlatform(code) = self {
            return write!(f, "probe_secure_store_platform_{code}");
        }
        f.write_str(match self {
            Self::Io => "probe_io_failed",
            Self::Limit => "probe_size_limit",
            Self::Descriptor => "probe_invalid_descriptor",
            Self::Frame => "probe_invalid_frame",
            Self::Integrity => "probe_integrity_failed",
            Self::Binding => "probe_binding_mismatch",
            Self::Setup => "probe_setup_failed",
            Self::Enrollment => "probe_enrollment_failed",
            Self::Append => "probe_append_failed",
            Self::Key => "probe_key_unavailable",
            Self::NotApproved => "probe_peer_not_approved",
            Self::NonSynthetic => "probe_non_synthetic_source",
            Self::StorePlatform(_) => unreachable!(),
        })
    }
}
impl std::error::Error for ProbeError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeDescriptor {
    pub schema: u32,
    pub scope: String,
    pub brain_id: String,
    pub device_id: String,
    pub event_id: String,
    pub public_key: String,
}

#[derive(Debug, Serialize)]
pub struct ProbeReport {
    pub scope: &'static str,
    pub outcome: &'static str,
    pub event_count: u32,
    pub trust_activated: bool,
    pub published: bool,
}

/// Caller owns the fresh marked pilot runtime's instance guard. No keys exported.
pub fn create<S: SecureStore + ?Sized>(
    root: &Path,
    store: &S,
    brain_id: &str,
    event_id: &str,
) -> Result<(), ProbeError> {
    let brain = cryptex_core::BrainIdentity::new(brain_id, "2026-11-08T00:00:00Z")
        .map_err(|_| ProbeError::Setup)?;
    crate::identity_runtime::write_brain_identity(root, &brain).map_err(|_| ProbeError::Setup)?;
    let enrollment =
        crate::local_device_identity::enroll_local_device(root, store, SecureStoreEnvironment::Dev)
            .map_err(|error| match error {
                crate::local_device_identity::LocalDeviceIdentityError::EventSigningKey(
                    crate::event_signing_key::EventSigningKeyError::SecureStore(
                        crate::secure_store::SecureStoreError::Platform { code, .. },
                    ),
                ) => ProbeError::StorePlatform(code),
                _ => ProbeError::Enrollment,
            })?;
    let device = DeviceId::new(enrollment.device_id).map_err(|_| ProbeError::Setup)?;
    append_memory_note(
        store,
        SecureStoreEnvironment::Dev,
        AppendMemoryNoteArgs {
            runtime_root: root,
            brain_id,
            device_id: &device,
            classification: Classification::Public,
            text: NOTE,
            event_id: Some(event_id),
            config: &CanonicalWriterConfig {
                seal_event_threshold: 1,
            },
        },
    )
    .map_err(|_| ProbeError::Append)?;
    let (public, _) = crate::event_signing_key::load_existing_event_verifying_key(
        store,
        SecureStoreEnvironment::Dev,
        root,
        &device,
    )
    .map_err(|_| ProbeError::Key)?;
    let descriptor = ProbeDescriptor {
        schema: 1,
        scope: SCOPE.into(),
        brain_id: brain_id.into(),
        device_id: device.as_str().into(),
        event_id: event_id.into(),
        public_key: public.public_key,
    };
    let bytes = serde_json::to_vec(&descriptor).map_err(|_| ProbeError::Setup)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(root.join("signed-probe.json"))
        .map_err(|_| ProbeError::Io)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| ProbeError::Io)?;
    let source = cryptex_storage::sealed_segment_path(root, &device, 0);
    verify_files(&source, &root.join("signed-probe.json"))?;
    Ok(())
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, ProbeError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ProbeError::Io)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ProbeError::Io);
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| ProbeError::Io)?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ProbeError::Io)?;
    if bytes.len() as u64 > limit {
        return Err(ProbeError::Limit);
    }
    Ok(bytes)
}

/// Pure bounded observation: no runtime initialization, store or canonical writes.
pub fn verify_files(segment: &Path, descriptor: &Path) -> Result<ProbeReport, ProbeError> {
    let bytes = read_bounded(segment, 1024 * 1024)?;
    let descriptor = read_bounded(descriptor, 4096)?;
    verify(&bytes, &descriptor)
}

pub fn verify(bytes: &[u8], descriptor: &[u8]) -> Result<ProbeReport, ProbeError> {
    if bytes.len() > 1024 * 1024 || descriptor.len() > 4096 {
        return Err(ProbeError::Limit);
    }
    let desc: ProbeDescriptor =
        serde_json::from_slice(descriptor).map_err(|_| ProbeError::Descriptor)?;
    if desc.schema != 1 || desc.scope != SCOPE || desc.public_key.len() != 64 {
        return Err(ProbeError::Descriptor);
    }
    let mut key = [0u8; 32];
    for (slot, pair) in key
        .iter_mut()
        .zip(desc.public_key.as_bytes().as_chunks::<2>().0)
    {
        if !pair
            .iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
        {
            return Err(ProbeError::Descriptor);
        }
        *slot = u8::from_str_radix(
            std::str::from_utf8(pair).map_err(|_| ProbeError::Descriptor)?,
            16,
        )
        .map_err(|_| ProbeError::Descriptor)?;
    }
    let key = VerifyingKey::from_bytes(&key).map_err(|_| ProbeError::Descriptor)?;
    let (_, body_len) =
        cryptex_events::decode_frame_header(bytes).map_err(|_| ProbeError::Frame)?;
    if bytes.len() != FRAME_HEADER_BYTES + body_len as usize {
        return Err(ProbeError::Frame);
    }
    let record = validate_accepted_record_bytes(&bytes[FRAME_HEADER_BYTES..], &key)
        .map_err(|_| ProbeError::Integrity)?;
    if record.brain_id != desc.brain_id
        || record.device_id != desc.device_id
        || record.event_id != desc.event_id
        || record
            .device_seq
            .to_u64()
            .map_err(|_| ProbeError::Binding)?
            != 1
        || record.classification != Classification::Public
        || !matches!(record.payload, EventPayload::MemoryNote(ref note) if note.text == NOTE)
    {
        return Err(ProbeError::Binding);
    }
    cryptex_events::check_first_event_prev_hash(&record.prev_content_hash)
        .map_err(|_| ProbeError::Binding)?;
    Ok(ProbeReport {
        scope: SCOPE,
        outcome: "signature_verified",
        event_count: 1,
        trust_activated: false,
        published: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Vec<u8>, Vec<u8>) {
        let root = crate::test_temp::exclusive_temp_dir("signed-probe");
        let store = crate::FakeSecureStore::new();
        create(
            &root,
            &store,
            "11111111-1111-4111-8111-111111111111",
            "22222222-2222-4222-8222-222222222222",
        )
        .unwrap();
        let descriptor = fs::read(root.join("signed-probe.json")).unwrap();
        let desc: ProbeDescriptor = serde_json::from_slice(&descriptor).unwrap();
        let segment = fs::read(cryptex_storage::sealed_segment_path(
            &root,
            &DeviceId::new(desc.device_id).unwrap(),
            0,
        ))
        .unwrap();
        fs::remove_dir_all(root).unwrap();
        (segment, descriptor)
    }
    #[test]
    fn daemon_sealed_event_verifies_without_admitting_trust() {
        let (segment, descriptor) = fixture();
        let report = verify(&segment, &descriptor).unwrap();
        assert!(!report.published && !report.trust_activated);
        assert_eq!(report.event_count, 1);
    }
    #[test]
    fn tampered_record_rejects_without_hash_sidecar_check() {
        let (mut segment, descriptor) = fixture();
        let offset = segment
            .windows(NOTE.len())
            .position(|w| w == NOTE.as_bytes())
            .unwrap();
        segment[offset] ^= 1;
        assert_eq!(
            verify(&segment, &descriptor).unwrap_err(),
            ProbeError::Integrity
        );
    }
    #[test]
    fn signature_bit_and_wrong_key_reject() {
        let (segment, descriptor) = fixture();
        let mut value: serde_json::Value =
            serde_json::from_slice(&segment[FRAME_HEADER_BYTES..]).unwrap();
        let mut sig = value["signature"].as_str().unwrap().as_bytes().to_vec();
        sig[0] = if sig[0] == b'0' { b'1' } else { b'0' };
        value["signature"] = String::from_utf8(sig).unwrap().into();
        let body = serde_json::to_vec(&value).unwrap();
        let mut corrupt = cryptex_events::encode_frame_header(body.len() as u32)
            .unwrap()
            .to_vec();
        corrupt.extend(body);
        assert_eq!(
            verify(&corrupt, &descriptor).unwrap_err(),
            ProbeError::Integrity
        );
        let mut desc: ProbeDescriptor = serde_json::from_slice(&descriptor).unwrap();
        desc.public_key = ed25519_dalek::SigningKey::from_bytes(&[42; 32])
            .verifying_key()
            .to_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            verify(&segment, &serde_json::to_vec(&desc).unwrap()).unwrap_err(),
            ProbeError::Integrity
        );
    }
    #[test]
    fn frame_limits_and_binding_reject() {
        let (segment, descriptor) = fixture();
        assert_eq!(
            verify(&segment[..5], &descriptor).unwrap_err(),
            ProbeError::Frame
        );
        let mut extra = segment.clone();
        extra.push(0);
        assert_eq!(verify(&extra, &descriptor).unwrap_err(), ProbeError::Frame);
        let mut desc: ProbeDescriptor = serde_json::from_slice(&descriptor).unwrap();
        desc.brain_id = "33333333-3333-4333-8333-333333333333".into();
        assert_eq!(
            verify(&segment, &serde_json::to_vec(&desc).unwrap()).unwrap_err(),
            ProbeError::Binding
        );
        assert_eq!(
            verify(&vec![0; 1024 * 1024 + 1], &descriptor).unwrap_err(),
            ProbeError::Limit
        );
        assert_eq!(
            verify(&segment, br#"{"schema":1,"schema":1}"#).unwrap_err(),
            ProbeError::Descriptor
        );
    }
}
