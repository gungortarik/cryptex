use cryptexd::retirement_notice::{MAX_NOTICE_BYTES, Notice};
use std::io::{Read, Write};
use std::path::Path;

fn read_notice(path: &Path) -> Result<Notice, &'static str> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|_| "notice_file")?;
    let meta = file.metadata().map_err(|_| "notice_file")?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err("notice_file");
        }
    }
    if !meta.is_file() || meta.len() > MAX_NOTICE_BYTES as u64 {
        return Err("notice_file");
    }
    let mut bytes = Vec::new();
    file.take(MAX_NOTICE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "notice_file")?;
    Notice::decode(&bytes)
}

pub(super) fn choose(
    import: bool,
    exchange: impl FnOnce(Option<Notice>) -> Result<Option<Notice>, String>,
) -> Result<bool, String> {
    let dialog = rfd::FileDialog::new().add_filter("Cryptex retirement notice", &["cxret"]);
    if import {
        let Some(path) = dialog.pick_file() else {
            return Ok(false);
        };
        exchange(Some(read_notice(&path)?))?;
    } else {
        let notice = exchange(None)?.ok_or("notice_missing")?;
        let Some(path) = dialog.set_file_name("retirement.cxret").save_file() else {
            return Ok(false);
        };
        if path.extension().and_then(|s| s.to_str()) != Some("cxret") {
            return Err("notice_extension".into());
        }
        let mut tmp = tempfile::NamedTempFile::new_in(path.parent().ok_or("notice_file")?)
            .map_err(|_| "notice_file")?;
        tmp.write_all(&serde_json::to_vec(&notice).map_err(|_| "notice_invalid")?)
            .map_err(|_| "notice_file")?;
        tmp.as_file().sync_all().map_err(|_| "notice_file")?;
        tmp.persist_noclobber(path)
            .map_err(|_| "notice_destination")?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_notice_must_be_a_bounded_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.cxret");
        std::fs::write(&path, b"{}").unwrap();
        assert_eq!(read_notice(&path).err(), Some("invalid_notice"));
        assert_eq!(read_notice(dir.path()).err(), Some("notice_file"));
        std::fs::write(&path, vec![0; MAX_NOTICE_BYTES + 1]).unwrap();
        assert_eq!(read_notice(&path).err(), Some("notice_file"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn selected_symbolic_link_is_refused_before_decode() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("original.cxret");
        std::fs::write(&target, b"{}").unwrap();
        let link = dir.path().join("selected.cxret");
        std::os::unix::fs::symlink(target, &link).unwrap();
        assert_eq!(read_notice(&link).err(), Some("notice_file"));
    }
}
