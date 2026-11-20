use super::*;
use std::io::BufRead;

/// One fresh native host per Desktop session. Only the typed search query can
/// cross the browser bridge. The admitted profile and reader belong to the host.
pub(crate) fn run(root: &Path, store: Arc<dyn SecureStore>, reader: Arc<RecoveredSearch>) {
    let instance = crate::instance::try_acquire_instance(root).unwrap();
    let context = SearchContext {
        runtime_root: root.into(),
        admission: instance.admission(),
        operation: None,
        store,
        environment: SecureStoreEnvironment::Dev,
        expected_desktop: Arc::new(
            if std::env::var("CRYPTEX_M11_REOPEN_CASE").as_deref() == Ok("installed") {
                desktop_process_identity().expect("installed Desktop must be the sibling parent")
            } else {
                test_desktop_process_identity().unwrap()
            },
        ),
        projection_sync: None,
        connections: None,
        #[cfg(windows)]
        transport_status: None,
        #[cfg(target_os = "macos")]
        transport_identity: None,
        isolated_attachment_writer: false,
        recovered_search: Some(reader),
        purge_reviews: None,
        transport_device_id_override: None,
    };
    let token = std::env::var("CRYPTEX_FILE_FIXTURE_SOCKET").unwrap();
    let listener = ListenerOptions::new()
        .name(attachment_flow::fixture_name(&token))
        .create_sync()
        .unwrap();
    let workers = Arc::new(AtomicUsize::new(0));
    println!("fixture-ready");
    io::stdout().flush().unwrap();
    for command in io::stdin().lock().lines() {
        match command.unwrap().as_str() {
            "finish" => break,
            "serve" => {
                let mut stream = listener.accept().unwrap();
                handle_connection(&mut stream, Some(&context), Some(&workers), None, None).unwrap();
            }
            _ => panic!("unexpected recovery host control"),
        }
    }
}
