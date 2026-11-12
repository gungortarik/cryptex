//! Explicit feature-only UI/native-session harness. Native rights never leave
//! one owner thread. Renderer results contain opaque local handles only.
use cryptexd::ipc::{
    DesktopOwnerSecurityOperationInput as Input, DesktopOwnerSecurityOperationPrepared as Prepared,
    DesktopOwnerSecurityOperationResult as Finished,
};
use cryptexd::local_signed_startup::{
    CONTENT_MARKER, ContentProfile, DURABLE_MARKER, LocalDaemon, Prepared as Startup,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, Ordering},
};
use std::time::{Duration, Instant};

type Reply<T> = SyncSender<Result<T, String>>;
enum Work {
    Bind([u8; 32], Reply<()>),
    Ping(Reply<()>),
    VerifyTwoEvents(Reply<()>),
    Prepare(Input, Reply<Prepared>),
    Finish(String, bool, Reply<Finished>),
    Exercise(Reply<()>),
    PendingLoss(Reply<String>),
    RejectOld(String, Reply<()>),
    FaultPrepare(bool, Reply<()>),
    VerifyInvalidated(Reply<()>),
    PrepareGated(Reply<()>, mpsc::Receiver<()>, Reply<Prepared>),
    PrepareDelivered(mpsc::Receiver<()>, Reply<Prepared>),
    PrepareHandoff(Reply<()>, Reply<Prepared>),
}
struct Queued {
    deadline: Instant,
    interested: Arc<AtomicU8>,
    work: Work,
}
const ACTIVE: u8 = 0;
const CANCELLED: u8 = 1;
const COMPLETED: u8 = 2;
struct Interest(Arc<AtomicU8>);
impl Interest {
    fn new() -> Self {
        Self(Arc::new(AtomicU8::new(ACTIVE)))
    }
    fn complete(self) {
        self.0.store(COMPLETED, Ordering::Release);
    }
}
impl Drop for Interest {
    fn drop(&mut self) {
        let _ = self
            .0
            .compare_exchange(ACTIVE, CANCELLED, Ordering::AcqRel, Ordering::Acquire);
    }
}
#[derive(Clone)]
struct Client {
    sender: SyncSender<Queued>,
}
impl Client {
    fn call<T>(&self, make: impl FnOnce(Reply<T>) -> Work) -> Result<T, String> {
        let interest = Interest::new();
        let result = self.call_with(Arc::clone(&interest.0), make);
        if result.is_ok() {
            interest.complete();
        }
        result
    }
    fn call_with<T>(
        &self,
        interested: Arc<AtomicU8>,
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
                interested: Arc::new(AtomicU8::new(ACTIVE)),
                work: Work::Prepare(self_retirement(), reply),
            })
            .map_err(|_| "candidate_queue")?;
        self.call(Work::VerifyInvalidated)
    }
    fn exercise_cancelled_inflight(&self) -> Result<(), String> {
        let (started, prepared) = mpsc::sync_channel(1);
        let (release, proceed) = mpsc::sync_channel(1);
        let (reply, receive) = mpsc::sync_channel(1);
        let interested = Arc::new(AtomicU8::new(ACTIVE));
        self.sender
            .try_send(Queued {
                deadline: Instant::now() + Duration::from_secs(25),
                interested: Arc::clone(&interested),
                work: Work::PrepareGated(started, proceed, reply),
            })
            .map_err(|_| "candidate_queue")?;
        prepared
            .recv_timeout(Duration::from_secs(25))
            .map_err(|_| "candidate_prepare_gate")??;
        // Match cancellation of the async future while spawn_blocking still
        // retains its receiver. Losing interest must burn the real grant.
        interested.store(CANCELLED, Ordering::Release);
        release.send(()).map_err(|_| "candidate_prepare_gate")?;
        self.call(Work::VerifyInvalidated)?;
        if receive.try_recv().is_ok() {
            return Err("candidate_cancelled_result_delivered".into());
        }
        Ok(())
    }
    fn exercise_completed_delivery(&self) -> Result<(), String> {
        let (release, proceed) = mpsc::sync_channel(1);
        let (reply, receive) = mpsc::sync_channel(1);
        let interest = Interest::new();
        self.sender
            .try_send(Queued {
                deadline: Instant::now() + Duration::from_secs(25),
                interested: Arc::clone(&interest.0),
                work: Work::PrepareDelivered(proceed, reply),
            })
            .map_err(|_| "candidate_queue")?;
        let prepared = receive
            .recv_timeout(Duration::from_secs(25))
            .map_err(|_| "candidate_delivery_gate")??;
        // Force normal completion before the worker's post-send check.
        interest.complete();
        release.send(()).map_err(|_| "candidate_delivery_gate")?;
        if !self
            .call(|reply| Work::Finish(prepared.session_handle, false, reply))?
            .accepted
        {
            return Err("candidate_delivered_cancel_refused".into());
        }
        Ok(())
    }
    fn exercise_cancelled_handoff(&self) -> Result<(), String> {
        let (sent, delivered) = mpsc::sync_channel(1);
        let (reply, _receive) = mpsc::sync_channel(1);
        let interest = Interest::new();
        self.sender
            .try_send(Queued {
                deadline: Instant::now() + Duration::from_secs(25),
                interested: Arc::clone(&interest.0),
                work: Work::PrepareHandoff(sent, reply),
            })
            .map_err(|_| "candidate_queue")?;
        delivered
            .recv_timeout(Duration::from_secs(25))
            .map_err(|_| "candidate_handoff_gate")??;
        // Worker has finished both delivery checks; async awaiter has not
        // consumed its result. Its cancellation must still be observed.
        drop(interest);
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
#[derive(Clone)]
struct Delivery {
    deadline: Instant,
    interested: Arc<AtomicU8>,
}
impl Delivery {
    fn active(&self) -> bool {
        match self.interested.load(Ordering::Acquire) {
            COMPLETED => true,
            ACTIVE => Instant::now() < self.deadline,
            _ => false,
        }
    }
}
fn check_pending_delivery(daemon: &mut LocalDaemon, pending: &mut Option<Delivery>) {
    if let Some(delivery) = pending.take() {
        // One state snapshot: if ACTIVE changes to CANCELLED immediately
        // after this load, retaining the lease guarantees the next check
        // observes it. A second load must never silently retire cancellation.
        match delivery.interested.load(Ordering::Acquire) {
            COMPLETED => {}
            ACTIVE if Instant::now() < delivery.deadline => *pending = Some(delivery),
            _ => daemon.invalidate_session(),
        }
    }
}
fn deliver<T>(
    daemon: &mut LocalDaemon,
    delivery: &Delivery,
    reply: Reply<T>,
    result: Result<T, String>,
) {
    deliver_with(daemon, delivery, reply, result, || true);
}
fn deliver_with<T>(
    daemon: &mut LocalDaemon,
    delivery: &Delivery,
    reply: Reply<T>,
    result: Result<T, String>,
    after_send: impl FnOnce() -> bool,
) {
    if !delivery.active() || reply.send(result).is_err() || !after_send() || !delivery.active() {
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
    let result = tauri::async_runtime::spawn_blocking(move || client.call_with(interested, make))
        .await
        .map_err(|_| "candidate_worker")?;
    if result.is_ok() {
        interest.complete();
    }
    result
}
struct Owner {
    client: Client,
    thread: Option<std::thread::JoinHandle<()>>,
    stopping: Arc<AtomicBool>,
    done: mpsc::Receiver<Result<(), String>>,
}
impl Owner {
    fn start(runtime: &Path) -> Result<(Self, cryptexd::ProtectedContent), String> {
        Self::start_profile(runtime, ContentProfile::Ephemeral)
    }
    fn start_profile(
        runtime: &Path,
        profile: ContentProfile,
    ) -> Result<(Self, cryptexd::ProtectedContent), String> {
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
        if profile != ContentProfile::KeychainResume {
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(runtime)
                .map_err(|_| "candidate_runtime")?;
            std::fs::write(
                runtime.join("m8-content.marker"),
                if profile == ContentProfile::Ephemeral {
                    CONTENT_MARKER
                } else {
                    DURABLE_MARKER
                },
            )
            .map_err(|_| "candidate_runtime")?;
        }
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
                    let prepared = Startup::current_profile(Duration::from_secs(15), profile)
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
                let mut pending_delivery = None;
                loop {
                    check_pending_delivery(&mut daemon, &mut pending_delivery);
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
                    check_pending_delivery(&mut daemon, &mut pending_delivery);
                    if Instant::now() >= queued.deadline
                        || queued.interested.load(Ordering::Acquire) != ACTIVE
                    {
                        daemon.invalidate_session();
                        continue;
                    }
                    let delivery = Delivery {
                        deadline: queued.deadline,
                        interested: queued.interested,
                    };
                    match queued.work {
                        Work::Bind(binding, reply) => {
                            let result = daemon.bind_content(&binding).map_err(|e| e.to_string());
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::VerifyTwoEvents(reply) => {
                            let result = daemon
                                .fixture_status()
                                .map_err(|e| e.to_string())
                                .and_then(|status| {
                                    if status["eventCount"] == 2
                                        && status["prefixPreserved"] == true
                                    {
                                        Ok(())
                                    } else {
                                        Err("candidate_restart_event_mismatch".into())
                                    }
                                });
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::Ping(reply) => {
                            let result = daemon.ping().map_err(|e| e.to_string());
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::Prepare(input, reply) => {
                            let result = daemon.prepare_owner(input).map_err(|e| e.to_string());
                            if result.is_ok() {
                                pending_delivery = Some(delivery.clone());
                            }
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::Finish(handle, confirm, reply) => {
                            let result = daemon
                                .finish_owner(&handle, confirm)
                                .map_err(|e| e.to_string());
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::Exercise(reply) => {
                            let result = daemon
                                .exercise_synthetic_owner_sessions()
                                .map_err(|e| e.to_string());
                            deliver(&mut daemon, &delivery, reply, result);
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
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::RejectOld(handle, reply) => {
                            let result = if daemon.finish_owner(&handle, true).is_err() {
                                Ok(())
                            } else {
                                Err("candidate_old_handle_accepted".into())
                            };
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::FaultPrepare(missing, reply) => {
                            let result = daemon
                                .exercise_prepare_failure(missing)
                                .map_err(|e| e.to_string());
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::VerifyInvalidated(reply) => {
                            let result = daemon.verify_invalidated().map_err(|e| e.to_string());
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::PrepareGated(started, proceed, reply) => {
                            let result = daemon
                                .prepare_owner(self_retirement())
                                .map_err(|e| e.to_string());
                            if result.is_ok() {
                                pending_delivery = Some(delivery.clone());
                            }
                            let ready = result.as_ref().map(|_| ()).map_err(Clone::clone);
                            if started.send(ready).is_err()
                                || proceed.recv_timeout(Duration::from_secs(25)).is_err()
                            {
                                daemon.invalidate_session();
                            } else {
                                deliver(&mut daemon, &delivery, reply, result);
                            }
                        }
                        Work::PrepareDelivered(proceed, reply) => {
                            let result = daemon
                                .prepare_owner(self_retirement())
                                .map_err(|e| e.to_string());
                            if result.is_ok() {
                                pending_delivery = Some(delivery.clone());
                            }
                            deliver_with(&mut daemon, &delivery, reply, result, || {
                                proceed.recv_timeout(Duration::from_secs(25)).is_ok()
                            });
                        }
                        Work::PrepareHandoff(sent, reply) => {
                            let result = daemon
                                .prepare_owner(self_retirement())
                                .map_err(|e| e.to_string());
                            let ready = result.as_ref().map(|_| ()).map_err(Clone::clone);
                            if result.is_ok() {
                                pending_delivery = Some(delivery.clone());
                            }
                            deliver(&mut daemon, &delivery, reply, result);
                            if sent.send(ready).is_err() {
                                daemon.invalidate_session();
                            }
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
fn durable_snapshot(root: &Path) -> Result<std::collections::BTreeMap<PathBuf, Vec<u8>>, String> {
    fn collect(
        path: &Path,
        files: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>,
    ) -> Result<(), String> {
        for entry in std::fs::read_dir(path).map_err(|_| "candidate_snapshot")? {
            let entry = entry.map_err(|_| "candidate_snapshot")?;
            let kind = entry.file_type().map_err(|_| "candidate_snapshot")?;
            if kind.is_symlink() {
                return Err("candidate_snapshot".into());
            }
            if kind.is_dir() {
                collect(&entry.path(), files)?;
            } else {
                files.insert(
                    entry.path(),
                    std::fs::read(entry.path()).map_err(|_| "candidate_snapshot")?,
                );
            }
        }
        Ok(())
    }
    let mut files = std::collections::BTreeMap::new();
    collect(&root.join("identity"), &mut files)?;
    collect(&root.join("canonical"), &mut files)?;
    Ok(files)
}

fn durable_owner(root: &Path, profile: ContentProfile) -> Result<Owner, String> {
    let (owner, content) = Owner::start_profile(root, profile)?;
    let (_context, binding) = install_context(content, root)?;
    owner.client.call(|reply| Work::Bind(binding, reply))?;
    Ok(owner)
}

fn check_durable_restart(root: &Path) -> Result<(), String> {
    let owner = durable_owner(root, ContentProfile::KeychainCreate)?;
    let seed = durable_snapshot(root)?;
    let handle = owner.client.call(Work::PendingLoss)?;
    owner.close()?;
    let resumed = durable_owner(root, ContentProfile::KeychainResume)?;
    if durable_snapshot(root)? != seed {
        return Err("candidate_restart_mutated_seed".into());
    }
    resumed
        .client
        .call(|reply| Work::RejectOld(handle, reply))?;
    resumed.client.call(Work::Exercise)?;
    resumed.close()?;
    let confirmed = durable_snapshot(root)?;
    let reopened = durable_owner(root, ContentProfile::KeychainResume)?;
    reopened.client.call(Work::VerifyTwoEvents)?;
    if durable_snapshot(root)? != confirmed {
        return Err("candidate_restart_mutated_confirmation".into());
    }
    reopened.close()?;
    println!("native_keychain_same_profile_restart=passed");
    println!("native_keychain_confirmed_history_preserved=passed");
    println!("native_keychain_old_local_handle_refused=passed");
    Ok(())
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
        if args.len() == 2 && args[0] == "--cryptex-m8-local-content-durable-check" {
            return check_durable_restart(Path::new(&args[1]));
        }
        if args.len() == 2 && args[0] == "--cryptex-m8-local-content-durable-resume-check" {
            return durable_owner(Path::new(&args[1]), ContentProfile::KeychainResume)?.close();
        }
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
            eprintln!("m8_candidate_scenario=synthetic_sessions");
            owner.client.call(Work::Exercise)?;
            owner.close()?;
            let lost_root = runtime.with_extension("lost");
            eprintln!("m8_candidate_scenario=lost_connection");
            let (lost, content) = Owner::start(&lost_root)?;
            let (_context, binding) = install_context(content, &lost_root)?;
            lost.client.call(|reply| Work::Bind(binding, reply))?;
            let handle = lost.client.call(Work::PendingLoss)?;
            lost.close()?;
            let fresh_root = runtime.with_extension("fresh");
            eprintln!("m8_candidate_scenario=fresh_handle_refusal");
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
                eprintln!("m8_candidate_scenario={name}");
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
            let root = runtime.with_extension("cancelled-inflight");
            eprintln!("m8_candidate_scenario=cancelled_inflight");
            let (fault, content) = Owner::start(&root)?;
            let (_context, binding) = install_context(content, &root)?;
            fault.client.call(|reply| Work::Bind(binding, reply))?;
            fault.client.exercise_cancelled_inflight()?;
            fault.close()?;
            let root = runtime.with_extension("completed-delivery");
            eprintln!("m8_candidate_scenario=completed_delivery");
            let (success, content) = Owner::start(&root)?;
            let (_context, binding) = install_context(content, &root)?;
            success.client.call(|reply| Work::Bind(binding, reply))?;
            success.client.exercise_completed_delivery()?;
            success.close()?;
            let root = runtime.with_extension("cancelled-handoff");
            eprintln!("m8_candidate_scenario=cancelled_handoff");
            let (fault, content) = Owner::start(&root)?;
            let (_context, binding) = install_context(content, &root)?;
            fault.client.call(|reply| Work::Bind(binding, reply))?;
            fault.client.exercise_cancelled_handoff()?;
            fault.close()?;
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
        let app = tauri::Builder::default()
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
            .map_err(|_| "candidate_window")?;
        // AppKit may terminate the process directly after the Exit callback.
        // Finish the receipt and reap the owned child while the callback is live.
        let mut owner = Some(owner);
        app.run(move |_, event| {
            if matches!(event, tauri::RunEvent::Exit)
                && let Some(owner) = owner.take()
                && let Err(error) = owner.close()
            {
                eprintln!("candidate_ui_shutdown: {error}");
                std::process::exit(2);
            }
        });
        Ok(())
    })())
}
