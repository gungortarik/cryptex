//! Exact upstream Mac 2.1.5 bytes; no signer/publisher or activation claim.
use super::*;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

#[cfg(target_arch = "aarch64")]
pub(crate) const CDHASH: [u8; 20] = [
    0x66, 0x07, 0x31, 0xa8, 0xe5, 0x4d, 0xee, 0x77, 0x59, 0x7c, 0x05, 0x26, 0x86, 0x5d, 0xc9, 0x38,
    0x52, 0x5f, 0xc2, 0x94,
];
#[cfg(target_arch = "x86_64")]
pub(crate) const CDHASH: [u8; 20] = [
    0x14, 0x11, 0x41, 0x3e, 0x32, 0xd8, 0x73, 0x0e, 0x78, 0xc9, 0x01, 0x56, 0xdb, 0x86, 0xcd, 0x66,
    0x1b, 0xae, 0x2f, 0x9a,
];
#[cfg(target_arch = "aarch64")]
const BINARY: (u64, &str) = (
    27_811_792,
    "6de41132c8a3fc78fc57486df32f9ac69d6f53db1e1910bb46f548ee7f29725e",
);
#[cfg(target_arch = "x86_64")]
const BINARY: (u64, &str) = (
    29_591_776,
    "277466c540abc5898c03b3e5e6d6349001483ba40b3ffa5e3782f5ba23f12f7e",
);

pub(super) fn open_pinned(directory: &Path) -> Result<Vec<File>, PackageError> {
    if !directory.is_absolute()
        || directory.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
        || crate::paths::assert_outside_repository(directory).is_err()
    {
        return Err(PackageError::InvalidPath);
    }
    let metadata = std::fs::symlink_metadata(directory).map_err(|_| PackageError::Io)?;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(PackageError::UnsafeObject);
    }
    let dir = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(directory)
        .map_err(|_| PackageError::Io)?;
    if !crate::owned_process::macos::private_acl(&dir) {
        return Err(PackageError::UnsafeObject);
    }
    let mut files = vec![dir];
    for (name, size, expected) in [
        ("syncthing", BINARY.0, BINARY.1),
        (
            "LICENSE.txt",
            17_099,
            "9221c2f936159b8446d329249fb4c0f25be510f447383a0f13336ac7985668a3",
        ),
        (
            "AUTHORS.txt",
            17_156,
            "2c197afd6113ec13ae134a20ce8f101d9812b078c3ef32c42c3bb4cdaf2b96a2",
        ),
    ] {
        let path = directory.join(name);
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(&path)
            .map_err(|_| PackageError::Io)?;
        let before = file.metadata().map_err(|_| PackageError::Io)?;
        if !before.is_file()
            || before.nlink() != 1
            || before.uid() != unsafe { libc::geteuid() }
            || before.mode() & 0o022 != 0
            || !crate::owned_process::macos::private_acl(&file)
        {
            return Err(PackageError::UnsafeObject);
        }
        if before.len() != size {
            return Err(PackageError::SizeMismatch);
        }
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut count = 0u64;
        loop {
            let n = file.read(&mut buffer).map_err(|_| PackageError::Io)?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > size {
                return Err(PackageError::SizeMismatch);
            }
            hash.update(&buffer[..n]);
        }
        let digest = hash
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let after = file.metadata().map_err(|_| PackageError::Io)?;
        let current = std::fs::symlink_metadata(path).map_err(|_| PackageError::Io)?;
        if count != size || digest != expected {
            return Err(PackageError::DigestMismatch);
        }
        if before.dev() != current.dev()
            || before.ino() != current.ino()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return Err(PackageError::UnsafeObject);
        }
        files.push(file);
    }
    Ok(files)
}
