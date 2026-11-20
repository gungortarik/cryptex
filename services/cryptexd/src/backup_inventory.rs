//! Signed contents of an M11 v2 backup. The caller supplies the independently
//! selected recovery checkpoint and a private, immutable staging file.
use crate::event_signing_key::load_existing_event_signing_key;
use crate::secure_store::{SecureStore, SecureStoreEnvironment};
use cryptex_core::BrainIdentity;
use cryptex_events::{LocalTrustAnchor, SignatureHex};
use ed25519_dalek::{Signature, Signer, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read, Seek};
use std::path::Path;

const DOMAIN: &[u8] = b"cryptex/backup/inventory/v2\0";
const IDENTITY: &str = "identity/brain_identity.json";
pub const MAX_INVENTORY_BYTES: usize = 4 * 1024 * 1024;

pub struct Checkpoint<'a> {
    pub brain_id: &'a str,
    pub writer: &'a LocalTrustAnchor,
    pub generation: u64,
    pub history_sha256: [u8; 32],
    pub brain_identity_sha256: [u8; 32],
}

/// Public selection saved independently of the encrypted backup. Its trust
/// comes from the owner's authenticated recovery material.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoverySelection {
    version: u8,
    brain_id: String,
    writer_device: cryptex_core::DeviceId,
    writer_key: [u8; 32],
    generation: u64,
    history_sha256: [u8; 32],
    brain_identity_sha256: [u8; 32],
}

impl RecoverySelection {
    pub fn capture(checkpoint: &Checkpoint<'_>) -> Self {
        Self {
            version: 1,
            brain_id: checkpoint.brain_id.into(),
            writer_device: checkpoint.writer.device_id().clone(),
            writer_key: *checkpoint.writer.public_key(),
            generation: checkpoint.generation,
            history_sha256: checkpoint.history_sha256,
            brain_identity_sha256: checkpoint.brain_identity_sha256,
        }
    }

    pub fn writer(&self) -> Result<LocalTrustAnchor, Error> {
        if self.version != 1 {
            return Err(Error::Checkpoint);
        }
        let key = VerifyingKey::from_bytes(&self.writer_key).map_err(|_| Error::Credentials)?;
        let id = cryptex_events::signing_key_id_from_verifying_key(&key)
            .map_err(|_| Error::Credentials)?;
        Ok(LocalTrustAnchor::new(
            self.writer_device.clone(),
            id,
            self.writer_key,
        ))
    }

