//! Path encoding for daemon-owned Win32 storage and metadata operations.
//!
//! This performs no filesystem lookup and is not an ownership/reparse validator.
//! Ordinary DOS/UNC paths retain their Win32 absolute-path semantics; existing
//! verbatim paths retain their literal semantics. Device namespaces are refused.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::{Component, Path, Prefix};

pub fn for_win32_api(path: &Path) -> io::Result<Vec<u16>> {
    if path.as_os_str().encode_wide().any(|unit| unit == 0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "metadata path contains NUL",
        ));
    }
    // Unlike canonicalize, absolute does not follow links or require the
    // destination to exist (exclusive metadata creation must still work).
    let absolute = std::path::absolute(path)?;
    let mut units: Vec<u16> = absolute.as_os_str().encode_wide().collect();
    let prefix = match absolute.components().next() {
        Some(Component::Prefix(prefix)) => prefix.kind(),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "metadata path has no drive or share",
            ));
        }
    };
    let mut wide = match prefix {
        Prefix::VerbatimDisk(_) | Prefix::VerbatimUNC(_, _) => units,
        Prefix::Disk(_) | Prefix::UNC(_, _) => {
            // Extended paths require backslashes even if the caller used '/'.
            units.iter_mut().for_each(|unit| {
                if *unit == u16::from(b'/') {
                    *unit = u16::from(b'\\');
                }
            });
            let (head, tail) = if matches!(prefix, Prefix::UNC(_, _)) {
                (r"\\?\UNC\", &units[2..])
            } else {
                (r"\\?\", &units[..])
            };
            head.encode_utf16().chain(tail.iter().copied()).collect()
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "metadata device namespace is unsupported",
            ));
        }
    };
    wide.push(0);
    Ok(wide)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(path: &str) -> String {
        let wide = for_win32_api(Path::new(path)).unwrap();
        assert_eq!(wide.last(), Some(&0));
        assert!(!wide[..wide.len() - 1].contains(&0));
        String::from_utf16(&wide[..wide.len() - 1]).unwrap()
    }

    #[test]
    fn encodes_drive_share_and_verbatim_without_filesystem_lookup() {
        assert_eq!(
            encoded("C:/not-present/çalışma/record.json"),
            r"\\?\C:\not-present\çalışma\record.json"
        );
        assert_eq!(
            encoded(r"\\server\share\missing\record.json"),
            r"\\?\UNC\server\share\missing\record.json"
        );
        assert_eq!(
            encoded(r"\\?\C:\missing\record.json"),
            r"\\?\C:\missing\record.json"
        );
        assert_eq!(
            encoded(r"\\?\UNC\server\share\record.json"),
            r"\\?\UNC\server\share\record.json"
        );
        let relative = Path::new("missing/record.json");
        assert_eq!(
            for_win32_api(relative).unwrap(),
            for_win32_api(&std::path::absolute(relative).unwrap()).unwrap()
        );
    }

    #[test]
    fn refuses_nul_and_device_namespaces() {
        for path in [
            "C:\\record\0.json",
            r"\\.\PhysicalDrive0",
            r"\\?\GLOBALROOT\Device\HarddiskVolume1",
        ] {
            assert_eq!(
                for_win32_api(Path::new(path)).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
    }
}
