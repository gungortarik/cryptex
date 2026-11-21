use super::*;
use std::io::{BufRead, Write};

#[cfg(test)]
pub(crate) fn run_fixture(root: &Path, store: Arc<dyn SecureStore>) {
    serve(root, store, test_desktop_process_identity().unwrap()).unwrap();
}

/// Isolated startup adapter for the installed CI package. Profile selection and
/// synthetic credentials stay outside the normal reader and renderer protocol.
#[cfg(feature = "m11-installed-search-check")]
pub fn installed() -> io::Result<()> {
    if !std::env::args_os()
        .skip(1)
        .eq([std::ffi::OsString::from("--rehearse-recovered-search")])
    {
        return Err(invalid_data("unexpected rehearsal arguments"));
    }
    let desktop = desktop_process_identity()?;
    let work = PathBuf::from(
        std::env::var_os("CRYPTEX_M11_REOPEN_FIXTURE")
            .ok_or_else(|| invalid_data("missing fixture"))?,
    )
    .canonicalize()?;
    if work.parent() != Some(std::env::temp_dir().canonicalize()?.as_path())
        || !work
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("cryptex-m11-recovery-"))
    {
        return Err(invalid_data("invalid fixture"));
    }
    let store = crate::secure_store::FileBackedSecureStore::open(work.join("test-credentials"))
        .map_err(|_| invalid_data("credentials unavailable"))?;
    serve(&work.join("replacement"), Arc::new(store), desktop)
}

fn serve(
    root: &Path,
    store: Arc<dyn SecureStore>,
    desktop: DesktopProcessIdentity,
) -> io::Result<()> {
    let instance = crate::instance::try_acquire_instance(root)
        .map_err(|_| invalid_data("instance unavailable"))?;
    let context = SearchContext {
        runtime_root: root.into(),
        admission: instance.admission(),
        operation: None,
        store,
        environment: SecureStoreEnvironment::Dev,
        expected_desktop: Arc::new(desktop),
        projection_sync: None,
        #[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
        connections: None,
        #[cfg(windows)]
        transport_status: None,
        #[cfg(all(target_os = "macos", feature = "m8-local-startup-check"))]
        transport_identity: None,
        #[cfg(test)]
        isolated_attachment_writer: false,
        backup_removal: Default::default(),
        #[cfg(test)]
        purge_reviews: None,
        #[cfg(test)]
        transport_device_id_override: None,
    };
    let token = std::env::var("CRYPTEX_FILE_FIXTURE_SOCKET")
        .map_err(|_| invalid_data("missing socket ID"))?;
    let listener = ListenerOptions::new()
        .name(rehearsal_socket(&token)?)
        .create_sync()?;
    let workers = Arc::new(AtomicUsize::new(0));
    println!("fixture-ready");
    io::stdout().flush()?;
    for command in io::stdin().lock().lines() {
        match command?.as_str() {
            "finish" => break,
            "serve" => {
                let mut stream = listener.accept()?;
                handle_connection(&mut stream, Some(&context), Some(&workers), None, None)?;
            }
            _ => return Err(invalid_data("unexpected host control")),
        }
    }
    Ok(())
}