    pub fn checkpoint<'a>(&'a self, writer: &'a LocalTrustAnchor) -> Result<Checkpoint<'a>, Error> {
        if self.writer()? != *writer || self.generation == 0 {
            return Err(Error::Checkpoint);
        }
        Ok(Checkpoint {
            brain_id: &self.brain_id,
            writer,
            generation: self.generation,
            history_sha256: self.history_sha256,
            brain_identity_sha256: self.brain_identity_sha256,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Credentials,
    Archive,
    Checkpoint,
    Signature,
    Inventory,
    Attachment,
    RecoveryHistory,
    Codec,
    CodecIntegrity,
    CodecStart,
    CodecExit,
    SourceIdentity,
    SourcePack,
}

/// Check a privately extracted profile before publishing it or marking a
/// generation verified. The caller must first authenticate the envelope.
#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
pub fn verify_reconstruction(root: &Path, expected: &Checkpoint<'_>) -> Result<(), Error> {
    crate::knowledge_materialization::recovered_search::verify_backup_reconstruction(root, expected)
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Member {
    bytes: u64,
    sha256: String,
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    format: String,
    brain_id: String,
    writer_device_id: String,
    writer_key_id: String,
    generation: u64,
    history_sha256: [u8; 32],
    archive_sha256: String,
    members: BTreeMap<String, Member>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    body: Body,
    signature: SignatureHex,
}

fn lower_hex(s: &str) -> bool {
    s.bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn canonical_member(name: &str) -> bool {
    if let Some(hash) = name.strip_prefix("canonical/blobs/") {
        return hash.len() == 64 && lower_hex(hash);
    }
    let Some((stream, file)) = name
        .strip_prefix("canonical/events/")
        .and_then(|s| s.split_once('/'))
    else {
        return name == IDENTITY;
    };
    let Some(sequence) = file
        .strip_suffix(".seg")
        .or_else(|| file.strip_suffix(".open"))
    else {
        return false;
    };
    (2..=256).contains(&stream.len())
        && stream.len().is_multiple_of(2)
        && lower_hex(stream)
        && sequence.len() == 20
        && sequence.bytes().all(|b| b.is_ascii_digit())
}

fn inspect(
    path: &Path,
    expected: &Checkpoint<'_>,
    signed_digest: Option<&str>,
) -> Result<Body, Error> {
    if expected.generation == 0 || expected.generation > (1 << 53) - 1 {
        return Err(Error::Checkpoint);
    }
    let mut input = File::open(path).map_err(|_| Error::Archive)?;
    let mut digest = Sha256::new();
    io::copy(&mut input, &mut digest).map_err(|_| Error::Archive)?;
    let archive_sha256 = format!("{:x}", digest.finalize());
    if signed_digest.is_some_and(|expected| expected != archive_sha256) {
        return Err(Error::Checkpoint);
    }
    input.rewind().map_err(|_| Error::Archive)?;
    let mut archive = zip::ZipArchive::new(input).map_err(|_| Error::Archive)?;
    if archive.len() > MAX_INVENTORY_BYTES / 64
        || archive
            .has_overlapping_files()
            .map_err(|_| Error::Archive)?
    {
        return Err(Error::Archive);
    }
    let mut members = BTreeMap::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|_| Error::Archive)?;
        let name = file.name().map_err(|_| Error::Archive)?.into_owned();
        if !canonical_member(&name)
            || name.as_bytes() != file.name_raw()
            || file.encrypted()
            || file.compression() != zip::CompressionMethod::Stored
            || !file.is_file()
            || file.is_symlink()
        {
            return Err(Error::Archive);
        }
        let mut hasher = Sha256::new();
        let bytes = if name == IDENTITY {
            let mut identity = Vec::new();
            (&mut file)
                .take(4097)
                .read_to_end(&mut identity)
                .map_err(|_| Error::Archive)?;
            if identity.len() > 4096
                || <[u8; 32]>::from(Sha256::digest(&identity)) != expected.brain_identity_sha256
                || BrainIdentity::from_json(
                    std::str::from_utf8(&identity).map_err(|_| Error::Archive)?,
                )
                .map_err(|_| Error::Archive)?
                .brain_id
                    != expected.brain_id
            {
                return Err(Error::Checkpoint);
            }
            hasher.update(&identity);
            identity.len() as u64
        } else {
            io::copy(&mut file, &mut hasher).map_err(|_| Error::Archive)?
        };
        let sha256 = format!("{:x}", hasher.finalize());
        if bytes != file.size()
            || name
                .strip_prefix("canonical/blobs/")
                .is_some_and(|hash| hash != sha256)
            || members.insert(name, Member { bytes, sha256 }).is_some()
        {
            return Err(Error::Archive);
        }
    }
    if !members.contains_key(IDENTITY) {
        return Err(Error::Checkpoint);
    }
    Ok(Body {
        format: "cryptex.backup.inventory.v2".into(),
        brain_id: expected.brain_id.into(),
        writer_device_id: expected.writer.device_id().as_str().into(),
        writer_key_id: expected.writer.signing_key_id().as_str().into(),
        generation: expected.generation,
        history_sha256: expected.history_sha256,
        archive_sha256,
        members,
    })
}

fn encode(value: &impl Serialize) -> Result<Vec<u8>, Error> {
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|_| Error::Inventory)?;
    if bytes.len() > MAX_INVENTORY_BYTES {
        return Err(Error::Inventory);
    }
    Ok(bytes)
}

fn preimage(body: &Body) -> Result<Vec<u8>, Error> {
    Ok([DOMAIN, encode(body)?.as_slice()].concat())
}

/// Sign with the existing enrolled credential. Never initializes a missing key.
pub fn sign(
    root: &Path,
    store: &(impl SecureStore + ?Sized),
    env: SecureStoreEnvironment,
    payload: &Path,
    expected: &Checkpoint<'_>,
) -> Result<Vec<u8>, Error> {
    let ready = load_existing_event_signing_key(store, env, root, expected.writer.device_id())
        .map_err(|_| Error::Credentials)?;
    if ready.signing_key.verifying_key().as_bytes() != expected.writer.public_key()
        || ready.public.signing_key_id != expected.writer.signing_key_id().as_str()
    {
        return Err(Error::Credentials);
    }
    let body = inspect(payload, expected, None)?;
    let signature = SignatureHex::from_bytes(&ready.signing_key.sign(&preimage(&body)?).to_bytes())
        .map_err(|_| Error::Signature)?;
    encode(&Inventory { body, signature })
}

fn authenticated_members(
    payload: &Path,
    signed: &[u8],
    expected: &Checkpoint<'_>,
) -> Result<BTreeMap<String, Member>, Error> {
    if signed.len() > MAX_INVENTORY_BYTES {
        return Err(Error::Inventory);
    }
    let inventory: Inventory = serde_json::from_slice(signed).map_err(|_| Error::Inventory)?;
    if encode(&inventory)? != signed {
        return Err(Error::Inventory);
    }
    let signature = Signature::from_bytes(
        &inventory
            .signature
            .to_bytes()
            .map_err(|_| Error::Signature)?,
    );
    VerifyingKey::from_bytes(expected.writer.public_key())
        .map_err(|_| Error::Signature)?
        .verify_strict(&preimage(&inventory.body)?, &signature)
        .map_err(|_| Error::Signature)?;
    if inventory.body != inspect(payload, expected, Some(&inventory.body.archive_sha256))? {
        return Err(Error::Checkpoint);
    }
    Ok(inventory.body.members)
}

pub fn verify(payload: &Path, signed: &[u8], expected: &Checkpoint<'_>) -> Result<(), Error> {
    authenticated_members(payload, signed, expected).map(drop)
}

/// Extract independently checked members into a fresh, owner-private sandbox.
/// A failed restore drops the scratch directory before returning to the caller.
#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
pub fn reconstruct_v2(
    payload: &Path,
    signed: &[u8],
    expected: &Checkpoint<'_>,
    parent: &Path,
) -> Result<tempfile::TempDir, Error> {
    let mut members = authenticated_members(payload, signed, expected)?;
    let input = File::open(payload).map_err(|_| Error::Archive)?;
    let mut zip = zip::ZipArchive::new(input).map_err(|_| Error::Archive)?;
    let sandbox = tempfile::tempdir_in(parent).map_err(|_| Error::Archive)?;
    for i in 0..zip.len() {
        let source = zip.by_index(i).map_err(|_| Error::Archive)?;
        let name = source.name().map_err(|_| Error::Archive)?.into_owned();
        let member = members.remove(&name).ok_or(Error::Inventory)?;
        if source.compression() != zip::CompressionMethod::Stored || source.size() != member.bytes {
            return Err(Error::Inventory);
        }
        let target = sandbox.path().join(name);
        std::fs::create_dir_all(target.parent().ok_or(Error::Archive)?)
            .map_err(|_| Error::Archive)?;
        let mut file = File::create_new(target).map_err(|_| Error::Archive)?;
        let count = io::copy(&mut source.take(member.bytes.saturating_add(1)), &mut file)
            .map_err(|_| Error::Archive)?;
        file.rewind().map_err(|_| Error::Archive)?;
        let mut digest = Sha256::new();
        io::copy(&mut file, &mut digest).map_err(|_| Error::Archive)?;
        if count != member.bytes || format!("{:x}", digest.finalize()) != member.sha256 {
            return Err(Error::Inventory);
        }
    }
    if !members.is_empty() {
        return Err(Error::Inventory);
    }
    verify_reconstruction(sandbox.path(), expected)?;
    Ok(sandbox)
}
