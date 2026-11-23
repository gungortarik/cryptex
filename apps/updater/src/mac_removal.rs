use super::{Layout, desktop, error, release, vacant};
use std::{
    ffi::{CString, c_char},
    fs,
    io::{self, Read},
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::Path,
};

unsafe extern "C" {
    fn cx_login_entry_matches(bytes: *const u8, len: usize, desktop: *const c_char) -> i32;
    fn cx_move_to_trash(path: *const c_char) -> i32;
}

fn trash(path: &Path) -> io::Result<()> {
    let path = CString::new(path.as_os_str().as_bytes())?;
    let code = unsafe { cx_move_to_trash(path.as_ptr()) };
    if code != 0 {
        return Err(error(&format!("macOS Trash failed ({code})")));
    }
    Ok(())
}

pub(super) fn remove(layout: &Layout, approved: &str) -> io::Result<()> {
    let _transaction = layout.begin_update()?;
    if !vacant(&layout.intent())? || !vacant(&layout.previous)? {
        return Err(error("finish the pending update recovery before removal"));
    }
    let applications = layout
        .active
        .parent()
        .ok_or_else(|| error("missing Applications directory"))?;
    let home = applications
        .parent()
        .ok_or_else(|| error("missing user home"))?;
    let library = home.join("Library");
    let agents = library.join("LaunchAgents");
    for directory in [applications, library.as_path(), agents.as_path()] {
        if !vacant(directory)? && !fs::symlink_metadata(directory)?.is_dir() {
            return Err(error("indirect application or startup directory"));
        }
    }
    release::verify(layout, &layout.active, approved)?;
    let login = agents.join("Cryptex.plist");
    if !vacant(&login)? {
        let info = fs::symlink_metadata(&login)?;
        if !info.is_file() || info.len() > 65536 {
            return Err(error("unexpected Cryptex login entry"));
        }
        let file = fs::File::open(&login)?;
        let mut bytes = Vec::new();
        (&file).take(65537).read_to_end(&mut bytes)?;
        let image = CString::new(desktop(&layout.active).as_os_str().as_bytes())?;
        if bytes.len() > 65536
            || unsafe { cx_login_entry_matches(bytes.as_ptr(), bytes.len(), image.as_ptr()) } != 1
        {
            return Err(error("login entry does not belong to this installation"));
        }
        let current = fs::symlink_metadata(&login)?;
        let opened = file.metadata()?;
        if (current.dev(), current.ino()) != (opened.dev(), opened.ino()) {
            return Err(error("login entry changed during review"));
        }
        trash(&login)?;
    }
    trash(&layout.active)
}
