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
        expected_desktop: Arc::new(tests::test_connected_process_identity()),
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
    let name = attachment_flow::fixture_name(&new_bootstrap_event_id().unwrap());
    let listener = ListenerOptions::new()
        .name(name.clone())
        .create_sync()
        .unwrap();
    FIXTURE_SOCKET.with(|socket| socket.set(name).unwrap());
    let workers = Arc::new(AtomicUsize::new(0));
    println!("desktop-search-ready");
    io::stdout().flush().unwrap();
    for line in io::stdin().lock().lines() {
        let query: DesktopSearchQuery = serde_json::from_str(&line.unwrap()).unwrap();
        let response = thread::scope(|scope| {
            let server = scope.spawn(|| {
                let mut stream = listener.accept().unwrap();
                handle_connection(&mut stream, Some(&context), Some(&workers), None, None).unwrap();
            });
            let response = desktop_search(query, Duration::from_secs(2)).unwrap();
            server.join().unwrap();
            response
        });
        println!(
            "desktop-search-reply {}",
            serde_json::to_string(&response).unwrap()
        );
        io::stdout().flush().unwrap();
    }
}
