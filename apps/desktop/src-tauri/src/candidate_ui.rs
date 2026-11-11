//! Explicit feature-only UI/native-session harness. Native rights never leave
//! one owner thread. Renderer results contain opaque local handles only.
use cryptexd::ipc::{
    DesktopOwnerSecurityOperationInput as Input, DesktopOwnerSecurityOperationPrepared as Prepared,
    DesktopOwnerSecurityOperationResult as Finished,
};
use cryptexd::local_signed_startup::{CONTENT_MARKER, LocalDaemon, Prepared as Startup};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

type Reply<T> = SyncSender<Result<T, String>>;
enum Work {
    Bind([u8; 32], Reply<()>),
    Ping(Reply<()>),
    Prepare(Input, Reply<Prepared>),
    Finish(String, bool, Reply<Finished>),
    Exercise(Reply<()>),
    PendingLoss(Reply<String>),
    RejectOld(String, Reply<()>),
    FaultPrepare(bool, Reply<()>),
    VerifyInvalidated(Reply<()>),
}
struct Queued {
    deadline: Instant,
    interested: Arc<AtomicBool>,
    work: Work,
}
struct Interest(Arc<AtomicBool>);
impl Interest {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }
}
impl Drop for Interest {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
#[derive(Clone)]
struct Client {
    sender: SyncSender<Queued>,
}
impl Client {
    fn call<T>(&self, make: impl FnOnce(Reply<T>) -> Work) -> Result<T, String> {
        let interest = Interest::new();
        self.call_with(Arc::clone(&interest.0), make)
    }
    fn call_with<T>(
        &self,
        interested: Arc<AtomicBool>,
        make: impl FnOnce(Reply<T>) -> Work,
    ) -> Result<T, String> {
        let (reply, receive) = mpsc::sync_channel(1);
        self.sender
            .try_send(Queued {
                deadline: Instant::now() + Duration::from_secs(25),
                interested,
                work: make(reply),
            })
            .map_err(|_| "candidate_queue_unavailable")?;
        receive
            .recv_timeout(Duration::from_secs(25))
            .map_err(|_| "candidate_result_uncertain")?
    }
    fn exercise_lost_receiver(&self, expired: bool) -> Result<(), String> {
        let (reply, receive) = mpsc::sync_channel(1);
        if !expired {
            drop(receive);
        }
        self.sender
            .try_send(Queued {
                deadline: if expired {
                    Instant::now()
                } else {
                    Instant::now() + Duration::from_secs(25)
                },
                interested: Arc::new(AtomicBool::new(true)),
                work: Work::Prepare(self_retirement(), reply),
            })
            .map_err(|_| "candidate_queue")?;
        self.call(Work::VerifyInvalidated)
    }
}
fn self_retirement() -> Input {
    Input {
        kind: "self_retirement".into(),
        target_device_id: None,
        reason: None,
        cutoff_seq: None,
    }
}
fn deliver<T>(daemon: &mut LocalDaemon, reply: Reply<T>, result: Result<T, String>) {
    if reply.send(result).is_err() {
        daemon.invalidate_session();
    }
}
async fn async_call<T: Send + 'static>(
    client: Client,
    make: impl FnOnce(Reply<T>) -> Work + Send + 'static,
) -> Result<T, String> {
    // Cancelling the async Tauri task drops this lease even if its blocking
    // worker has not reached the queue yet.
    let interest = Interest::new();
    let interested = Arc::clone(&interest.0);
    tauri::async_runtime::spawn_blocking(move || client.call_with(interested, make))
        .await
        .map_err(|_| "candidate_worker")?
}
struct Owner {
    client: Client,
    thread: Option<std::thread::JoinHandle<()>>,
    stopping: Arc<AtomicBool>,
    done: mpsc::Receiver<Result<(), String>>,
}
impl Owner {
    fn start(runtime: &Path) -> Result<(Self, cryptexd::ProtectedContent), String> {
        use std::os::unix::fs::DirBuilderExt;
        if !runtime.is_absolute() {
            return Err("candidate_runtime".into());
        }
        cryptexd::paths::assert_outside_repository(runtime).map_err(|_| "candidate_runtime")?;
        let parent = runtime
            .parent()
            .ok_or("candidate_runtime")?
            .canonicalize()
            .map_err(|_| "candidate_runtime")?;
        if !parent.starts_with(
            std::env::temp_dir()
                .canonicalize()
                .map_err(|_| "candidate_runtime")?,
        ) {
            return Err("candidate_runtime".into());
        }
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(runtime)
            .map_err(|_| "candidate_runtime")?;
        std::fs::write(runtime.join("m8-content.marker"), CONTENT_MARKER)
            .map_err(|_| "candidate_runtime")?;
        let runtime = runtime.to_path_buf();
        let (sender, receiver) = mpsc::sync_channel::<Queued>(8);
        let (init, ready) = mpsc::sync_channel(1);
        let (done_send, done) = mpsc::sync_channel(1);
        let stopping = Arc::new(AtomicBool::new(false));
        let stop_worker = Arc::clone(&stopping);
        let thread = std::thread::Builder::new()
            .name("cryptex-native-ui-owner".into())
            .spawn(move || {
                let startup = (|| {
                    let prepared = Startup::current_content(Duration::from_secs(15))
                        .map_err(|e| e.to_string())?;
                    let path = prepared.daemon_path();
                    let mut command = super::daemon_hold_command(
                        path.to_str().ok_or("candidate_path")?,
                        &runtime,
                    )?;
                    command.stderr(std::process::Stdio::inherit());
                    let daemon = prepared
                        .launch(command, &runtime.join("ready"))
                        .map_err(|e| e.to_string())?;
                    let content = daemon.content().map_err(|e| e.to_string())?;
                    Ok::<_, String>((daemon, content))
                })();
                let (mut daemon, content) = match startup {
                    Ok(value) => value,
                    Err(error) => {
                        let _ = init.send(Err(error));
                        return;
                    }
                };
                if init.send(Ok(content)).is_err() {
                    return;
                }
                loop {
                    if stop_worker.load(Ordering::Acquire) {
                        let receipt = daemon
                            .fixture_status()
                            .map(|value| value.to_string())
                            .unwrap_or_else(|_| {
                                "{\"sessionInvalidated\":true,\"outcome\":\"uncertain\"}".into()
                            });
                        let saved = std::fs::write(runtime.join("ui-receipt.json"), receipt)
                            .map_err(|_| "candidate_receipt_write".to_owned());
                        let cleanup = daemon
                            .shutdown(&runtime.join("stop"))
                            .map_err(|e| e.to_string());
                        let _ = done_send.send(cleanup.and(saved));
                        return;
                    }
                    let queued = match receiver.recv_timeout(Duration::from_millis(50)) {
                        Ok(work) => work,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(_) => {
                            stop_worker.store(true, Ordering::Release);
                            continue;
                        }
                    };
                    if Instant::now() >= queued.deadline
                        || !queued.interested.load(Ordering::Acquire)
                    {
                        daemon.invalidate_session();
                        continue;
                    }
                    match queued.work {
                        Work::Bind(binding, reply) => {
                            let result = daemon.bind_content(&binding).map_err(|e| e.to_string());
                            deliver(&mut daemon, reply, result);
                        }
                        Work::Ping(reply) => {
                            let result = daemon.ping().map_err(|e| e.to_string());
                            deliver(&mut daemon, reply, result);
                        }
                        Work::Prepare(input, reply) => {
                            let result = daemon.prepare_owner(input).map_err(|e| e.to_string());
                            deliver(&mut daemon, reply, result);
                        }
                        Work::Finish(handle, confirm, reply) => {
                            let result = daemon
                                .finish_owner(&handle, confirm)
                                .map_err(|e| e.to_string());
                            deliver(&mut daemon, reply, result);
                        }
                        Work::Exercise(reply) => {
                            let result = daemon
                                .exercise_synthetic_owner_sessions()
                                .map_err(|e| e.to_string());
                            deliver(&mut daemon, reply, result);
                        }
                        Work::PendingLoss(reply) => {
                            let result = (|| {
                                let prepared = daemon.prepare_owner(self_retirement())?;
                                daemon.terminate_for_restart()?;
                                daemon.require_stale_rejection()?;
                                if daemon.finish_owner(&prepared.session_handle, true).is_ok()
                                    || daemon.finish_owner(&prepared.session_handle, true).is_ok()
                                {
                                    return Err(
                                        cryptexd::local_signed_startup::Error::StaleAccepted,
                                    );
                                }
                                Ok(prepared.session_handle)
                            })()
                            .map_err(|e| e.to_string());
                            deliver(&mut daemon, reply, result);
                        }
                        Work::RejectOld(handle, reply) => {
                            let result = if daemon.finish_owner(&handle, true).is_err() {
                                Ok(())
                            } else {
                                Err("candidate_old_handle_accepted".into())
                            };
                            deliver(&mut daemon, reply, result);
                        }
                        Work::FaultPrepare(missing, reply) => {
                            let result = daemon
                                .exercise_prepare_failure(missing)
                                .map_err(|e| e.to_string());
                            deliver(&mut daemon, reply, result);
                        }
                        Work::VerifyInvalidated(reply) => {
                            let result = daemon.verify_invalidated().map_err(|e| e.to_string());
                            deliver(&mut daemon, reply, result);
                        }
                    }
                }
            })
            .map_err(|_| "candidate_worker")?;
        match ready.recv().map_err(|_| "candidate_worker")? {
            Ok(content) => Ok((
                Self {
                    client: Client { sender },
                    thread: Some(thread),
                    stopping,
                    done,
                },
                content,
            )),
            Err(error) => {
                let _ = thread.join();
                Err(error)
            }
        }
    }
    fn close(mut self) -> Result<(), String> {
        self.stopping.store(true, Ordering::Release);
        let result = self
            .done
            .recv()
            .unwrap_or_else(|_| Err("candidate_worker_cleanup".into()));
        if let Some(thread) = self.thread.take() {
            thread.join().map_err(|_| "candidate_worker_cleanup")?;
        }
        result
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            self.stopping.store(true, Ordering::Release);
            if thread.join().is_err() {
                eprintln!("candidate_worker_cleanup_failed");
            }
        }
    }
}
#[tauri::command]
async fn m8_prepare_owner_security_operation(
    state: tauri::State<'_, Client>,
    input: Input,
) -> Result<Prepared, String> {
    async_call(state.inner().clone(), move |reply| {
        Work::Prepare(input, reply)
    })
    .await
}
#[tauri::command]
async fn m8_confirm_owner_security_operation(
    state: tauri::State<'_, Client>,
    session_handle: String,
) -> Result<Finished, String> {
    async_call(state.inner().clone(), move |reply| {
        Work::Finish(session_handle, true, reply)
    })
    .await
}
#[tauri::command]
async fn m8_cancel_owner_security_operation(
    state: tauri::State<'_, Client>,
    session_handle: String,
) -> Result<Finished, String> {
    async_call(state.inner().clone(), move |reply| {
        Work::Finish(session_handle, false, reply)
    })
    .await
}
#[tauri::command]
async fn p0_3_ipc_health(state: tauri::State<'_, Client>) -> Result<(), String> {
    async_call(state.inner().clone(), Work::Ping).await
}
#[tauri::command]
fn p0_5_set_capture_session_guard() {}
fn install_context(
    content: cryptexd::ProtectedContent,
    runtime: &Path,
) -> Result<(tauri::Context<tauri::Wry>, [u8; 32]), String> {
    let original = content
        .get("/index.html")
        .ok_or("candidate_frontend")?
        .to_vec();
    let binding = *content.binding();
    let mut context = tauri::generate_context!();
    super::protected_assets::install(&mut context, content, runtime);
    if context.assets().get(&"/index.html".into()).as_deref() != Some(original.as_slice())
        || context.assets().get(&"/unlisted.js".into()).is_some()
        || context.config().build.dev_url.is_some()
        || context.config().identifier != "com.cryptex.m8.native-ui-fixture"
        || context.config().app.app_directories_override.is_none()
        || context
            .config()
            .app
            .windows
            .iter()
            .any(|w| !w.incognito || !matches!(&w.url, tauri::WebviewUrl::App(_)))
    {
        return Err("candidate_asset_provider".into());
    }
    Ok((context, binding))
}
pub fn entry() -> Option<Result<(), String>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !args.first().is_some_and(|arg| {
        arg.to_string_lossy()
            .starts_with("--cryptex-m8-local-content")
    }) {
        return None;
    }
    Some((|| {
        if args.len() != 2
            || (args[0] != "--cryptex-m8-local-content-check"
                && args[0] != "--cryptex-m8-local-content-ui")
        {
            return Err("candidate_arguments".into());
        }
        let runtime = PathBuf::from(&args[1]);
        let (owner, content) = Owner::start(&runtime)?;
        let (mut context, binding) = install_context(content, &runtime)?;
        owner.client.call(|reply| Work::Bind(binding, reply))?;
        if args[0] == "--cryptex-m8-local-content-check" {
            owner.client.call(Work::Exercise)?;
            owner.close()?;
            let lost_root = runtime.with_extension("lost");
            let (lost, content) = Owner::start(&lost_root)?;
            let (_context, binding) = install_context(content, &lost_root)?;
            lost.client.call(|reply| Work::Bind(binding, reply))?;
            let handle = lost.client.call(Work::PendingLoss)?;
            drop(lost);
            let fresh_root = runtime.with_extension("fresh");
            let (fresh, content) = Owner::start(&fresh_root)?;
            let (_context, binding) = install_context(content, &fresh_root)?;
            fresh.client.call(|reply| Work::Bind(binding, reply))?;
            fresh.client.call(|reply| Work::RejectOld(handle, reply))?;
            fresh.close()?;
            for (name, missing, expired) in [
                ("missing-snapshot", Some(true), false),
                ("handle-allocation", Some(false), false),
                ("expired-queue", None, true),
                ("lost-receiver", None, false),
            ] {
                let root = runtime.with_extension(name);
                let (fault, content) = Owner::start(&root)?;
                let (_context, binding) = install_context(content, &root)?;
                fault.client.call(|reply| Work::Bind(binding, reply))?;
                if let Some(missing) = missing {
                    fault
                        .client
                        .call(|reply| Work::FaultPrepare(missing, reply))?;
                } else {
                    fault.client.exercise_lost_receiver(expired)?;
                }
                fault.close()?;
            }
            println!("verified_tauri_assets=passed");
            println!("native_synthetic_approval_sessions=passed");
            println!("native_lost_session_and_old_handle_refused=passed");
            println!("native_prepare_and_actor_failures_burn_grants=passed");
            return Ok(());
        }
        let mut window = context
            .config()
            .app
            .windows
            .first()
            .cloned()
            .ok_or("candidate_window")?;
        window.title = "Cryptex · İzole onay testi".into();
        context.config_mut().app.windows.clear();
        tauri::Builder::default()
            .manage(owner.client.clone())
            .invoke_handler(tauri::generate_handler![
                m8_prepare_owner_security_operation,
                m8_confirm_owner_security_operation,
                m8_cancel_owner_security_operation,
                p0_3_ipc_health,
                p0_5_set_capture_session_guard,
            ])
            .setup(move |app| {
                tauri::WebviewWindowBuilder::from_config(app, &window)?
                    .incognito(true)
                    .on_navigation(|url| {
                        url.scheme() == "tauri" && url.host_str() == Some("localhost")
                    })
                    .build()?;
                Ok(())
            })
            .build(context)
            .map_err(|_| "candidate_window")?
            .run(|_, _| {});
        owner.close()
    })())
}
