//! Authenticated release bytes and an owned content snapshot. Neither object
//! authorizes launching a path or dispatching an owner operation.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CString, c_int};
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::Instant;

use super::image_policy::PublisherDefinition;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const MAX_INVENTORY: usize = 1024 * 1024;
const MAX_FILES: usize = 4096;
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_BUNDLE_BYTES: u64 = 256 * 1024 * 1024;

unsafe extern "C" {
    fn cx_verify_inventory(
        cms: *const u8,
        cms_size: usize,
        content: *const u8,
        content_size: usize,
        cert: *const u8,
        cert_size: usize,
    ) -> c_int;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Signature,
    Schema,
    Release,
    Path,
    Limit,
    Changed,
    Io,
    Deadline,
}

// Provisioning these values is a separate reviewed release decision.
// No product factory, environment variable or wire field supplies this type.
struct ReleaseTrust {
    signer_der: &'static [u8],
    release_id: &'static str,
    architecture: &'static str,
    publisher: PublisherDefinition,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    schema: u32,
    purpose: String,
    release_id: String,
    architecture: String,
    desktop: Image,
    daemon: Image,
    files: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Image {
    path: String,
    cdhash: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
    size: u64,
    sha256: String,
}

pub(super) struct VerifiedInventory {
    inventory: Inventory,
    publisher: PublisherDefinition,
    desktop: [u8; 20],
    daemon: [u8; 20],
}

impl VerifiedInventory {
    fn verify(
        raw: &[u8],
        cms: &[u8],
        trust: ReleaseTrust,
        deadline: Instant,
    ) -> Result<Self, Error> {
        check_deadline(deadline)?;
        if raw.len() > MAX_INVENTORY
            || raw.is_empty()
            || cms.is_empty()
            || cms.len() > 2 * MAX_INVENTORY
        {
            return Err(Error::Limit);
        }
        // SAFETY: native code borrows bounded slices, evaluates the actual CMS
        // signature, exact pinned signer and offline CodeSigning trust policy.
        if unsafe {
            cx_verify_inventory(
                cms.as_ptr(),
                cms.len(),
                raw.as_ptr(),
                raw.len(),
                trust.signer_der.as_ptr(),
                trust.signer_der.len(),
            )
        } != 0
        {
            return Err(Error::Signature);
        }
        check_deadline(deadline)?;
        let text = std::str::from_utf8(raw).map_err(|_| Error::Schema)?;
        cryptex_events::reject_duplicate_json_keys(text).map_err(|_| Error::Schema)?;
        let inventory: Inventory = serde_json::from_slice(raw).map_err(|_| Error::Schema)?;
        if inventory.schema != 1 || inventory.purpose != "cryptex-m8-release-inventory" {
            return Err(Error::Schema);
        }
        if inventory.release_id != trust.release_id
            || inventory.architecture != trust.architecture
            || !matches!(inventory.architecture.as_str(), "arm64" | "x86_64")
        {
            return Err(Error::Release);
        }
        if inventory.files.is_empty() || inventory.files.len() > MAX_FILES {
            return Err(Error::Limit);
        }
        let mut names = BTreeSet::new();
        let mut total = 0u64;
        for entry in &inventory.files {
            validate_path(&entry.path)?;
            if !names.insert(entry.path.to_ascii_lowercase()) {
                return Err(Error::Path);
            }
            total = total.checked_add(entry.size).ok_or(Error::Limit)?;
            if entry.size > MAX_FILE_BYTES || total > MAX_BUNDLE_BYTES {
                return Err(Error::Limit);
            }
            parse_hex::<32>(&entry.sha256)?;
        }
        for image in [&inventory.desktop, &inventory.daemon] {
            validate_path(&image.path)?;
            if !inventory
                .files
                .iter()
                .any(|entry| entry.path == image.path && entry.size > 0)
            {
                return Err(Error::Schema);
            }
        }
        if inventory.desktop.path == inventory.daemon.path {
            return Err(Error::Schema);
        }
        let desktop = parse_hex::<20>(&inventory.desktop.cdhash)?;
        let daemon = parse_hex::<20>(&inventory.daemon.cdhash)?;
        if desktop == [0; 20] || daemon == [0; 20] || desktop == daemon {
            return Err(Error::Schema);
        }
        Ok(Self {
            inventory,
            publisher: trust.publisher,
            desktop,
            daemon,
        })
    }

