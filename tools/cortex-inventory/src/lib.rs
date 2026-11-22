//! Inventory a legacy source or preview the mapping of one explicitly selected note.
mod note_mapping;
use cap_fs_ext::DirExt;
use cap_std::fs::{Dir, Metadata};
pub use note_mapping::{ReviewIssue, preview, reviewed_payload};
use serde::Serialize;
use std::{io, path::Path, process::ExitCode};

const MAX_DEPTH: usize = 64;
const MAX_ENTRIES: usize = 100_000;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Issue {
    AccessDenied,
    Disappeared,
    ReadFailed,
    LinkOrReparsePoint,
    SpecialFile,
    NonUtf8Name,
    DepthLimit,
    EntryLimit,
    NotDirectory,
}

impl From<io::Error> for Issue {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::PermissionDenied => Self::AccessDenied,
            io::ErrorKind::NotFound => Self::Disappeared,
            io::ErrorKind::NotADirectory => Self::NotDirectory,
            _ => Self::ReadFailed,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Item {
    File {
        bytes: u64,
        extension: Option<String>,
    },
    Directory,
}

#[derive(Serialize)]
struct Entry {
    path: String,
    #[serde(flatten)]
    item: Item,
}

#[derive(Serialize)]
struct Finding {
    path: String,
    issue: Issue,
}

#[derive(Default, Serialize)]
struct Inventory {
    entries: Vec<Entry>,
    findings: Vec<Finding>,
    #[serde(skip)]
    visited: usize,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    complete: bool,
    #[serde(flatten)]
    inventory: Inventory,
}

fn indirect(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    metadata.file_type().is_symlink()
}

impl Inventory {
    fn finding(&mut self, path: &str, issue: Issue) {
        self.findings.push(Finding {
            path: path.to_owned(),
            issue,
        });
    }

    fn visit(&mut self, dir: &Dir, prefix: &str, depth: usize) -> Result<(), Issue> {
        let children = match dir.entries() {
            Ok(children) => children,
            Err(error) => {
                self.finding(prefix, error.into());
                return Ok(());
            }
        };
        for child in children {
            if self.visited >= MAX_ENTRIES {
                return Err(Issue::EntryLimit);
            }
            self.visited += 1;
            let child = match child {
                Ok(child) => child,
                Err(error) => {
                    self.finding(prefix, error.into());
                    continue;
                }
            };
            let Ok(name) = child.file_name().into_string() else {
                self.finding(prefix, Issue::NonUtf8Name);
                continue;
            };
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let metadata = match dir.symlink_metadata(&name) {
                Ok(metadata) => metadata,
                Err(error) => {
                    self.finding(&path, error.into());
                    continue;
                }
            };
            if indirect(&metadata) {
                self.finding(&path, Issue::LinkOrReparsePoint);
            } else if metadata.is_dir() {
                self.entries.push(Entry {
                    path: path.clone(),
                    item: Item::Directory,
                });
                if depth >= MAX_DEPTH {
                    self.finding(&path, Issue::DepthLimit);
                } else {
                    match dir.open_dir_nofollow(&name) {
                        Ok(nested) => self.visit(&nested, &path, depth + 1)?,
                        Err(error) => self.finding(&path, error.into()),
                    }
                }
            } else if metadata.is_file() {
                let extension = Path::new(&name)
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(str::to_ascii_lowercase);
                self.entries.push(Entry {
                    path,
                    item: Item::File {
                        bytes: metadata.len(),
                        extension,
                    },
                });
            } else {
                self.finding(&path, Issue::SpecialFile);
            }
        }
        Ok(())
    }
}

fn source_directory(source: &Path) -> Result<Dir, Issue> {
    let path = std::path::absolute(source)?;
    let dir = if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
        let parent = Dir::open_ambient_dir(parent, cap_std::ambient_authority())?;
        let metadata = parent.symlink_metadata(name)?;
        if indirect(&metadata) {
            return Err(Issue::LinkOrReparsePoint);
        }
        if !metadata.is_dir() {
            return Err(Issue::NotDirectory);
        }
        parent.open_dir_nofollow(name)?
    } else {
        Dir::open_ambient_dir(path, cap_std::ambient_authority())?
    };
    Ok(dir)
}

fn inventory(source: &Path) -> Result<Inventory, Issue> {
    let dir = source_directory(source)?;
    let mut report = Inventory::default();
    report.visit(&dir, "", 0)?;
    report.entries.sort_by(|a, b| a.path.cmp(&b.path));
    report
        .findings
        .sort_by(|a, b| (&a.path, a.issue).cmp(&(&b.path, b.issue)));
    Ok(report)
}

pub fn list(source: &Path) -> ExitCode {
    let inventory = match inventory(source) {
        Ok(report) => report,
        Err(issue) => {
            let mut failure = Inventory::default();
            failure.finding("", issue);
            failure
        }
    };
    let complete = inventory.findings.is_empty();
    let output = Report {
        schema: "cryptex.cortex.inventory.v1",
        complete,
        inventory,
    };
    write_report(&output, complete)
}

fn write_report(output: &impl Serialize, complete: bool) -> ExitCode {
    if serde_json::to_writer(io::stdout().lock(), output).is_err() {
        eprintln!("Could not write inventory report");
        return ExitCode::FAILURE;
    }
    if complete {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
