//! Copy the stopped engine's native data using pinned directories. Existing
//! staged files are verify-only. The original stays usable for cold rollback.
use super::{Directory, Error, source_snapshot};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read, Seek};
use std::path::Path;

const JOURNAL_FILES: &[&str] = &[
    "session.lock",
    "config.xml",
    "exchange-binding.json",
    "exchange-install.json",
    "exchange-config.tmp",
    "exchange-binding.tmp",
    "exchange-install.tmp",
];

fn component(name: &str) -> Result<(), Error> {
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if name.is_empty()
        || name.ends_with('.')
        || name.len() > 255
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        || matches!(stem.as_str(), "CON" | "PRN" | "NUL" | "AUX")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit())
    {
        return Err(Error::Unsafe);
    }
    Ok(())
}

fn entries(dir: &Directory, root: bool) -> Result<BTreeMap<String, bool>, Error> {
    dir.validate()?;
    let mut result = BTreeMap::new();
    let mut spellings = std::collections::BTreeSet::new();
    for entry in fs::read_dir(&dir.path)? {
        let entry = entry?;
        let name = entry.file_name().into_string().map_err(|_| Error::Unsafe)?;
        component(&name)?;
        let ty = entry.file_type()?;
        if !(ty.is_file() || ty.is_dir()) || !spellings.insert(name.to_ascii_lowercase()) {
            return Err(Error::Unsafe);
        }
        if root
            && let Some(fixed) = JOURNAL_FILES
                .iter()
                .find(|fixed| name.eq_ignore_ascii_case(fixed))
        {
            if name != *fixed || !ty.is_file() {
                return Err(Error::Unsafe);
            }
            continue;
        }
        result.insert(name, ty.is_dir());
    }
    dir.validate()?;
    Ok(result)
}

fn readonly(dir: &Directory, name: &str) -> Result<File, Error> {
    #[cfg(windows)]
    let file = super::open_source_from_directory_chain(&dir.pins, &dir.path.join(name))?;
    #[cfg(target_os = "macos")]
    let file = dir.native.open_file(name.as_ref(), libc::O_RDONLY)?;
    dir.checked_regular(file)
}

fn fingerprint(file: &mut File) -> Result<[u8; 32], Error> {
    let initial = source_snapshot(file)?;
    file.rewind()?;
    let mut hash = Sha256::new();
    if io::copy(&mut file.take(initial.length), &mut hash)? != initial.length
        || source_snapshot(file)? != initial
    {
        return Err(Error::Unsafe);
    }
    Ok(hash.finalize().into())
}

fn visit(
    source: &Directory,
    staged: &Directory,
    depth: usize,
    copy_missing: bool,
) -> Result<(), Error> {
    if depth > 64 {
        return Err(Error::Unsafe);
    }
    let original = entries(source, depth == 0)?;
    let existing = entries(staged, depth == 0)?;
    if existing
        .iter()
        .any(|(name, dir)| original.get(name) != Some(dir))
    {
        return Err(Error::Unsafe);
    }
    for (name, directory) in &original {
        let exists = existing.contains_key(name);
        if !exists && !copy_missing {
            return Err(Error::Unsafe);
        }
        if *directory {
            let src = source.child_validated(name, false)?;
            let dest = if exists {
                staged.child_validated(name, false)?
            } else {
                staged.create_child_exclusive_validated(name)?
            };
            // A pre-existing subdirectory must already be a complete copy.
            // A failed new generation can always be retried in a fresh private slot.
            visit(&src, &dest, depth + 1, !exists)?;
        } else {
            let mut src = readonly(source, name)?;
            if !exists {
                let before = source_snapshot(&src)?;
                let mut output = staged.open_file_validated(name, true)?;
                if io::copy(&mut (&mut src).take(before.length), &mut output)? != before.length
                    || source_snapshot(&src)? != before
                {
                    return Err(Error::Unsafe);
                }
                output.sync_all()?;
            }
            let mut output = readonly(staged, name)?;
            if source_snapshot(&src)?.identity == source_snapshot(&output)?.identity
                || fingerprint(&mut src)? != fingerprint(&mut output)?
            {
                return Err(Error::Unsafe);
            }
        }
    }
    if original != entries(source, depth == 0)? || original != entries(staged, depth == 0)? {
        return Err(Error::Unsafe);
    }
    staged.sync()
}

/// Both roots are independently pinned, locked private native homes. A journal
/// for the candidate is published only after all retained bytes match.
pub(crate) fn stage(source: &Path, staged: &Path) -> Result<(), Error> {
    let a = source.canonicalize()?;
    let b = staged.canonicalize()?;
    if a.starts_with(&b) || b.starts_with(&a) {
        return Err(Error::Unsafe);
    }
    let source = Directory::open(source)?;
    let staged = Directory::open(staged)?;
    visit(&source, &staged, 0, true)
}
