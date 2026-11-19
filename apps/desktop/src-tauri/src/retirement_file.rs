use cryptexd::retirement_notice::{MAX_NOTICE_BYTES, Notice};
use std::io::{Read, Write};

pub(super) fn choose(
    import: bool,
    exchange: impl FnOnce(Option<Notice>) -> Result<Option<Notice>, String>,
) -> Result<bool, String> {
    let dialog = rfd::FileDialog::new().add_filter("Cryptex retirement notice", &["cxret"]);
    if import {
        let Some(path) = dialog.pick_file() else {
            return Ok(false);
        };
        let meta = std::fs::symlink_metadata(&path).map_err(|_| "notice_file")?;
        if !meta.is_file() || meta.len() > MAX_NOTICE_BYTES as u64 {
            return Err("notice_file".into());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| "notice_file")?
            .take(MAX_NOTICE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "notice_file")?;
        exchange(Some(Notice::decode(&bytes)?))?;
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
