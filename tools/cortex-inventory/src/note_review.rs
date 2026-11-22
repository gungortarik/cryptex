//! Read-only comparison of the mapped Notes directory in a stable export snapshot.
use super::note_mapping::{digest, notes_directory, read_listed_note};
use super::{Issue, MAX_ENTRIES, ReviewIssue, write_report};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, process::ExitCode};

#[derive(Serialize)]
struct Finding {
    source_path: String,
    issue: ReviewIssue,
}

#[derive(Serialize)]
pub struct NoteReview {
    schema: &'static str,
    pub notes: BTreeMap<String, String>,
    exact_duplicates: Vec<Vec<String>>,
    possible_conflicts: Vec<Vec<String>>,
    findings: Vec<Finding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_sha256: Option<String>,
}

impl NoteReview {
    pub fn requires_choice(&self, path: &str) -> bool {
        self.exact_duplicates
            .iter()
            .chain(&self.possible_conflicts)
            .flatten()
            .any(|item| item == path)
    }
}

// A leading Markdown title is a comparison hint from untrusted source text.
// Keep only its digest while grouping to avoid putting title text in reports.
fn title_key(text: &str) -> Option<[u8; 32]> {
    let mut lines = text.lines().skip_while(|line| line.trim().is_empty());
    let mut first = lines.next()?;
    if first.trim() == "---" {
        lines.find(|line| matches!(line.trim(), "---" | "..."))?;
        first = lines.find(|line| !line.trim().is_empty())?;
    }
    let title = first.strip_prefix('#')?;
    if !title.starts_with([' ', '\t']) {
        return None;
    }
    let words: Vec<_> = title.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    Some(Sha256::digest(words.join(" ").to_lowercase().as_bytes()).into())
}

// Enumerate once with the actual case and spelling provided by the directory.
fn scan(source: &Path, review: &mut NoteReview) -> Result<(), ReviewIssue> {
    let folder = notes_directory(source)?;
    let mut names = Vec::new();
    for (index, entry) in folder.entries()?.enumerate() {
        if index >= MAX_ENTRIES {
            return Err(ReviewIssue::Source(Issue::EntryLimit));
        }
        let name = entry?.file_name();
        if Path::new(&name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        {
            names.push(
                name.into_string()
                    .map_err(|_| ReviewIssue::Source(Issue::NonUtf8Name))?,
            );
        }
    }
    names.sort();
    let mut by_digest = BTreeMap::<String, Vec<String>>::new();
    let mut by_title = BTreeMap::<[u8; 32], Vec<String>>::new();
    for name in names {
        let path = format!("Notes/{name}");
        let note = folder
            .symlink_metadata(&name)
            .map_err(ReviewIssue::from)
            .and_then(|metadata| read_listed_note(&folder, &name, &metadata));
        match note {
            Ok(note) => {
                let hash = digest(&note);
                by_digest
                    .entry(hash.clone())
                    .or_default()
                    .push(path.clone());
                if let Some(title) = title_key(&note.text) {
                    by_title.entry(title).or_default().push(path.clone());
                }
                review.notes.insert(path, hash);
            }
            Err(issue) => review.findings.push(Finding {
                source_path: path,
                issue,
            }),
        }
    }
    review.exact_duplicates = by_digest
        .into_values()
        .filter(|paths| paths.len() > 1)
        .collect();
    review.possible_conflicts = by_title
        .into_values()
        .filter(|paths| {
            paths
                .iter()
                .skip(1)
                .any(|path| review.notes[path] != review.notes[&paths[0]])
        })
        .collect();
    review.exact_duplicates.sort();
    review.possible_conflicts.sort();
    Ok(())
}

pub fn review_notes(source: &Path) -> NoteReview {
    let mut review = NoteReview {
        schema: "cryptex.cortex.note-review.v1",
        notes: BTreeMap::new(),
        exact_duplicates: Vec::new(),
        possible_conflicts: Vec::new(),
        findings: Vec::new(),
        review_sha256: None,
    };
    if let Err(issue) = scan(source, &mut review) {
        review.findings.push(Finding {
            source_path: "Notes".into(),
            issue,
        });
    }
    if review.findings.is_empty() {
        let mut hash = Sha256::new();
        serde_json::to_writer(&mut hash, &review).expect("in-memory review digest");
        review.review_sha256 = Some(format!("{:x}", hash.finalize()));
    }
    review
}

pub fn print_review(source: &Path) -> ExitCode {
    let review = review_notes(source);
    let clear = review.findings.is_empty()
        && review.exact_duplicates.is_empty()
        && review.possible_conflicts.is_empty();
    write_report(&review, clear)
}
