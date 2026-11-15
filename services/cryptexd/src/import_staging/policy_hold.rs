//! Purpose-separated recovery evidence. Nothing in this module grants trust or
//! authorizes canonical publication. A receipt is replayed, never believed.

#![cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]

use super::*;
use crate::secure_store::{SecureStore, SecureStoreEnvironment};
use cryptex_events::{LocalTrustAnchor, TrustedDaemonKeyResolver};

const INTENT_NAME: &str = "policy-review.json";
const LATCH_NAME: &str = "policy-held.json";
const PURPOSE: &str = "cryptex-incoming-policy-review";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ReviewIntent {
    version: u32,
    purpose: String,
    transaction_id: String,
    brain_id: String,
    local_device_id: String,
    local_signing_key_id: String,
    local_public_key: [u8; 32],
    segments: Vec<ReviewSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ReviewSegment {
    device_id: String,
    segment_index: u64,
    length: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct HeldDecision {
    version: u32,
    purpose: String,
    intent_sha256: String,
    reason: AuthenticatedPolicyConflict,
}

fn recovery(message: &str) -> ImportStagingError {
    ImportStagingError::RecoveryConflict(message.to_owned())
}

fn journal_path(root: &Path, id: &str, suffix: &str) -> PathBuf {
    root.join("canonical/import-journal")
        .join(format!("{id}.{suffix}"))
}

fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, ImportStagingError> {
    let bytes = serde_json_canonicalizer::to_vec(value)
        .map_err(|_| recovery("policy evidence serialization failed"))?;
    if bytes.len() as u64 > MAX_PREPARED_MANIFEST_BYTES {
        return Err(recovery("policy evidence exceeds its size bound"));
    }
    Ok(bytes)
}

fn read_closed<T: serde::de::DeserializeOwned + Serialize>(
    path: &Path,
) -> Result<T, ImportStagingError> {
    let (mut file, before) = open_recovery_source(path)?;
    if before.length > MAX_PREPARED_MANIFEST_BYTES {
        return Err(recovery("policy evidence exceeds its size bound"));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_PREPARED_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != before.length || source_snapshot(&file)? != before {
        return Err(recovery("policy evidence changed while reading"));
    }
    let parsed: T = serde_json::from_slice(&bytes)
        .map_err(|_| recovery("invalid or unsupported policy evidence schema"))?;
    if canonical_bytes(&parsed)? != bytes {
        return Err(recovery("policy evidence is not closed canonical JSON"));
    }
    Ok(parsed)
}

fn persist_new(path: &Path, bytes: &[u8]) -> Result<(), ImportStagingError> {
    let parent = path
        .parent()
        .ok_or_else(|| recovery("policy evidence has no parent"))?;
    let _pins = inspect_directory_chain(parent, true)?;
    let temporary = path.with_extension("json.pending");
    let mut file = create_import_file(&temporary)?;
    file.write_all(bytes)?;
    sync_import_file(&file)?;
    drop(file);
    publish_staged_no_replace(&temporary, path)?;
    sync_staged_parent(path)?;
    Ok(())
}

impl ReviewIntent {
    fn validate(&self) -> Result<(), ImportStagingError> {
        if self.version != 1 || self.purpose != PURPOSE {
            return Err(recovery("unsupported policy review purpose or version"));
        }
        for id in [&self.transaction_id, &self.brain_id] {
            if cryptex_core::validate::parse_uuid_str(id).is_err() || id.to_ascii_lowercase() != *id
            {
                return Err(recovery("noncanonical policy review identity"));
            }
        }
        let key = ed25519_dalek::VerifyingKey::from_bytes(&self.local_public_key)
            .map_err(|_| recovery("invalid policy review anchor"))?;
        if cryptex_events::signing_key_id_from_verifying_key(&key)
            .map_err(|_| recovery("invalid policy review key binding"))?
            .as_str()
            != self.local_signing_key_id
            || DeviceId::new(self.local_device_id.clone()).is_err()
        {
            return Err(recovery("invalid policy review identity binding"));
        }
        if self.segments.is_empty()
            || self.segments.len() > crate::import_limits::MAX_IMPORT_FILES_PER_BATCH
        {
            return Err(recovery("policy review inventory is outside its bound"));
        }
        let mut budget = ImportBatchBudget::empty();
        let mut previous = None;
        for segment in &self.segments {
            let device = DeviceId::new(segment.device_id.clone())
                .map_err(|_| recovery("invalid policy review device"))?;
            if device.as_str() != segment.device_id
                || segment.sha256.len() != 64
                || !segment
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(recovery("noncanonical policy review inventory"));
            }
            let order = (segment.device_id.as_str(), segment.segment_index);
            if previous.is_some_and(|prior| prior >= order) {
                return Err(recovery("duplicate or unordered policy review inventory"));
            }
            previous = Some(order);
            budget.admit_file(segment.length)?;
        }
        Ok(())
    }

    fn matches_anchor(&self, brain: &str, anchor: &LocalTrustAnchor) -> bool {
        self.brain_id == brain
            && self.local_device_id == anchor.device_id().as_str()
            && self.local_signing_key_id == anchor.signing_key_id().as_str()
            && &self.local_public_key == anchor.public_key()
    }
}

impl ImportStagingSession {
    pub(crate) fn begin_policy_review(
        &mut self,
        brain: &str,
        anchor: &LocalTrustAnchor,
    ) -> Result<(), ImportStagingError> {
        self.validate_runtime_identity()?;
        if self.policy_review || self.prepared || self.failed || self._single_flight.is_some() {
            return Err(recovery(
                "policy review requires a complete released staging batch",
            ));
        }
        let mut segments = self
            .candidates
            .iter()
            .map(|c| ReviewSegment {
                device_id: c.device_id.as_str().to_owned(),
                segment_index: c.segment_index,
                length: c.length,
                sha256: c.sha256.clone(),
            })
            .collect::<Vec<_>>();
        segments.sort_by(|a, b| {
            a.device_id
                .cmp(&b.device_id)
                .then_with(|| a.segment_index.cmp(&b.segment_index))
        });
        let intent = ReviewIntent {
            version: 1,
            purpose: PURPOSE.into(),
            transaction_id: self.transaction_id.clone(),
            brain_id: brain.into(),
            local_device_id: anchor.device_id().as_str().into(),
            local_signing_key_id: anchor.signing_key_id().as_str().into(),
            local_public_key: *anchor.public_key(),
            segments,
        };
        intent.validate()?;
        let bytes = canonical_bytes(&intent)?;
        let _slot = review_slot(&self.runtime_root)?;
        check_review_namespace(&self.runtime_root, &self.transaction_id)?;
        // Set before the first write: uncertain persistence cannot clean-abort.
        self.policy_review = true;
        persist_new(&self.staging_dir.join(INTENT_NAME), &bytes)?;
        persist_new(
            &journal_path(&self.runtime_root, &self.transaction_id, INTENT_NAME),
            &bytes,
        )?;
        #[cfg(test)]
        crate::test_interrupt::maybe_abrupt_exit(
            "m8_policy_intent_synced",
            &self.runtime_root,
            "complete policy inventory durable before authentication/classification",
        );
        self.validate_runtime_identity()
    }

    pub(crate) fn has_policy_review(&self) -> bool {
        self.policy_review
    }

    pub(crate) fn finish_policy_review(&mut self) -> Result<(), ImportStagingError> {
        self.validate_runtime_identity()?;
        if !self.policy_review {
            return Ok(());
        }
        let _slot = review_slot(&self.runtime_root)?;
        check_review_namespace(&self.runtime_root, &self.transaction_id)?;
        let intent = read_intent_pair(&self.runtime_root, &self.transaction_id)?;
        if entry_exists(&self.staging_dir.join(LATCH_NAME))?
            || entry_exists(&journal_path(
                &self.runtime_root,
                &self.transaction_id,
                LATCH_NAME,
            ))?
        {
            return Err(recovery(
                "held policy evidence requires reviewed resolution",
            ));
        }
        verify_review_files(&intent, &self.staging_dir, false)?;
        let journal = journal_path(&self.runtime_root, &self.transaction_id, INTENT_NAME);
        fs::remove_file(&journal)?;
        sync_staged_parent(&journal)?;
        let sentinel = self.staging_dir.join(INTENT_NAME);
        fs::remove_file(&sentinel)?;
        sync_staged_parent(&sentinel)?;
        self.policy_review = false;
        self.validate_runtime_identity()
    }

    pub(crate) fn retain_policy_hold(
        &self,
        reason: AuthenticatedPolicyConflict,
    ) -> Result<(), ImportStagingError> {
        self.validate_runtime_identity()?;
        let _slot = review_slot(&self.runtime_root)?;
        check_review_namespace(&self.runtime_root, &self.transaction_id)?;
        let intent = read_intent_pair(&self.runtime_root, &self.transaction_id)?;
        verify_review_files(&intent, &self.staging_dir, false)?;
        let decision = HeldDecision {
            version: 1,
            purpose: "cryptex-incoming-policy-held".into(),
            intent_sha256: format!("{:x}", Sha256::digest(canonical_bytes(&intent)?)),
            reason,
        };
        let bytes = canonical_bytes(&decision)?;
        // Monotone held-seen latch first. A missing final decision after this
        // point is recovery uncertainty, never an unclassified review intent.
        persist_new(&self.staging_dir.join(LATCH_NAME), &bytes)?;
        #[cfg(test)]
        crate::test_interrupt::maybe_abrupt_exit(
            "m8_policy_held_latch_synced",
            &self.runtime_root,
            "held latch durable before final held decision",
        );
        persist_new(
            &journal_path(&self.runtime_root, &self.transaction_id, LATCH_NAME),
            &bytes,
        )?;
        #[cfg(test)]
        crate::test_interrupt::maybe_abrupt_exit(
            "m8_policy_held_decision_synced",
            &self.runtime_root,
            "held decision durable; no canonical publication",
        );
        self.validate_runtime_identity()
    }
}

fn read_intent_pair(root: &Path, id: &str) -> Result<ReviewIntent, ImportStagingError> {
    let journal = journal_path(root, id, INTENT_NAME);
    let intent: ReviewIntent = read_closed(&journal)?;
    intent.validate()?;
    if intent.transaction_id != id {
        return Err(recovery("policy review filename binding mismatch"));
    }
    let sentinel: ReviewIntent = read_closed(
        &root
            .join("canonical/import-staging")
            .join(id)
            .join(INTENT_NAME),
    )?;
    if sentinel != intent {
        return Err(recovery("policy review inventory copies disagree"));
    }
    Ok(intent)
}

fn verify_review_files(
    intent: &ReviewIntent,
    staging: &Path,
    latched: bool,
) -> Result<(ImportBatchBudget, Vec<StagedSegment>), ImportStagingError> {
    let mut expected = std::collections::BTreeSet::from([staging.join(INTENT_NAME)]);
    if latched {
        expected.insert(staging.join(LATCH_NAME));
    }
    let mut candidates = Vec::new();
    let mut budget = ImportBatchBudget::empty();
    for segment in &intent.segments {
        let device = DeviceId::new(segment.device_id.clone())
            .map_err(|_| recovery("invalid review device"))?;
        let relative = format!(
            "{}/{:020}.seg",
            device.as_filename_stem(),
            segment.segment_index
        );
        let path = staging.join(&relative);
        let descriptor = PreparedImportSegment {
            device_id: segment.device_id.clone(),
            segment_index: segment.segment_index,
            staged_relative_path: relative.clone(),
            destination_relative_path: relative,
            length: segment.length,
            sha256: segment.sha256.clone(),
        };
        // This is only the shared exact-file validator, not a prepared manifest.
        manifest_file_matches(&path, &descriptor, false)?;
        expected.insert(path.clone());
        budget.admit_file(segment.length)?;
        candidates.push(StagedSegment {
            device_id: device,
            segment_index: segment.segment_index,
            staged_path: path,
            length: segment.length,
            sha256: segment.sha256.clone(),
        });
    }
    let mut actual = std::collections::BTreeSet::new();
    for entry in fs::read_dir(staging)?.take(crate::import_limits::MAX_IMPORT_FILES_PER_BATCH + 3) {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if is_link_or_reparse(&metadata) {
            return Err(recovery("linked policy evidence"));
        }
        if metadata.is_dir() {
            ensure_existing_plain_directory(&path)?;
            let mut count = 0;
            for file in
                fs::read_dir(&path)?.take(crate::import_limits::MAX_IMPORT_FILES_PER_BATCH + 1)
            {
                actual.insert(file?.path());
                count += 1;
            }
            if count == 0 {
                return Err(recovery("empty policy evidence directory"));
            }
        } else {
            actual.insert(path);
        }
    }
    if actual != expected {
        return Err(recovery(
            "policy review must retain its exact complete inventory",
        ));
    }
    Ok((budget, candidates))
}

fn review_slot(root: &Path) -> Result<File, ImportStagingError> {
    cleanup::lock_slot(root)
}

fn check_review_namespace(root: &Path, id: &str) -> Result<(), ImportStagingError> {
    let journal = root.join("canonical/import-journal");
    let allowed = [
        journal.join("import.lock"),
        journal_path(root, id, INTENT_NAME),
        journal_path(root, id, LATCH_NAME),
    ];
    for entry in fs::read_dir(journal)? {
        if !allowed.contains(&entry?.path()) {
            return Err(recovery("unexpected policy review journal state"));
        }
    }
    Ok(())
}

/// Called before legacy staging cleanup, and while instance/exclusive ownership
/// is held. Never publishes, prepares a manifest or rebuilds derived state.
pub(super) fn recover<S: SecureStore + ?Sized>(
    root: &Path,
    store: &S,
    env: SecureStoreEnvironment,
) -> Result<(), ImportStagingError> {
    let journal = root.join("canonical/import-journal");
    if !entry_exists(&journal)? {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    let runtime_identity = super::macos_fs::RuntimeIdentity::capture(root)?;
    let _pins = inspect_directory_chain(&journal, true)?;
    let mut intent_id = None;
    let mut names = Vec::new();
    for entry in fs::read_dir(&journal)?.take(8) {
        let name = entry?
            .file_name()
            .into_string()
            .map_err(|_| recovery("invalid policy evidence name"))?;
        names.push(name.clone());
        if let Some(id) = name.strip_suffix(".policy-review.json") {
            if intent_id.replace(id.to_owned()).is_some() {
                return Err(recovery("competing policy review intents"));
            }
        } else if name != "import.lock" && !name.ends_with(".policy-held.json") {
            // Other legacy states are handled conservatively by the existing
            // recovery scanner. No policy intent can coexist with them.
            if name.contains("policy-") {
                return Err(recovery("partial policy evidence requires recovery"));
            }
        }
    }
    let Some(id) = intent_id else {
        return Ok(());
    };
    if names.iter().any(|name| {
        name != "import.lock"
            && name != &format!("{id}.{INTENT_NAME}")
            && name != &format!("{id}.{LATCH_NAME}")
    }) {
        return Err(recovery(
            "policy review cannot coexist with another journal state",
        ));
    }
    let staging_root = root.join("canonical/import-staging");
    let transactions = fs::read_dir(&staging_root)?
        .take(2)
        .collect::<Result<Vec<_>, _>>()?;
    if transactions.len() != 1 || transactions[0].file_name().to_str() != Some(id.as_str()) {
        return Err(recovery(
            "policy review requires its single exact staging transaction",
        ));
    }
    let intent = read_intent_pair(root, &id)?;
    let staging = root.join("canonical/import-staging").join(&id);
    let staging_pins = inspect_directory_chain(&staging, true)?;
    let latch_path = staging.join(LATCH_NAME);
    let held_path = journal_path(root, &id, LATCH_NAME);
    let latched = entry_exists(&latch_path)?;
    let decided = entry_exists(&held_path)?;
    if latched != decided {
        return Err(recovery(
            "incomplete held decision; preserve policy evidence",
        ));
    }
    let decision = if latched {
        let latch: HeldDecision = read_closed(&latch_path)?;
        let held: HeldDecision = read_closed(&held_path)?;
        if latch != held
            || held.version != 1
            || held.purpose != "cryptex-incoming-policy-held"
            || held.intent_sha256 != format!("{:x}", Sha256::digest(canonical_bytes(&intent)?))
        {
            return Err(recovery(
                "held decision does not bind the complete policy intent",
            ));
        }
        Some(held)
    } else {
        None
    };
    let (budget, candidates) = verify_review_files(&intent, &staging, latched)?;
    let brain = crate::identity_runtime::load_brain_identity(root)
        .map_err(|_| recovery("policy review Brain unavailable"))?;
    let resolver = crate::local_device_identity::LocalDeviceKeyResolver::load(
        root,
        &brain.brain_id,
        store,
        env,
    )
    .map_err(|_| recovery("policy review local key unavailable"))?;
    let anchor = resolver
        .local_trust_anchor()
        .ok_or_else(|| recovery("policy review anchor unavailable"))?;
    if !intent.matches_anchor(&brain.brain_id, &anchor) {
        return Err(recovery("policy review belongs to another local identity"));
    }
    let mut session = ImportStagingSession {
        runtime_root: root.to_path_buf(),
        transaction_id: id,
        staging_dir: staging,
        _single_flight: None,
        _directory_handles: staging_pins,
        budget,
        candidates,
        failed: false,
        prepared: false,
        policy_review: true,
        #[cfg(target_os = "macos")]
        runtime_identity,
    };
    match crate::knowledge_materialization::revalidate_incoming_policy_evidence(
        root,
        &brain.brain_id,
        &anchor,
        &session,
    ) {
        Err(ImportStagingError::PolicyConflict(reason)) => {
            if decision.is_none() {
                session.retain_policy_hold(reason)?;
            }
            Err(ImportStagingError::PolicyConflict(reason))
        }
        result => {
            if decision.is_some() {
                return Err(recovery(
                    "retained held decision requires reviewed resolution",
                ));
            }
            match result {
                Ok(()) => {}
                Err(ImportStagingError::Invalid(_)) => {}
                Err(error) => return Err(error),
            }
            session.finish_policy_review()?;
            session.discard()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (PathBuf, ImportStagingSession, LocalTrustAnchor) {
        let root = crate::test_temp::exclusive_temp_dir("policy-review-evidence");
        let device = DeviceId::new("peer").unwrap();
        let relative = PathBuf::from(device.as_filename_stem()).join("00000000000000000000.seg");
        let path = root.join("transport/incoming").join(&relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"synthetic unverified candidate").unwrap();
        let mut session = ImportStagingSession::begin(&root).unwrap();
        session.stage_segment(&relative, &device, 0).unwrap();
        session.release_staging_slot().unwrap();
        let key = ed25519_dalek::SigningKey::from_bytes(&[0x72; 32]);
        let anchor = LocalTrustAnchor::new(
            DeviceId::new("local").unwrap(),
            cryptex_events::signing_key_id_from_verifying_key(&key.verifying_key()).unwrap(),
            key.verifying_key().to_bytes(),
        );
        (root, session, anchor)
    }

    const BRAIN: &str = "22222222-2222-4222-8222-222222222222";

    #[test]
    fn review_intent_is_closed_bounded_and_never_a_publication_manifest() {
        let (root, mut session, anchor) = fixture();
        session.begin_policy_review(BRAIN, &anchor).unwrap();
        let intent = read_intent_pair(&root, &session.transaction_id).unwrap();
        assert!(intent.matches_anchor(BRAIN, &anchor));
        for fault in [
            "version",
            "purpose",
            "duplicate",
            "hash",
            "length",
            "extra-field",
            "duplicate-key",
        ] {
            let mut value = serde_json::to_value(&intent).unwrap();
            match fault {
                "version" => value["version"] = 2.into(),
                "purpose" => value["purpose"] = "prepared-import".into(),
                "duplicate" => {
                    let segment = value["segments"][0].clone();
                    value["segments"].as_array_mut().unwrap().push(segment);
                }
                "hash" => value["segments"][0]["sha256"] = "00".into(),
                "length" => {
                    value["segments"][0]["length"] =
                        (crate::import_limits::MAX_IMPORT_FILE_BYTES + 1).into()
                }
                "extra-field" => value["publication_ready"] = true.into(),
                "duplicate-key" => {}
                _ => unreachable!(),
            }
            let parsed = if fault == "duplicate-key" {
                let bytes = canonical_bytes(&intent).unwrap();
                let text = String::from_utf8(bytes)
                    .unwrap()
                    .replacen("{", "{\"version\":1,", 1);
                serde_json::from_str::<ReviewIntent>(&text)
            } else {
                serde_json::from_value::<ReviewIntent>(value)
            };
            assert!(
                parsed.is_err() || parsed.unwrap().validate().is_err(),
                "{fault}"
            );
        }
        session.finish_policy_review().unwrap();
        session.discard().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generic_startup_preserves_review_markers_and_missing_journal_sentinel() {
        let (root, mut session, anchor) = fixture();
        session.begin_policy_review(BRAIN, &anchor).unwrap();
        let stage = session.staging_dir.clone();
        let marker = journal_path(&root, &session.transaction_id, INTENT_NAME);
        let before = fs::read(stage.join(INTENT_NAME)).unwrap();
        assert!(recover_import_staging(&root).is_err());
        fs::remove_file(marker).unwrap();
        assert!(recover_import_staging(&root).is_err());
        assert_eq!(fs::read(stage.join(INTENT_NAME)).unwrap(), before);
        assert!(session.discard().is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn uncertain_intent_write_preserves_stage_and_refuses_discard() {
        let (root, mut session, anchor) = fixture();
        let marker = journal_path(&root, &session.transaction_id, INTENT_NAME);
        fs::write(marker, b"preexisting state must not be overwritten").unwrap();
        assert!(session.begin_policy_review(BRAIN, &anchor).is_err());
        let stage = session.staging_dir.clone();
        assert!(session.discard().is_err());
        assert!(stage.join(INTENT_NAME).exists());
        assert!(recover_import_staging(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_evidence_and_busy_import_slot_cannot_close_a_review_intent() {
        let (root, mut session, anchor) = fixture();
        session.begin_policy_review(BRAIN, &anchor).unwrap();
        let marker = journal_path(&root, &session.transaction_id, INTENT_NAME);
        let slot = review_slot(&root).unwrap();
        assert!(session.finish_policy_review().is_err());
        assert!(marker.exists());
        drop(slot);
        fs::write(&session.candidates[0].staged_path, b"changed evidence").unwrap();
        assert!(session.finish_policy_review().is_err());
        assert!(marker.exists());
        assert!(session.discard().is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn held_seen_latch_makes_missing_final_decision_sticky_before_revalidation() {
        let (root, mut session, anchor) = fixture();
        session.begin_policy_review(BRAIN, &anchor).unwrap();
        session
            .retain_policy_hold(AuthenticatedPolicyConflict::AdmissionDependency)
            .unwrap();
        let decision = journal_path(&root, &session.transaction_id, LATCH_NAME);
        fs::remove_file(decision).unwrap();
        let error = recover(
            &root,
            &crate::FakeSecureStore::new(),
            SecureStoreEnvironment::Dev,
        )
        .unwrap_err();
        assert!(error.to_string().contains("incomplete held decision"));
        assert!(session.finish_policy_review().is_err());
        assert!(session.discard().is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
