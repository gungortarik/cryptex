//! Daemon-only export of complete verified packs. The exchange tree is disposable
//! and untrusted; publication here never means durable acceptance by a peer.
use crate::import_staging::{export_fs::Directory, source_snapshot};
use crate::knowledge_materialization::export_plan::ExportPack;
use crate::runtime_admission::{RuntimeAdmission, TransportGuard};
use crate::secure_store::{SecureStore, SecureStoreEnvironment};
use cryptex_events::TrustedDaemonKeyResolver;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub(crate) mod reader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    Deferred,
    Authority,
    SourceChanged,
    DestinationConflict,
    UnsafeFilesystem,
    Io(std::io::ErrorKind, Option<i32>),
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.kind(), e.raw_os_error())
    }
}
impl From<crate::import_staging::ImportStagingError> for Error {
    fn from(_: crate::import_staging::ImportStagingError) -> Self {
        Self::UnsafeFilesystem
    }
}

impl From<crate::import_staging::export_fs::Error> for Error {
    fn from(e: crate::import_staging::export_fs::Error) -> Self {
        match e {
            crate::import_staging::export_fs::Error::Unsafe => Self::UnsafeFilesystem,
            crate::import_staging::export_fs::Error::Io(kind, code) => Self::Io(kind, code),
        }
    }
}

#[derive(Default)]
pub(crate) struct Exporter {
    after: Option<(String, u64)>,
    reader_check: reader::ReaderCheck,
    last_reader_status: Option<reader::Status>,
}
#[derive(Default, Debug)]
pub(crate) struct Report {
    pub(crate) copied: usize,
    pub(crate) present: usize,
    pub(crate) sealed: usize,
    pub(crate) withheld: usize,
    pub(crate) attachment_waiting_for_reader: usize,
    pub(crate) attachment_transfer_disabled: usize,
}

#[cfg(test)]
thread_local! { static FAULT: std::cell::Cell<Option<u8>> = const { std::cell::Cell::new(None) }; }
fn checkpoint(_point: u8) -> Result<(), Error> {
    #[cfg(test)]
    if FAULT.with(|fault| {
        if fault.get() == Some(_point) {
            fault.set(None);
            true
        } else {
            false
        }
    }) {
        return Err(Error::Io(std::io::ErrorKind::StorageFull, None));
    }
    Ok(())
}

fn authority(_stage: &'static str) -> Error {
    // Native fixtures need the failing boundary, never raw errors, paths, keys
    // or note bodies. Production error behavior is unchanged.
    #[cfg(test)]
    eprintln!("component=sealed_export_test authority_stage={_stage}");
    Error::Authority
}

impl Exporter {
    pub(crate) fn end_reader_session(&mut self) {
        self.reader_check = reader::ReaderCheck::default();
        self.last_reader_status = None;
    }

