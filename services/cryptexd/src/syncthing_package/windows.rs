use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::mem::zeroed;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Component, Path, Prefix};

use sha2::{Digest, Sha256};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
    FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle,
};

use super::{AUTHORS_SHA256, LICENSE_SHA256, PackageError, WINDOWS_AMD64_SHA256};

pub(super) fn open_pinned(directory: &Path) -> Result<Vec<File>, PackageError> {
    let mut files = pin_directories(directory)?;
    for (name, length, digest) in [
        ("syncthing.exe", 27_448_104, WINDOWS_AMD64_SHA256),
        ("LICENSE.txt", 16_726, LICENSE_SHA256),
        ("AUTHORS.txt", 16_811, AUTHORS_SHA256),
    ] {
        // Notices have small fixed upper bounds. Their exact bytes are still
        // required by the digest; no CRLF normalization or alternate digest.
        let mut file = open_file(&directory.join(name))?;
        verify_digest(&mut file, length, digest)?;
        files.push(file);
    }
    Ok(files)
}

fn pin_directories(directory: &Path) -> Result<Vec<File>, PackageError> {
    if !directory.is_absolute()
        || directory
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        || !matches!(directory.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_)))
        || crate::paths::assert_outside_repository(directory).is_err()
    {
        return Err(PackageError::InvalidPath);
    }
    let mut pins = Vec::new();
    for ancestor in directory.ancestors().collect::<Vec<_>>().into_iter().rev() {
        let file = OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(ancestor)
            .map_err(|_| PackageError::Io)?;
        validate_object(&file, true)?;
        if ancestor
            .join(".git")
            .try_exists()
            .map_err(|_| PackageError::Io)?
        {
            return Err(PackageError::InvalidPath);
        }
        pins.push(file);
    }
    Ok(pins)
}

fn open_file(path: &Path) -> Result<File, PackageError> {
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| PackageError::Io)?;
    validate_object(&file, false)?;
    Ok(file)
}

fn validate_object(file: &File, directory: bool) -> Result<(), PackageError> {
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(PackageError::Io);
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory
        || !directory && info.nNumberOfLinks != 1
    {
        return Err(PackageError::UnsafeObject);
    }
    Ok(())
}

fn verify_digest(file: &mut File, exact_size: u64, expected: &str) -> Result<(), PackageError> {
    let size = file.metadata().map_err(|_| PackageError::Io)?.len();
    if exact_size != 0 && size != exact_size || exact_size == 0 && size > 256 * 1024 {
        return Err(PackageError::SizeMismatch);
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| PackageError::Io)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 16384];
    let mut remaining = size;
    while remaining > 0 {
        let limit = remaining.min(buffer.len() as u64) as usize;
        let read = file
            .read(&mut buffer[..limit])
            .map_err(|_| PackageError::Io)?;
        if read == 0 {
            return Err(PackageError::SizeMismatch);
        }
        digest.update(&buffer[..read]);
        remaining -= read as u64;
    }
    if file.read(&mut buffer[..1]).map_err(|_| PackageError::Io)? != 0 {
        return Err(PackageError::SizeMismatch);
    }
    let actual: String = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual != expected {
        return Err(PackageError::DigestMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinning_blocks_mutation_and_refuses_existing_writer_and_hardlink() {
        let root = crate::test_temp::exclusive_temp_dir("cryptex-m8-package-pins");
        let path = root.join("fixture");
        std::fs::write(&path, b"synthetic").unwrap();
        let pins = pin_directories(&root).unwrap();
        let writer = OpenOptions::new().write(true).open(&path).unwrap();
        assert!(open_file(&path).is_err());
        drop(writer);
        let pinned = open_file(&path).unwrap();
        assert!(OpenOptions::new().write(true).open(&path).is_err());
        assert!(std::fs::rename(&path, root.join("replaced")).is_err());
        assert!(std::fs::rename(&root, root.with_extension("replaced")).is_err());
        drop(pinned);
        std::fs::hard_link(&path, root.join("alias")).unwrap();
        assert!(matches!(open_file(&path), Err(PackageError::UnsafeObject)));
        std::fs::remove_file(root.join("alias")).unwrap();
        drop(pins);
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }

    #[test]
    fn directory_or_file_reparse_attribute_is_refused() {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::System::IO::DeviceIoControl;
        let root = crate::test_temp::exclusive_temp_dir("cryptex-m8-package-junction");
        let outside = root.join("outside");
        let link = root.join("link");
        std::fs::create_dir(&outside).unwrap();
        std::fs::create_dir(&link).unwrap();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&link)
            .unwrap();
        let substitute: Vec<u16> = format!(r"\??\{}", outside.display())
            .encode_utf16()
            .collect();
        let print: Vec<u16> = outside.as_os_str().encode_wide().collect();
        let length = 8 + (substitute.len() + 1 + print.len() + 1) * 2;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0xA0000003u32.to_le_bytes());
        for word in [
            length as u16,
            0,
            0,
            (substitute.len() * 2) as u16,
            ((substitute.len() + 1) * 2) as u16,
            (print.len() * 2) as u16,
        ] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        for word in substitute
            .into_iter()
            .chain(Some(0))
            .chain(print)
            .chain(Some(0))
        {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        let mut returned = 0;
        assert_ne!(
            unsafe {
                DeviceIoControl(
                    file.as_raw_handle(),
                    0x000900A4,
                    bytes.as_ptr().cast(),
                    bytes.len() as u32,
                    std::ptr::null_mut(),
                    0,
                    &mut returned,
                    std::ptr::null_mut(),
                )
            },
            0
        );
        drop(file);
        assert!(matches!(
            pin_directories(&link),
            Err(PackageError::UnsafeObject)
        ));
        std::fs::create_dir(outside.join("child")).unwrap();
        assert!(matches!(
            pin_directories(&link.join("child")),
            Err(PackageError::UnsafeObject)
        ));
        std::fs::remove_dir(&link).unwrap(); // exact junction, never recurse
        std::fs::remove_dir(outside.join("child")).unwrap();
        std::fs::remove_dir(&outside).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }

    #[test]
    fn digest_has_exact_length_and_bytes_no_normalization() {
        let root = crate::test_temp::exclusive_temp_dir("cryptex-m8-package-digest");
        let path = root.join("fixture");
        std::fs::write(&path, b"synthetic\n").unwrap();
        let expected = format!("{:x}", Sha256::digest(b"synthetic\n"));
        let mut file = open_file(&path).unwrap();
        verify_digest(&mut file, 10, &expected).unwrap();
        assert_eq!(
            verify_digest(&mut file, 11, &expected),
            Err(PackageError::SizeMismatch)
        );
        assert_eq!(
            verify_digest(&mut file, 10, &"0".repeat(64)),
            Err(PackageError::DigestMismatch)
        );
        drop(file);
        std::fs::write(&path, b"synthetic\r\n").unwrap();
        let mut file = open_file(&path).unwrap();
        assert_eq!(
            verify_digest(&mut file, 0, &expected),
            Err(PackageError::DigestMismatch)
        );
        drop(file);
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
}
