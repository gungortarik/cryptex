//! Authenticate the full installed image with the owner's independent build-time DER pin.
use crate::{error, package_digest};
use cryptex_updater::{Layout, desktop};
use std::{
    env, fs,
    io::{self, Read, Write},
    path::Path,
};

const PIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/update-signer.der"));
const MAX_CMS: u64 = 65536;

fn read_receipt(path: &Path) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || !(1..=MAX_CMS).contains(&metadata.len()) {
        return Err(error("invalid release receipt file"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_CMS + 1)
        .read_to_end(&mut bytes)?;
    if !(1..=MAX_CMS as usize).contains(&bytes.len()) {
        return Err(error("invalid release receipt size"));
    }
    Ok(bytes)
}

fn content(digest: &str) -> String {
    format!(
        "cryptex.owner-package.v1\nos={}\narch={}\nsha256={digest}\n",
        env::consts::OS,
        env::consts::ARCH
    )
}

#[cfg(any(windows, target_os = "macos"))]
unsafe extern "C" {
    fn cx_verify_inventory(
        cms: *const u8,
        size: usize,
        content: *const u8,
        length: usize,
        pin: *const u8,
        pin_size: usize,
    ) -> i32;
}

fn authenticate(package: &Path, digest: &str, receipt: &[u8]) -> io::Result<()> {
    if PIN.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "owner release signer is not provisioned",
        ));
    }
    #[cfg(any(windows, target_os = "macos"))]
    {
        let message = content(digest);
        // The native verifier compares the authenticated, attached CMS content and the exact signing leaf.
        let result = unsafe {
            cx_verify_inventory(
                receipt.as_ptr(),
                receipt.len(),
                message.as_ptr(),
                message.len(),
                PIN.as_ptr(),
                PIN.len(),
            )
        };
        if result != 0 {
            return Err(error("owner release receipt verification failed"));
        }
        let gui = desktop(package);
        verify_image(&gui, c"com.cryptex.desktop")?;
        verify_image(
            &gui.with_file_name(if cfg!(windows) {
                "cryptexd.exe"
            } else {
                "cryptexd"
            }),
            c"com.cryptex.daemon",
        )
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (receipt, desktop(package), content(digest));
        Err(error("unsupported release signing platform"))
    }
}

#[cfg(windows)]
fn verify_image(path: &Path, _role: &std::ffi::CStr) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    unsafe extern "C" {
        fn cx_update_image(path: *const u16, pin: *const u8, size: usize) -> i32;
    }
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe { cx_update_image(path.as_ptr(), PIN.as_ptr(), PIN.len()) } != 0 {
        return Err(error(
            "installed image does not carry the expected Authenticode signature",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_image(path: &Path, role: &std::ffi::CStr) -> io::Result<()> {
    use std::{
        ffi::{CString, c_char},
        os::unix::ffi::OsStrExt,
    };
    unsafe extern "C" {
        fn cx_update_image(
            path: *const c_char,
            role: *const c_char,
            pin: *const u8,
            size: usize,
        ) -> i32;
    }
    let path = CString::new(path.as_os_str().as_bytes())?;
    if unsafe { cx_update_image(path.as_ptr(), role.as_ptr(), PIN.as_ptr(), PIN.len()) } != 0 {
        return Err(error(
            "installed image does not carry the expected Mac role and signer",
        ));
    }
    Ok(())
}

pub(crate) fn verify(layout: &Layout, package: &Path, expected: &str) -> io::Result<()> {
    // Hash the complete image before constructing any receipt path from a journal value.
    let digest = package_digest(package)?;
    if digest != expected {
        return Err(error("installed image differs from its approved digest"));
    }
    let receipt = read_receipt(&layout.release_receipts().join(&digest))?;
    authenticate(package, &digest, &receipt)
}

pub(crate) fn admit(layout: &Layout, package: &Path, source: &Path) -> io::Result<()> {
    let digest = package_digest(package)?;
    let receipt = read_receipt(source)?;
    authenticate(package, &digest, &receipt)?;
    let dir = layout.release_receipts();
    fs::create_dir_all(&dir)?;
    let destination = dir.join(&digest);
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    temp.write_all(&receipt)?;
    temp.as_file().sync_all()?;
    match temp.persist_noclobber(&destination) {
        Ok(_) => Ok(()),
        Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => {
            authenticate(package, &digest, &read_receipt(&destination)?)
        }
        Err(e) => Err(e.error),
    }
}
