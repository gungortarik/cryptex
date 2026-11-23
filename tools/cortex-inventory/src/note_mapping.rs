//! Local review of a single plain Markdown note from the synthetic Cortex Notes category.
use super::{Issue, MAX_ENTRIES, indirect, source_directory, write_report};
use cap_fs_ext::{DirExt, FollowSymlinks, MetadataExt, OpenOptionsFollowExt};
use cap_std::fs::{Dir, Metadata, OpenOptions};
use cryptex_events::{EventType, MAX_MEMORY_NOTE_TEXT_BYTES, MemoryNotePayload};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path, process::ExitCode};

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewIssue {
    UnmappedCategory,
    Source(Issue),
    MultipleLinks,
    NotRegularFile,
    EmptyText,
    InvalidText,
    TooLarge,
    SourceChanged,
    DigestMismatch,
}

impl From<std::io::Error> for ReviewIssue {
    fn from(error: std::io::Error) -> Self {
        Self::Source(error.into())
    }
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Preview<'a> {
    Mapped {
        source_path: &'a str,
        source_category: &'static str,
        source_sha256: String,
        text_bytes: usize,
        event_type: EventType,
        classification_required: bool,
    },
    NeedsReview {
        issue: ReviewIssue,
    },
}

// Match the actual inventory spelling even on case-insensitive volumes.
fn exact_metadata(parent: &Dir, name: &str) -> Result<Metadata, ReviewIssue> {
    for (index, entry) in parent.entries()?.enumerate() {
        if index >= MAX_ENTRIES {
            return Err(ReviewIssue::Source(Issue::EntryLimit));
        }
        if entry?.file_name() == std::ffi::OsStr::new(name) {
            let meta = parent.symlink_metadata(name)?;
            if indirect(&meta) {
                return Err(ReviewIssue::Source(Issue::LinkOrReparsePoint));
            }
            return Ok(meta);
        }
    }
    Err(ReviewIssue::Source(Issue::Disappeared))
}

fn validate_name(name: &str) -> Result<(), ReviewIssue> {
    if name.contains(['/', '\\', ':'])
        || !Path::new(name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
    {
        return Err(ReviewIssue::UnmappedCategory);
    }
    Ok(())
}

pub(super) fn notes_directory(source: &Path) -> Result<Dir, ReviewIssue> {
    let root = source_directory(source).map_err(ReviewIssue::Source)?;
    if !exact_metadata(&root, "Notes")?.is_dir() {
        return Err(ReviewIssue::Source(Issue::NotDirectory));
    }
    Ok(root.open_dir_nofollow("Notes")?)
}

pub fn mapped_note_name(path: &str) -> Result<&str, ReviewIssue> {
    let name = path
        .strip_prefix("Notes/")
        .ok_or(ReviewIssue::UnmappedCategory)?;
    validate_name(name)?;
    Ok(name)
}

fn read_note(source: &Path, path: &str) -> Result<MemoryNotePayload, ReviewIssue> {
    let name = mapped_note_name(path)?;
    let folder = notes_directory(source)?;
    let metadata = exact_metadata(&folder, name)?;
    read_listed_note(&folder, name, &metadata)
}

pub(super) fn read_listed_note(
    folder: &Dir,
    name: &str,
    metadata: &Metadata,
) -> Result<MemoryNotePayload, ReviewIssue> {
    validate_name(name)?;
    if indirect(metadata) {
        return Err(ReviewIssue::Source(Issue::LinkOrReparsePoint));
    }
    if !metadata.is_file() {
        return Err(ReviewIssue::NotRegularFile);
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let mut file = folder.open_with(name, &options)?;
    let before = file.metadata()?;
    if indirect(&before) || !before.is_file() {
        return Err(ReviewIssue::NotRegularFile);
    }
    if before.nlink() != 1 {
        return Err(ReviewIssue::MultipleLinks);
    }
    if before.len() > MAX_MEMORY_NOTE_TEXT_BYTES as u64 {
        return Err(ReviewIssue::TooLarge);
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_MEMORY_NOTE_TEXT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_MEMORY_NOTE_TEXT_BYTES {
        return Err(ReviewIssue::TooLarge);
    }
    let after = file.metadata()?;
    if before.len() != bytes.len() as u64
        || after.len() != before.len()
        || before.modified()? != after.modified()?
        || after.nlink() != 1
    {
        return Err(ReviewIssue::SourceChanged);
    }
    let text = String::from_utf8(bytes).map_err(|_| ReviewIssue::InvalidText)?;
    if text.contains('\0') {
        return Err(ReviewIssue::InvalidText);
    }
    if text.trim().is_empty() {
        return Err(ReviewIssue::EmptyText);
    }
    MemoryNotePayload::new(text).map_err(|_| ReviewIssue::TooLarge)
}

pub(super) fn digest(payload: &MemoryNotePayload) -> String {
    format!("{:x}", Sha256::digest(payload.text.as_bytes()))
}

/// Return exactly the bytes checked against the owner's preview.
pub fn reviewed_payload(
    source: &Path,
    path: &str,
    approved_sha256: &str,
) -> Result<MemoryNotePayload, ReviewIssue> {
    let payload = read_note(source, path)?;
    if digest(&payload) != approved_sha256 {
        return Err(ReviewIssue::DigestMismatch);
    }
    Ok(payload)
}

fn map<'a>(source: &Path, path: &'a str) -> Result<Preview<'a>, ReviewIssue> {
    let payload = read_note(source, path)?;
    Ok(Preview::Mapped {
        source_category: "markdown_note",
        source_path: path,
        event_type: EventType::MemoryNote,
        source_sha256: digest(&payload),
        text_bytes: payload.text.len(),
        classification_required: true,
    })
}

pub fn preview(source: &Path, path: Option<&str>) -> ExitCode {
    let result = path
        .ok_or(ReviewIssue::Source(Issue::NonUtf8Name))
        .and_then(|path| map(source, path));
    let mapped = result.is_ok();
    let output = result.unwrap_or_else(|issue| Preview::NeedsReview { issue });
    write_report(&output, mapped)
}
