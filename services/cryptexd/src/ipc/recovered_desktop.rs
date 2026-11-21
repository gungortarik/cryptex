use super::*;
use std::io::{BufRead, Write};

#[cfg(test)]
pub(crate) fn run_fixture(root: &Path, store: Arc<dyn SecureStore>) {
    serve(
        root,
        store,
        test_desktop_process_identity().unwrap(),
        |_| Ok(()),
    )
    .unwrap();
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
    // Rebuild the already validated child under the native temp directory.
    // On Windows canonicalize yields a verbatim prefix while the pinned
    // directory adapter requires a rooted drive-letter path.
    let work = std::env::temp_dir().join(
        work.file_name()
            .ok_or_else(|| invalid_data("missing fixture name"))?,
    );
    let exporting = std::env::var_os("CRYPTEX_M11_INSTALLED_EXPORT").is_some();
    let verifying =
        std::env::var("CRYPTEX_M11_INSTALLED_EXPORT").is_ok_and(|mode| mode.starts_with("verify"));
    let (profile, credentials) = if std::env::var("CRYPTEX_M11_INSTALLED_EXPORT")
        .is_ok_and(|m| m.starts_with("first-device-check") || m.starts_with("first-device-clean"))
    {
        ("clean-replacement", "clean-credentials")
    } else if std::env::var("CRYPTEX_M11_INSTALLED_EXPORT").is_ok_and(|m| {
        matches!(
            m.as_str(),
            "writer-transfer-request" | "writer-transfer-accept" | "writer-transfer-peer"
        )
    }) {
        ("writer-peer", "writer-peer-credentials")
    } else if std::env::var_os("CRYPTEX_M11_REHEARSE_RECOVERED_SCHEDULE").is_some() {
        ("replacement", "test-credentials")
    } else if verifying {
        ("verify-session", "verify-credentials")
    } else if exporting
        && !std::env::var("CRYPTEX_M11_INSTALLED_EXPORT")
            .is_ok_and(|mode| mode == "pair-write" || mode.starts_with("first-device-"))
    {
        ("export-source", "export-credentials")
    } else {
        ("replacement", "test-credentials")
    };
    #[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
    if profile == "clean-replacement" && !work.join(profile).try_exists()? {
        crate::instance::create_m9_profile_root(&work, profile)
            .map_err(|_| invalid_data("could not prepare replacement destination"))?;
    }
    let store = crate::secure_store::FileBackedSecureStore::open(work.join(credentials))
        .map_err(|_| invalid_data("credentials unavailable"))?;
    serve(&work.join(profile), Arc::new(store), desktop, |ctx| {
        #[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
        if exporting {
            let exe = std::env::current_exe()?;
            let parent = exe
                .parent()
                .ok_or_else(|| invalid_data("installed directory"))?;
            let resources = if cfg!(windows) {
                parent.to_owned()
            } else {
                parent.join("../Resources")
            };
            ctx.backup_export = Some(Arc::new(backup_export::Config {
                codec: resources.join("m11-codec"),
                busy: Mutex::new(()),
            }));
        }
        let _ = ctx;
        Ok(())
    })
}

fn serve(
    root: &Path,
    store: Arc<dyn SecureStore>,
    desktop: DesktopProcessIdentity,
    configure: impl FnOnce(&mut SearchContext) -> io::Result<()>,
) -> io::Result<()> {
    let instance = crate::instance::try_acquire_instance(root)
        .map_err(|_| invalid_data("instance unavailable"))?;
    let mut context = SearchContext {
        runtime_root: root.into(),
        admission: instance.admission(),
        operation: None,
        store,
        environment: SecureStoreEnvironment::Dev,
        public_consent: Default::default(),
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
        #[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
        backup_export: None,
        backup_removal: Default::default(),
        #[cfg(test)]
        purge_reviews: None,
        #[cfg(test)]
        transport_device_id_override: None,
    };
    configure(&mut context)?;
    #[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
    let _schedule = backup_writer::start(Some(&context))?;
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