    pub(crate) fn tick(
        &mut self,
        root: &Path,
        admission: &RuntimeAdmission,
        guard: &TransportGuard,
        identity: &str,
        credentials: (&dyn SecureStore, SecureStoreEnvironment),
        stop: &AtomicBool,
    ) -> Result<Report, Error> {
        let (store, environment) = credentials;
        let deadline = Instant::now() + Duration::from_secs(20);
        let permit = admission.enter(root).map_err(|_| Error::Deferred)?;
        let root_pin = Directory::open(root)?;
        let check = || {
            if stop.load(Ordering::Acquire) || Instant::now() >= deadline {
                return Err(Error::Deferred);
            }
            admission
                .validate_operation(root, &permit)
                .map_err(|_| authority("admission"))?;
            guard.check().map_err(|_| Error::Deferred)?;
            root_pin.validate()?;
            Ok(())
        };
        check()?;
        let brain =
            crate::identity_runtime::load_brain_identity(root).map_err(|_| authority("brain"))?;
        let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
            root,
            &brain.brain_id,
            store,
            environment,
        )
        .map_err(|_| authority("local_key"))?;
        let anchor = resolver
            .local_trust_anchor()
            .ok_or_else(|| authority("anchor"))?;
        let snapshot = crate::knowledge_materialization::scan_verified_canonical_from_local_anchor_until(root, &brain.brain_id, &anchor, Some(deadline)).map_err(|e| match e {
            crate::knowledge_materialization::KnowledgeMaterializationError::DeadlineExceeded => Error::Deferred,
            crate::knowledge_materialization::KnowledgeMaterializationError::Canonical(_) => authority("scan_canonical"),
            crate::knowledge_materialization::KnowledgeMaterializationError::Storage(_) => authority("scan_storage"),
            crate::knowledge_materialization::KnowledgeMaterializationError::Verification(_) => authority("scan_verification"),
            crate::knowledge_materialization::KnowledgeMaterializationError::Knowledge(_) => authority("scan_knowledge"),
            crate::knowledge_materialization::KnowledgeMaterializationError::InvalidInput(_) => authority("scan_input"),
        })?;
        let peers = super::exchange_policy::ExchangePeers::from_snapshot(&snapshot, root, identity)
            .map_err(|_| authority("peers"))?;
        if peers.bindings().is_empty() {
            return Err(authority("no_peers"));
        }
        let check = || {
            check()?;
            peers.revalidate().map_err(|_| authority("peers_recheck"))
        };
        let packs = snapshot.export_packs(|| check().is_ok()).map_err(|e| match e { crate::knowledge_materialization::KnowledgeMaterializationError::DeadlineExceeded => Error::Deferred, _ => authority("packs") })?;
        let canonical = root_pin.child("canonical", false)?.child("events", false)?;
        let transport = root_pin.child("transport", false)?;
        let incoming = transport.child("incoming", false)?;
        let status = self
            .reader_check
            .tick(
                &snapshot,
                &peers,
                &incoming,
                &transport,
                credentials,
                &check,
            )
            .unwrap_or_else(reader::Status::Unavailable);
        check()?;
        let readers_verified = matches!(status, reader::Status::Verified(_));
        if self.last_reader_status.as_ref() != Some(&status) {
            let label = match &status {
                reader::Status::Legacy => "legacy_profile",
                reader::Status::Waiting => "waiting",
                reader::Status::Rejected => "rejected",
                reader::Status::Unavailable(reader::Error::Authority) => "authority_unavailable",
                reader::Status::Unavailable(reader::Error::Storage) => "storage_unavailable",
                reader::Status::Unavailable(reader::Error::LocalReader) => {
                    "local_reader_unavailable"
                }
                reader::Status::Unavailable(reader::Error::Message) => "message_unavailable",
                reader::Status::Verified(_) => "verified",
            };
            eprintln!(
                "component=m9_reader_check status={label} peers={}",
                peers.bindings().len()
            );
            self.last_reader_status = Some(status);
        }
        let scratch = transport.child("export-staging", true)?;
        if scratch.exists("segment.tmp")? {
            let old = scratch.open_file("segment.tmp", false)?;
            check()?;
            scratch.discard_scratch(old)?;
        }
        let mut report = Report::default();
        let mut budget = crate::import_limits::ImportBatchBudget::empty();
        let mut remaining = false;
        for pack in &packs {
            let key = (pack.device.as_str().to_owned(), pack.index);
            if self.after.as_ref().is_some_and(|after| &key <= after) {
                continue;
            }
            if budget.files() == crate::import_limits::MAX_IMPORT_FILES_PER_BATCH
                || budget.bytes().saturating_add(pack.length)
                    > crate::import_limits::MAX_IMPORT_BATCH_BYTES
            {
                remaining = true;
                break;
            }
            budget
                .admit_file(pack.length)
                .map_err(|_| Error::SourceChanged)?;
            if pack.contains_attachment && !readers_verified {
                report.attachment_waiting_for_reader += 1;
                report.withheld += 1;
                self.after = Some(key);
                continue;
            }
            if !(pack.eligible || pack.retirement_only) {
                if pack.contains_attachment {
                    report.attachment_transfer_disabled += 1;
                }
                // Never slice/re-sign a physical pack at a reduced cutoff.
                // Withhold the complete pack without starving unrelated peers.
                report.withheld += 1;
                self.after = Some(key);
                continue;
            }
            check()?;
            let source_dir = canonical.child(&pack.device.as_filename_stem(), false)?;
            let sealed = format!("{:020}.seg", pack.index);
            let active = format!("{:020}.open", pack.index);
            let was_active = !source_dir.exists(&sealed)?;
            if was_active && pack.local_lock().is_none() {
                return Err(Error::SourceChanged);
            }
            let mut source =
                source_dir.open_file(if was_active { &active } else { &sealed }, false)?;
            verify(&mut source, pack, &check).map_err(|e| {
                if e == Error::DestinationConflict {
                    Error::SourceChanged
                } else {
                    e
                }
            })?;
            if was_active {
                check()?;
                crate::canonical_writer::seal_export_checkpoint(root, pack, &source_dir, &source)?;
                report.sealed += 1;
            }
            checkpoint(0)?;
            let target = incoming.child(&pack.device.as_filename_stem(), true)?;
            if target.exists(&sealed)? {
                let mut existing = target.open_file(&sealed, false)?;
                verify(&mut existing, pack, &check)?;
                report.present += 1;
            } else {
                let mut staged = scratch.open_file("segment.tmp", true)?;
                let initial = source_snapshot(&source)?;
                source.seek(SeekFrom::Start(0))?;
                let mut buffer = [0u8; 64 * 1024];
                let mut left = pack.length;
                while left > 0 {
                    check()?;
                    let n = source.read(&mut buffer[..left.min(64 * 1024) as usize])?;
                    if n == 0 {
                        return Err(Error::SourceChanged);
                    }
                    staged.write_all(&buffer[..n])?;
                    checkpoint(1)?;
                    left -= n as u64;
                }
                if source_snapshot(&source)? != initial {
                    return Err(Error::SourceChanged);
                }
                crate::import_staging::export_fs::flush(&staged)?;
                verify(&mut staged, pack, &check)?;
                check()?;
                checkpoint(2)?;
                #[cfg(test)]
                tests::publish_racing_delivery();
                match scratch.promote("segment.tmp", &staged, &target, &sealed) {
                    Ok(()) => {}
                    Err(crate::import_staging::export_fs::Error::Io(
                        std::io::ErrorKind::AlreadyExists,
                        _,
                    )) => {
                        // A transport delivery may win the same leaf between
                        // lookup and no-replace publication. Only exact bytes
                        // count as a replay; never overwrite the winner.
                        let mut existing = target.open_file(&sealed, false)?;
                        verify(&mut existing, pack, &check)?;
                        scratch.discard_scratch(staged)?;
                        report.present += 1;
                        self.after = Some(key);
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                }
                checkpoint(3)?;
                verify(&mut staged, pack, &check)?;
                report.copied += 1;
            }
            check()?;
            self.after = Some(key);
        }
        if !remaining {
            self.after = None;
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests;

fn verify(
    file: &mut File,
    pack: &ExportPack<'_>,
    check: &impl Fn() -> Result<(), Error>,
) -> Result<(), Error> {
    let before = source_snapshot(file)?;
    if !before.is_regular_file || before.link_count != 1 || before.length != pack.length {
        return Err(Error::DestinationConflict);
    }
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut left = pack.length;
    while left > 0 {
        check()?;
        let n = file.read(&mut buffer[..left.min(64 * 1024) as usize])?;
        if n == 0 {
            return Err(Error::DestinationConflict);
        }
        hash.update(&buffer[..n]);
        left -= n as u64;
    }
    if <[u8; 32]>::from(hash.finalize()) != pack.digest || source_snapshot(file)? != before {
        return Err(Error::DestinationConflict);
    }
    Ok(())
}