    fn snapshot(self, root: &Path, deadline: Instant) -> Result<VerifiedBundle, Error> {
        self.snapshot_with(root, deadline, |_| {})
    }

    fn snapshot_with(
        self,
        root: &Path,
        deadline: Instant,
        mut after_read: impl FnMut(&str),
    ) -> Result<VerifiedBundle, Error> {
        check_deadline(deadline)?;
        let root = CString::new(root.as_os_str().as_encoded_bytes()).map_err(|_| Error::Path)?;
        // SAFETY: NUL-terminated path, no create operation, owned returned FD.
        let fd = unsafe {
            libc::open(
                root.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(Error::Io);
        }
        let root = unsafe { File::from_raw_fd(fd) };
        let mut files = BTreeMap::new();
        for entry in &self.inventory.files {
            check_deadline(deadline)?;
            let mut file = open_member(&root, &entry.path)?;
            let before = file.metadata().map_err(|_| Error::Io)?;
            if !before.is_file() || before.nlink() != 1 || before.len() != entry.size {
                return Err(Error::Changed);
            }
            let mut bytes = Vec::new();
            // Read at most the authenticated length plus one, including growth.
            (&mut file)
                .take(entry.size + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| Error::Io)?;
            after_read(&entry.path);
            check_deadline(deadline)?;
            let after = file.metadata().map_err(|_| Error::Io)?;
            if bytes.len() as u64 != entry.size
                || before.dev() != after.dev()
                || before.ino() != after.ino()
                || before.len() != after.len()
                || before.mtime() != after.mtime()
                || before.mtime_nsec() != after.mtime_nsec()
                || before.ctime() != after.ctime()
                || before.ctime_nsec() != after.ctime_nsec()
                || Sha256::digest(&bytes).as_slice() != parse_hex::<32>(&entry.sha256)?
            {
                return Err(Error::Changed);
            }
            files.insert(entry.path.clone(), bytes.into_boxed_slice());
        }
        check_deadline(deadline)?;
        Ok(VerifiedBundle {
            inventory: self,
            files,
        })
    }
}

pub(super) struct VerifiedBundle {
    inventory: VerifiedInventory,
    files: BTreeMap<String, Box<[u8]>>,
}

impl VerifiedBundle {
    fn content(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(AsRef::as_ref)
    }
    pub(super) fn image_hashes(&self) -> ([u8; 20], [u8; 20]) {
        (self.inventory.desktop, self.inventory.daemon)
    }
    pub(super) fn publisher(&self) -> PublisherDefinition {
        self.inventory.publisher
    }
}

fn validate_path(path: &str) -> Result<(), Error> {
    if path.len() > 1024
        || !path.starts_with("Contents/")
        || !path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
        })
    {
        return Err(Error::Path);
    }
    Ok(())
}

fn open_member(root: &File, path: &str) -> Result<File, Error> {
    validate_path(path)?;
    let mut directory = root.try_clone().map_err(|_| Error::Io)?;
    let mut parts = path.split('/').peekable();
    while let Some(part) = parts.next() {
        let name = CString::new(part).map_err(|_| Error::Path)?;
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | if parts.peek().is_some() {
                libc::O_DIRECTORY
            } else {
                0
            };
        // SAFETY: borrowed directory FD and NUL-terminated single component.
        let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(Error::Io);
        }
        directory = unsafe { File::from_raw_fd(fd) };
    }
    Ok(directory)
}

fn parse_hex<const N: usize>(value: &str) -> Result<[u8; N], Error> {
    if value.len() != N * 2
        || !value
            .bytes()
            .all(|c| c.is_ascii_digit() || matches!(c, b'a'..=b'f'))
    {
        return Err(Error::Schema);
    }
    let mut result = [0; N];
    for (i, out) in result.iter_mut().enumerate() {
        *out = u8::from_str_radix(&value[2 * i..2 * i + 2], 16).map_err(|_| Error::Schema)?;
    }
    Ok(result)
}

fn check_deadline(deadline: Instant) -> Result<(), Error> {
    if Instant::now() >= deadline {
        Err(Error::Deadline)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
