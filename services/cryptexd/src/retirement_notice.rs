//! Portable copy of one original terminal frame. The index is an untrusted
//! locator. The receiving daemon verifies its consecutive position, the signed
//! predecessor, and the sender's key from the receiver's own trust history.
use crate::import_staging::ImportStagingError;
use cryptex_core::DeviceId;
use serde::{Deserialize, Serialize};

pub const MAX_NOTICE_BYTES: usize = 16 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notice {
    segment_index: u64,
    signed_record: String,
}

impl Notice {
    pub fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() > MAX_NOTICE_BYTES {
            return Err("notice_too_large");
        }
        let text = std::str::from_utf8(bytes).map_err(|_| "invalid_notice")?;
        cryptex_events::reject_duplicate_json_keys(text).map_err(|_| "invalid_notice")?;
        let notice: Self = serde_json::from_str(text).map_err(|_| "invalid_notice")?;
        notice.device().map_err(|_| "invalid_notice")?;
        Ok(notice)
    }

    fn device(&self) -> Result<DeviceId, ImportStagingError> {
        let invalid = || ImportStagingError::Invalid("invalid retirement notice".into());
        if self.signed_record.len() > MAX_NOTICE_BYTES / 2 {
            return Err(invalid());
        }
        cryptex_events::reject_duplicate_json_keys(&self.signed_record).map_err(|_| invalid())?;
        let record: serde_json::Value =
            serde_json::from_str(&self.signed_record).map_err(|_| invalid())?;
        if record["event_type"] != "security.device_self_retired" || record["schema_version"] != 3 {
            return Err(invalid());
        }
        DeviceId::new(record["device_id"].as_str().ok_or_else(invalid)?.to_owned())
            .map_err(|_| invalid())
    }

    pub(crate) fn frame(&self) -> Result<(DeviceId, u64, Vec<u8>), ImportStagingError> {
        let device = self.device()?;
        let header = cryptex_events::encode_frame_header(self.signed_record.len() as u32)
            .map_err(|_| ImportStagingError::Invalid("invalid retirement frame".into()))?;
        Ok((
            device,
            self.segment_index,
            [header.as_slice(), self.signed_record.as_bytes()].concat(),
        ))
    }
}

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
pub(crate) fn export(
    root: &std::path::Path,
    admission: &crate::runtime_admission::RuntimeAdmission,
    store: &dyn crate::secure_store::SecureStore,
    env: crate::secure_store::SecureStoreEnvironment,
) -> Result<Notice, &'static str> {
    use crate::import_staging::export_fs::Directory;
    use cryptex_events::TrustedDaemonKeyResolver;
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let permit = admission.enter(root).map_err(|_| "notice_held")?;
    let pin = Directory::open(root).map_err(|_| "notice_storage")?;
    let brain =
        crate::identity_runtime::load_brain_identity(root).map_err(|_| "notice_identity")?;
    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        root,
        &brain.brain_id,
        store,
        env,
    )
    .map_err(|_| "notice_identity")?;
    let anchor = resolver.local_trust_anchor().ok_or("notice_identity")?;
    let snapshot =
        crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(
            root,
            &brain.brain_id,
            &anchor,
            Some(std::time::Instant::now() + std::time::Duration::from_secs(20)),
        )
        .map_err(|_| "notice_held")?;
    let packs = snapshot
        .export_packs(|| permit.check_namespace().is_ok())
        .map_err(|_| "notice_held")?;
    let mut candidates = packs
        .iter()
        .filter(|p| p.device == *anchor.device_id() && p.retirement_only);
    let pack = candidates.next().ok_or("no_terminal_only_segment")?;
    if candidates.next().is_some() || pack.length > (MAX_NOTICE_BYTES / 2) as u64 {
        return Err("invalid_terminal_segment");
    }
    let dir = pin
        .child("canonical", false)
        .and_then(|p| p.child("events", false))
        .and_then(|p| p.child(&pack.device.as_filename_stem(), false))
        .map_err(|_| "notice_storage")?;
    let sealed = format!("{:020}.seg", pack.index);
    let active = !dir.exists(&sealed).map_err(|_| "notice_storage")?;
    let name = if active {
        format!("{:020}.open", pack.index)
    } else {
        sealed
    };
    let file = dir.open_file(&name, false).map_err(|_| "notice_storage")?;
    let before = crate::import_staging::source_snapshot(&file).map_err(|_| "notice_storage")?;
    let mut bytes = Vec::new();
    (&file)
        .take(pack.length + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "notice_storage")?;
    if !before.is_regular_file
        || before.link_count != 1
        || bytes.len() as u64 != pack.length
        || Sha256::digest(&bytes)[..] != pack.digest
        || crate::import_staging::source_snapshot(&file).map_err(|_| "notice_storage")? != before
    {
        return Err("notice_source_changed");
    }
    admission
        .validate_operation(root, &permit)
        .map_err(|_| "notice_held")?;
    if active {
        crate::canonical_writer::seal_export_checkpoint(root, pack, &dir, &file)
            .map_err(|_| "notice_storage")?;
    }
    let notice = Notice {
        segment_index: pack.index,
        signed_record: std::str::from_utf8(
            bytes
                .get(cryptex_events::FRAME_HEADER_BYTES..)
                .ok_or("notice_invalid")?,
        )
        .map_err(|_| "notice_invalid")?
        .to_owned(),
    };
    if notice.frame().map_err(|_| "notice_invalid")?.2 != bytes {
        return Err("notice_invalid");
    }
    pin.validate().map_err(|_| "notice_storage")?;
    Ok(notice)
}

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
pub(crate) const OPERATION: &str = "connections.retirement_notice";

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    operation: String,
    request_id: String,
    notice: Option<Notice>,
}

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    request_id: String,
    result: Result<Option<Notice>, String>,
}

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
pub(crate) fn exchange(
    notice: Option<Notice>,
    send: impl FnOnce(&[u8]) -> std::io::Result<Vec<u8>>,
) -> std::io::Result<Option<Notice>> {
    use std::io::{Error, ErrorKind};
    let mut nonce = [0u8; 16];
    getrandom::getrandom(&mut nonce).map_err(|_| Error::other("notice_nonce"))?;
    let request = Request {
        operation: OPERATION.into(),
        request_id: nonce.iter().map(|b| format!("{b:02x}")).collect(),
        notice,
    };
    let bytes = send(&serde_json::to_vec(&request)?)?;
    let invalid = || Error::new(ErrorKind::InvalidData, "invalid_notice_reply");
    if bytes.len() > MAX_NOTICE_BYTES * 2 {
        return Err(invalid());
    }
    cryptex_events::reject_duplicate_json_keys(std::str::from_utf8(&bytes).map_err(|_| invalid())?)
        .map_err(|_| invalid())?;
    let reply: Response = serde_json::from_slice(&bytes)?;
    if reply.request_id != request.request_id {
        return Err(invalid());
    }
    let result = reply.result.map_err(Error::other)?;
    if result.is_some() == request.notice.is_some() {
        return Err(invalid());
    }
    Ok(result)
}

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
pub(crate) fn reply(
    bytes: &[u8],
    root: &std::path::Path,
    instance: &mut crate::instance::InstanceLock,
    store: &dyn crate::secure_store::SecureStore,
    env: crate::secure_store::SecureStoreEnvironment,
) -> std::io::Result<(Vec<u8>, Option<crate::runtime_admission::OperationPermit>)> {
    use std::io::Error;
    if bytes.len() > MAX_NOTICE_BYTES * 2 {
        return Err(Error::other("invalid_notice_request"));
    }
    cryptex_events::reject_duplicate_json_keys(std::str::from_utf8(bytes).map_err(Error::other)?)
        .map_err(|_| Error::other("invalid_notice_request"))?;
    let request: Request = serde_json::from_slice(bytes)?;
    if request.operation != OPERATION
        || request.request_id.len() != 32
        || !request.request_id.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(Error::other("invalid_notice_request"));
    }
    let mut result = match request.notice {
        None => export(root, &instance.admission(), store, env).map(Some),
        Some(notice) => crate::incoming_publication::publish_retirement_notice(
            root, &notice, instance, store, env,
        )
        .map(|_| None)
        .map_err(|e| match e {
            crate::incoming_publication::IncomingPublicationError::Rejected(_) => "notice_rejected",
            crate::incoming_publication::IncomingPublicationError::Admission(_) => {
                "notice_unavailable"
            }
            crate::incoming_publication::IncomingPublicationError::PolicyHeld(_) => {
                "notice_policy_hold"
            }
            _ => "notice_recovery_required",
        }),
    };
    let lease = if result.is_ok() {
        let lease = instance.admission().enter(root).ok();
        if lease.is_none() {
            result = Err("notice_held");
        }
        lease
    } else {
        None
    };
    Ok((
        serde_json::to_vec(&Response {
            request_id: request.request_id,
            result: result.map_err(String::from),
        })?,
        lease,
    ))
}
