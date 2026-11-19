//! Explicit feature-only UI/native-session harness. Native rights never leave
//! one owner thread. Renderer results contain opaque local handles only.
use cryptexd::ipc::{
    DesktopBootstrapApprovalPrepared as PeerPrepared,
    DesktopBootstrapApprovalResult as PeerFinished, DesktopBootstrapExchangeResult as Exchange,
    DesktopOwnerSecurityOperationInput as Input, DesktopOwnerSecurityOperationPrepared as Prepared,
    DesktopOwnerSecurityOperationResult as Finished,
};
use cryptexd::local_signed_startup::{
    CONTENT_MARKER, ContentProfile, DURABLE_MARKER, LocalDaemon, NativeProfileStatus,
    Prepared as Startup,
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
    Retirement(
        Option<cryptexd::retirement_notice::Notice>,
        Reply<Option<cryptexd::retirement_notice::Notice>>,
    ),
    Bind([u8; 32], Reply<()>),
    Ping(Reply<()>),
    ProfileStatus(Reply<NativeProfileStatus>),
    Connections(
        Option<cryptexd::managed_profile::connection_settings::Update>,
        Reply<cryptexd::managed_profile::connection_settings::View>,
    ),
    Prepare(Input, Reply<Prepared>),
    Finish(String, bool, Reply<Finished>),
    Brain(Reply<String>),
    Transport(Reply<Exchange>),
    Offer(String, String, Reply<Exchange>),
    Proof(String, String, String, Reply<Exchange>),
    PreparePeer(String, Vec<String>, Reply<PeerPrepared>),
    FinishPeer(String, bool, Reply<PeerFinished>),
    CompletePeer(Vec<String>, Reply<PeerFinished>),
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
        let work = make(reply);
        let budget = Duration::from_secs(
            if matches!(&work, Work::Connections(..) | Work::Retirement(..)) {
                125
            } else {
                25
            },
        );
        self.sender
            .try_send(Queued {
                deadline: Instant::now() + budget,
                interested,
                work,
            })
            .map_err(|_| "candidate_queue_unavailable")?;
        receive
            .recv_timeout(budget)
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
    fn prepare_root(runtime: &Path, profile: ContentProfile) -> Result<(), String> {
        if profile == ContentProfile::ExistingProfile {
            cryptexd::paths::assert_outside_repository(runtime)
                .map_err(|_| "existing_profile_runtime")?;
            // The daemon alone reads the OS-backed signing key and verifies
            // canonical policy. Desktop never creates/adopts this profile.
            if !runtime.is_dir() {
                return Err("existing_profile_missing".into());
            }
            return Ok(());
        }
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
        if !matches!(
            profile,
            ContentProfile::KeychainResume | ContentProfile::TransportResume
        ) {
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
        Ok(())
    }
    fn start_profile(
        runtime: &Path,
        profile: ContentProfile,
    ) -> Result<(Self, cryptexd::ProtectedContent), String> {
        Self::prepare_root(runtime, profile)?;
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
                let mut bound = false;
                let mut last_profile_poll = Instant::now();
                loop {
                    check_pending_delivery(&mut daemon, &mut pending_delivery);
                    if stop_worker.load(Ordering::Acquire) {
                        let saved = if profile == ContentProfile::ExistingProfile {
                            Ok(())
                        } else {
                            let receipt = daemon
                                .fixture_status()
                                .map(|value| value.to_string())
                                .unwrap_or_else(|_| {
                                    "{\"sessionInvalidated\":true,\"outcome\":\"uncertain\"}".into()
                                });
                            std::fs::write(runtime.join("ui-receipt.json"), receipt)
                                .map_err(|_| "candidate_receipt_write".to_owned())
                        };
                        let cleanup = daemon
                            .shutdown(&runtime.join("stop"))
                            .map_err(|e| e.to_string());
                        let _ = done_send.send(cleanup.and(saved));
                        return;
                    }
                    let queued = match receiver.recv_timeout(Duration::from_millis(50)) {
                        Ok(work) => work,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if profile == ContentProfile::ExistingProfile
                                && bound
                                && last_profile_poll.elapsed() >= Duration::from_secs(2)
                            {
                                if daemon.profile_status().is_err() {
                                    daemon.invalidate_session();
                                }
                                last_profile_poll = Instant::now();
                            }
                            continue;
                        }
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
                            bound = result.is_ok();
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::ProfileStatus(reply) => {
                            let result = daemon.profile_status().map_err(|e| e.to_string());
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::Retirement(notice, reply) => {
                            pending_delivery = None;
                            let result = daemon
                                .retirement_notice(notice)
                                .map_err(|_| "notice_unavailable".to_string());
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::Connections(update, reply) => {
                            if update.is_some() {
                                pending_delivery = None;
                            }
                            let result = daemon
                                .connection_settings(update)
                                .map_err(|_| "connection_settings_unavailable".to_string());
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
                        Work::Brain(reply) => {
                            let result = daemon
                                .read_pairing_brain_identity()
                                .map_err(|e| e.to_string());
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::Transport(reply) => {
                            let result = daemon
                                .read_pairing_transport_identity()
                                .map_err(|e| e.to_string());
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::Offer(transaction, transport, reply) => {
                            let result = daemon
                                .create_pairing_offer(transaction, transport)
                                .map_err(|e| e.to_string());
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::Proof(offer, brain, transport, reply) => {
                            let result = daemon
                                .create_pairing_proof(offer, brain, transport)
                                .map_err(|e| e.to_string());
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::PreparePeer(role, messages, reply) => {
                            let result = daemon
                                .prepare_peer(role, messages)
                                .map_err(|e| e.to_string());
                            if result.is_ok() {
                                pending_delivery = Some(delivery.clone());
                            }
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::FinishPeer(handle, confirm, reply) => {
                            let result = daemon
                                .finish_peer(&handle, confirm)
                                .map_err(|e| e.to_string());
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::CompletePeer(messages, reply) => {
                            let result =
                                daemon.complete_pairing(messages).map_err(|e| e.to_string());
                            deliver(&mut daemon, &delivery, reply, result);
                        }
                        Work::Exercise(reply) => {
                            let result = daemon
                                .exercise_pairing_boundary()
                                .and_then(|_| daemon.exercise_synthetic_owner_sessions())
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
async fn m8_native_profile_status(
    state: tauri::State<'_, Client>,
) -> Result<NativeProfileStatus, String> {
    async_call(state.inner().clone(), Work::ProfileStatus).await
}

#[tauri::command]
async fn m8_retirement_file(state: tauri::State<'_, Client>, import: bool) -> Result<bool, String> {
    let client = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::retirement_file::choose(import, |notice| {
            client.call(|reply| Work::Retirement(notice, reply))
        })
    })
    .await
    .map_err(|_| "notice_worker")?
}

#[tauri::command]
async fn m8_connection_settings(
    state: tauri::State<'_, Client>,
    update: Option<cryptexd::managed_profile::connection_settings::Update>,
) -> Result<cryptexd::managed_profile::connection_settings::View, String> {
    async_call(state.inner().clone(), move |reply| {
        Work::Connections(update, reply)
    })
    .await
}
#[tauri::command]
fn m8_new_bootstrap_transaction_id() -> Result<String, String> {
    cryptexd::ipc::desktop_new_bootstrap_transaction_id().map_err(|e| e.to_string())
}
#[tauri::command]
async fn m8_read_bootstrap_brain_identity(
    state: tauri::State<'_, Client>,
) -> Result<String, String> {
    async_call(state.inner().clone(), Work::Brain).await
}
#[tauri::command]
async fn m8_read_transport_device_id(state: tauri::State<'_, Client>) -> Result<String, String> {
    let result = async_call(state.inner().clone(), Work::Transport).await?;
    if !result.accepted {
        return Err(result
            .error
            .unwrap_or("transport_identity_unavailable".into()));
    }
    result
        .exchange_message
        .ok_or("transport_identity_unavailable".into())
}
#[tauri::command]
async fn m8_create_bootstrap_offer(
    state: tauri::State<'_, Client>,
    transaction_id: String,
    transport_device_id: String,
) -> Result<Exchange, String> {
    async_call(state.inner().clone(), move |reply| {
        Work::Offer(transaction_id, transport_device_id, reply)
    })
    .await
}
#[tauri::command]
async fn m8_create_bootstrap_proof(
    state: tauri::State<'_, Client>,
    offer_message: String,
    brain_identity_json: String,
    transport_device_id: String,
) -> Result<Exchange, String> {
    async_call(state.inner().clone(), move |reply| {
        Work::Proof(
            offer_message,
            brain_identity_json,
            transport_device_id,
            reply,
        )
    })
    .await
}
#[tauri::command]
async fn m8_prepare_peer_approval(
    state: tauri::State<'_, Client>,
    role: String,
    transcript_messages: Vec<String>,
) -> Result<PeerPrepared, String> {
    async_call(state.inner().clone(), move |reply| {
        Work::PreparePeer(role, transcript_messages, reply)
    })
    .await
}
#[tauri::command]
async fn m8_confirm_peer_approval(
    state: tauri::State<'_, Client>,
    session_handle: String,
) -> Result<PeerFinished, String> {
    async_call(state.inner().clone(), move |reply| {
        Work::FinishPeer(session_handle, true, reply)
    })
    .await
}
#[tauri::command]
async fn m8_cancel_peer_approval(
    state: tauri::State<'_, Client>,
    session_handle: String,
) -> Result<PeerFinished, String> {
    async_call(state.inner().clone(), move |reply| {
        Work::FinishPeer(session_handle, false, reply)
    })
    .await
}
#[tauri::command]
async fn m8_complete_peer_approval(
    state: tauri::State<'_, Client>,
    transcript_messages: Vec<String>,
) -> Result<PeerFinished, String> {
    async_call(state.inner().clone(), move |reply| {
        Work::CompletePeer(transcript_messages, reply)
    })
    .await
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
#[cfg(not(feature = "m8-development-product"))]
fn p0_5_set_capture_session_guard() {}
#[cfg(feature = "m8-development-product")]
struct DevelopmentDraft(AtomicBool);
#[cfg(feature = "m8-development-product")]
impl Default for DevelopmentDraft {
    fn default() -> Self {
        // Until the verified renderer acknowledges its draft state, ask before
        // quitting rather than assume that no unsaved input exists.
        Self(AtomicBool::new(true))
    }
}
#[cfg(feature = "m8-development-product")]
#[tauri::command]
fn p0_5_set_capture_session_guard(
    dirty: bool,
    sync_failed: Option<bool>,
    state: tauri::State<'_, DevelopmentDraft>,
) {
    state
        .0
        .store(dirty || sync_failed.unwrap_or(false), Ordering::Release);
}
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
        || context.config().identifier != super::protected_assets::APP_IDENTIFIER
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

fn durable_daemon(root: &Path, profile: ContentProfile) -> Result<LocalDaemon, String> {
    Owner::prepare_root(root, profile)?;
    let prepared =
        Startup::current_profile(Duration::from_secs(15), profile).map_err(|e| e.to_string())?;
    let path = prepared.daemon_path();
    let mut command = super::daemon_hold_command(path.to_str().ok_or("candidate_path")?, root)?;
    command.stderr(std::process::Stdio::inherit());
    let mut daemon = prepared
        .launch(command, &root.join("ready"))
        .map_err(|e| e.to_string())?;
    let content = daemon.content().map_err(|e| e.to_string())?;
    let (_context, binding) = install_context(content, root)?;
    daemon.bind_content(&binding).map_err(|e| e.to_string())?;
    Ok(daemon)
}

fn check_durable_restart(root: &Path) -> Result<(), String> {
    // All replay bytes stay on this one native owner thread, including across
    // process restart. They are never renderer input or persisted to disk.
    let mut daemon = durable_daemon(root, ContentProfile::KeychainCreate)?;
    let seed = durable_snapshot(root)?;
    let pending = daemon
        .prepare_owner(self_retirement())
        .map_err(|e| e.to_string())?;
    let pending_probe = daemon
        .capture_confirmation_probe()
        .map_err(|e| e.to_string())?;
    daemon.terminate_for_restart().map_err(|e| e.to_string())?;
    daemon
        .require_stale_rejection()
        .map_err(|e| e.to_string())?;
    daemon.close().map_err(|e| e.to_string())?;

    let mut resumed = durable_daemon(root, ContentProfile::KeychainResume)?;
    if durable_snapshot(root)? != seed {
        return Err("candidate_restart_mutated_seed".to_owned());
    }
    if resumed.finish_owner(&pending.session_handle, true).is_ok() {
        return Err("candidate_old_handle_accepted".to_owned());
    }
    let journal_before =
        std::fs::read(root.join("bootstrap/pending.json")).map_err(|_| "candidate_snapshot")?;
    resumed
        .require_wire_replay_refusal(&pending_probe)
        .map_err(|e| e.to_string())?;
    if durable_snapshot(root)? != seed
        || std::fs::read(root.join("bootstrap/pending.json")).map_err(|_| "candidate_snapshot")?
            != journal_before
    {
        return Err("candidate_pending_replay_mutated_history".to_owned());
    }
    let confirmed_probe = resumed
        .exercise_and_capture_confirmation()
        .map_err(|e| e.to_string())?;
    resumed
        .shutdown(&root.join("stop"))
        .map_err(|e| e.to_string())?;
    let confirmed = durable_snapshot(root)?;
    let mut reopened = durable_daemon(root, ContentProfile::KeychainResume)?;
    let journal_confirmed =
        std::fs::read(root.join("bootstrap/pending.json")).map_err(|_| "candidate_snapshot")?;
    for probe in [&pending_probe, &confirmed_probe, &confirmed_probe] {
        reopened
            .require_wire_replay_refusal(probe)
            .map_err(|e| e.to_string())?;
        let status = reopened.fixture_status().map_err(|e| e.to_string())?;
        if status["eventCount"] != 2
            || status["prefixPreserved"] != true
            || durable_snapshot(root)? != confirmed
            || std::fs::read(root.join("bootstrap/pending.json"))
                .map_err(|_| "candidate_snapshot")?
                != journal_confirmed
        {
            return Err("candidate_replay_mutated_confirmation".to_owned());
        }
    }
    let status = reopened.fixture_status().map_err(|e| e.to_string())?;
    std::fs::write(root.join("ui-receipt.json"), status.to_string())
        .map_err(|_| "candidate_receipt_write")?;
    reopened
        .shutdown(&root.join("stop"))
        .map_err(|e| e.to_string())?;
    println!("native_keychain_same_profile_restart=passed");
    println!("native_keychain_confirmed_history_preserved=passed");
    println!("native_keychain_old_local_handle_refused=passed");
    println!("native_keychain_cross_daemon_wire_replay_refused=passed");
    Ok(())
}

fn pairing_transport(daemon: &mut LocalDaemon) -> Result<String, String> {
    let until = Instant::now() + Duration::from_secs(45);
    loop {
        let reply = daemon
            .read_pairing_transport_identity()
            .map_err(|e| e.to_string())?;
        if reply.accepted {
            return reply
                .exchange_message
                .ok_or("candidate_transport_shape".into());
        }
        if reply.error.as_deref() != Some("transport_identity_unavailable")
            || Instant::now() >= until
        {
            return Err("candidate_transport_unavailable".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn canonical_event_snapshot(
    root: &Path,
) -> Result<std::collections::BTreeMap<PathBuf, Vec<u8>>, String> {
    Ok(durable_snapshot(root)?
        .into_iter()
        .filter(|(path, _)| {
            path.starts_with(root.join("canonical/events"))
                && path
                    .extension()
                    .is_some_and(|ext| ext == "seg" || ext == "open")
        })
        .collect())
}
fn check_existing_profile_import(
    source: &Path,
    receiver: &Path,
    mut daemon: LocalDaemon,
) -> Result<(), String> {
    use cryptexd::managed_profile::connection_settings::{State, Update};
    use std::os::unix::fs::DirBuilderExt;
    let before_settings = canonical_event_snapshot(receiver)?;
    let settings = daemon
        .connection_settings(None)
        .map_err(|e| e.to_string())?;
    if settings.state != State::Paused || settings.devices.len() != 1 {
        return Err("existing_profile_settings_initial".into());
    }
    let pending = daemon
        .prepare_owner(self_retirement())
        .map_err(|e| e.to_string())?;
    let stale = daemon
        .capture_confirmation_probe()
        .map_err(|e| e.to_string())?;
    let saved = daemon
        .connection_settings(Some(Update {
            expected_revision: settings.revision,
            paused: true,
            listen_address: None,
            peers: Vec::new(),
        }))
        .map_err(|e| e.to_string())?;
    if saved.state != State::Paused {
        return Err("existing_profile_settings_save".into());
    }
    daemon
        .require_wire_replay_refusal(&stale)
        .map_err(|e| e.to_string())?;
    if daemon.finish_owner(&pending.session_handle, true).is_ok()
        || canonical_event_snapshot(receiver)? != before_settings
    {
        return Err("existing_profile_settings_old_approval".into());
    }
    let source_before = canonical_event_snapshot(source)?;
    let brain = cryptexd::identity_runtime::load_brain_identity(source)
        .map_err(|_| "existing_profile_source")?;
    let local =
        cryptexd::local_device_identity::load_local_device_identity(source, &brain.brain_id)
            .map_err(|_| "existing_profile_source")?;
    let device = &local.device_id;
    let stem = device
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let relative = PathBuf::from(stem).join("00000000000000000000.seg");
    let incoming = receiver.join("transport/incoming").join(&relative);
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(incoming.parent().ok_or("existing_profile_incoming")?)
        .map_err(|_| "existing_profile_incoming")?;
    // Deliberate untrusted transport test bytes only. Desktop never writes or
    // seals a canonical event; the native daemon must authenticate this batch.
    std::fs::copy(source.join("canonical/events").join(&relative), &incoming)
        .map_err(|_| "existing_profile_incoming")?;
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        let status = daemon.profile_status().map_err(|e| e.to_string())?;
        if status.remote_durability_verified {
            return Err("existing_profile_false_protection".into());
        }
        if status.incoming == cryptexd::local_signed_startup::NativeIncomingState::Published {
            break;
        }
        if !matches!(
            status.incoming,
            cryptexd::local_signed_startup::NativeIncomingState::WaitingForTransport
                | cryptexd::local_signed_startup::NativeIncomingState::Deferred
                | cryptexd::local_signed_startup::NativeIncomingState::Idle
        ) || Instant::now() >= deadline
        {
            return Err(format!("existing_profile_incoming_{:?}", status.incoming));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let destination = receiver.join("canonical/events").join(relative);
    if std::fs::read(&destination).map_err(|_| "existing_profile_publication")?
        != std::fs::read(&incoming).map_err(|_| "existing_profile_incoming")?
    {
        return Err("existing_profile_publication_mismatch".into());
    }
    let restriction = daemon
        .prepare_owner(Input {
            kind: "device_restriction".into(),
            target_device_id: Some(local.device_id),
            reason: Some("compromised".into()),
            cutoff_seq: Some(1),
        })
        .map_err(|e| e.to_string())?;
    if !daemon
        .finish_owner(&restriction.session_handle, true)
        .map_err(|e| e.to_string())?
        .accepted
    {
        return Err("existing_profile_live_restriction".into());
    }
    let status = daemon.profile_status().map_err(|e| e.to_string())?;
    if status.remote_durability_verified
        || matches!(
            status.incoming,
            cryptexd::local_signed_startup::NativeIncomingState::Held
                | cryptexd::local_signed_startup::NativeIncomingState::Unavailable
        )
    {
        return Err("existing_profile_restriction_not_reconciled".into());
    }
    let restricted = daemon
        .connection_settings(None)
        .map_err(|e| e.to_string())?;
    if restricted.state != State::Paused
        || !restricted.devices.is_empty()
        || restricted.revision != saved.revision
    {
        eprintln!(
            "component=native_settings_check stage=restricted_peer state={:?} error={:?} devices={} revision_equal={}",
            restricted.state,
            restricted.error,
            restricted.devices.len(),
            restricted.revision == saved.revision
        );
        return Err("existing_profile_settings_restricted_peer".into());
    }
    let prepared = daemon
        .prepare_owner(self_retirement())
        .map_err(|e| e.to_string())?;
    let old = daemon
        .capture_confirmation_probe()
        .map_err(|e| e.to_string())?;
    daemon.close().map_err(|e| e.to_string())?;
    let before_restart = canonical_event_snapshot(receiver)?;
    let mut reopened = durable_daemon(receiver, ContentProfile::ExistingProfile)?;
    let settings = reopened
        .connection_settings(None)
        .map_err(|e| e.to_string())?;
    if settings.state != State::Paused
        || !settings.devices.is_empty()
        || settings.revision != saved.revision
    {
        return Err("existing_profile_settings_restart".into());
    }
    reopened
        .require_wire_replay_refusal(&old)
        .map_err(|e| e.to_string())?;
    if reopened
        .finish_owner(&prepared.session_handle, true)
        .is_ok()
    {
        return Err("existing_profile_old_handle_accepted".into());
    }
    reopened.profile_status().map_err(|e| e.to_string())?;
    reopened.close().map_err(|e| e.to_string())?;
    if canonical_event_snapshot(receiver)? != before_restart
        || canonical_event_snapshot(source)? != source_before
    {
        return Err("existing_profile_restart_changed_events".into());
    }
    println!("native_existing_profile_signed_import=passed");
    println!("native_existing_profile_restart_replay_and_local_status=passed");
    println!("native_connection_settings_owner_channel_and_roster=passed");
    Ok(())
}

fn signed_pairing_messages(
    first: &mut LocalDaemon,
    second: &mut LocalDaemon,
    id_a: String,
    id_b: String,
) -> Result<Vec<String>, String> {
    let brain = first
        .read_pairing_brain_identity()
        .map_err(|e| e.to_string())?;
    if second
        .read_pairing_brain_identity()
        .map_err(|e| e.to_string())?
        != brain
    {
        return Err("candidate_pairing_brain_mismatch".into());
    }
    let txn = cryptexd::ipc::desktop_new_bootstrap_transaction_id().map_err(|e| e.to_string())?;
    let offer = first
        .create_pairing_offer(txn, id_a.clone())
        .map_err(|e| e.to_string())?;
    if !offer.accepted {
        return Err("candidate_offer_refused".into());
    }
    let offer = offer.exchange_message.ok_or("candidate_offer_missing")?;
    let proof = second
        .create_pairing_proof(offer.clone(), brain, id_b.clone())
        .map_err(|e| e.to_string())?;
    if !proof.accepted {
        return Err("candidate_proof_refused".into());
    }
    let mut messages = vec![
        offer,
        proof.exchange_message.ok_or("candidate_proof_missing")?,
    ];
    let pending = first
        .prepare_peer("initiator".into(), messages.clone())
        .map_err(|e| e.to_string())?;
    let approval = first
        .finish_peer(&pending.session_handle, true)
        .map_err(|e| e.to_string())?;
    if !approval.accepted {
        return Err("candidate_initiator_refused".into());
    }
    messages.push(
        approval
            .signed_message
            .ok_or("candidate_approval_missing")?,
    );
    let pending = second
        .prepare_peer("responder".into(), messages.clone())
        .map_err(|e| e.to_string())?;
    let approval = second
        .finish_peer(&pending.session_handle, true)
        .map_err(|e| e.to_string())?;
    if !approval.accepted {
        return Err("candidate_responder_refused".into());
    }
    messages.push(
        approval
            .signed_message
            .ok_or("candidate_approval_missing")?,
    );
    Ok(messages)
}

/// Actual signed Desktop/native daemon pairs; two isolated local engines.
/// This fixture proves pairing and reopen, not cross-device data replication.
fn check_transport_pairing(root: &Path) -> Result<(), String> {
    Owner::prepare_root(root, ContentProfile::Ephemeral)?;
    let a = root.join("initiator");
    let b = root.join("responder");
    let mut first = durable_daemon(&a, ContentProfile::TransportCreate)?;
    let mut second = durable_daemon(&b, ContentProfile::TransportCreate)?;
    let id_a = pairing_transport(&mut first)?;
    let id_b = pairing_transport(&mut second)?;
    if id_a == id_b {
        return Err("candidate_transport_identity_collision".into());
    }
    let messages = signed_pairing_messages(&mut first, &mut second, id_a.clone(), id_b.clone())?;
    for daemon in [&mut first, &mut second] {
        if !daemon
            .complete_pairing(messages.clone())
            .map_err(|e| e.to_string())?
            .accepted
        {
            return Err("candidate_pairing_not_recorded".into());
        }
        let status = daemon.fixture_status().map_err(|e| e.to_string())?;
        if status["eventCount"] != 2 || status["prefixPreserved"] != true {
            return Err("candidate_pairing_history".into());
        }
    }
    first.close().map_err(|e| e.to_string())?;
    second.close().map_err(|e| e.to_string())?;
    eprintln!("native_managed_pairing_clean_shutdown=passed");
    for (path, identity) in [(&a, id_a), (&b, id_b)] {
        let before = durable_snapshot(path)?;
        let mut reopened = durable_daemon(path, ContentProfile::TransportResume)?;
        if pairing_transport(&mut reopened)? != identity
            || !reopened
                .complete_pairing(messages.clone())
                .map_err(|e| e.to_string())?
                .accepted
            || durable_snapshot(path)? != before
        {
            return Err("candidate_pairing_reopen_changed_history".into());
        }
        reopened.close().map_err(|e| e.to_string())?;
    }
    let mut native_a = durable_daemon(&a, ContentProfile::ExistingProfile)?;
    let mut native_b = durable_daemon(&b, ContentProfile::ExistingProfile)?;
    let native_id_a = pairing_transport(&mut native_a)?;
    let native_id_b = pairing_transport(&mut native_b)?;
    let messages = signed_pairing_messages(&mut native_a, &mut native_b, native_id_a, native_id_b)?;
    for daemon in [&mut native_a, &mut native_b] {
        if !daemon
            .complete_pairing(messages.clone())
            .map_err(|e| e.to_string())?
            .accepted
        {
            return Err("existing_profile_live_pairing".into());
        }
    }
    native_a.close().map_err(|e| e.to_string())?;
    check_existing_profile_import(&a, &b, native_b)?;
    println!("native_managed_transport_pairing=passed");
    println!("native_managed_pairing_restart_idempotent=passed");
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
        if args.len() == 2 && args[0] == "--cryptex-m8-local-content-transport-check" {
            return check_transport_pairing(Path::new(&args[1]));
        }
        if args.len() == 2 && args[0] == "--cryptex-m8-local-content-durable-check" {
            return check_durable_restart(Path::new(&args[1]));
        }
        if args.len() == 2 && args[0] == "--cryptex-m8-local-content-durable-resume-check" {
            return durable_owner(Path::new(&args[1]), ContentProfile::KeychainResume)?.close();
        }
        if args.len() != 2
            || (args[0] != "--cryptex-m8-local-content-check"
                && args[0] != "--cryptex-m8-local-content-ui"
                && args[0] != "--cryptex-m8-local-content-existing-profile-ui")
        {
            return Err("candidate_arguments".into());
        }
        let existing_profile = args[0] == "--cryptex-m8-local-content-existing-profile-ui";
        let runtime = if existing_profile {
            PathBuf::from(&args[1])
                .canonicalize()
                .map_err(|_| "existing_profile_missing")?
        } else {
            PathBuf::from(&args[1])
        };
        let (owner, content) = if existing_profile {
            Owner::start_profile(&runtime, ContentProfile::ExistingProfile)?
        } else {
            Owner::start(&runtime)?
        };
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
        window.title = "Cryptex Ã‚Â· Ã„Â°zole onay testi".into();
        if existing_profile {
            window.title = "Cryptex - Existing profile".into();
        }
        context.config_mut().app.windows.clear();
        let app = tauri::Builder::default()
            .manage(owner.client.clone())
            .invoke_handler(tauri::generate_handler![
                m8_native_profile_status,
                m8_new_bootstrap_transaction_id,
                m8_read_bootstrap_brain_identity,
                m8_read_transport_device_id,
                m8_connection_settings,
                m8_retirement_file,
                m8_create_bootstrap_offer,
                m8_create_bootstrap_proof,
                m8_prepare_peer_approval,
                m8_confirm_peer_approval,
                m8_cancel_peer_approval,
                m8_complete_peer_approval,
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

#[cfg(feature = "m8-development-product")]
#[path = "development_ui.rs"]
mod development_ui;

/// Explicit acceptance executable only. Reuses the normal ExistingProfile
/// startup, protected content and binding; adds no renderer command or trust.
#[cfg(all(
    feature = "m8-development-product",
    feature = "m8-device-recovery-check"
))]
pub(super) fn recovery_start(runtime: &Path) -> Result<LocalDaemon, String> {
    Owner::prepare_root(runtime, ContentProfile::ExistingProfile)?;
    let prepared =
        Startup::current_profile(Duration::from_secs(15), ContentProfile::ExistingProfile)
            .map_err(|_| "recovery_native_prepare")?;
    let path = prepared.daemon_path();
    let command =
        super::daemon_hold_command(path.to_str().ok_or("recovery_daemon_path")?, runtime)?;
    let mut daemon = prepared
        .launch(command, &runtime.join("ready"))
        .map_err(|_| "recovery_native_start")?;
    let content = daemon.content().map_err(|_| "recovery_content")?;
    let (_, binding) = install_context(content, runtime)?;
    daemon
        .bind_content(&binding)
        .map_err(|_| "recovery_content_binding")?;
    daemon.ping().map_err(|_| "recovery_native_ping")?;
    daemon
        .profile_status()
        .map_err(|_| "recovery_profile_status")?;
    Ok(daemon)
}

/// The selected development build always owns normal entry. No failure falls
/// through to fixture commands, environment daemon overrides or legacy startup.
#[cfg(feature = "m8-development-product")]
pub fn development_entry() -> Option<Result<(), String>> {
    Some((|| {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        match super::development_launch::mode(&args)? {
            super::development_launch::Mode::Window => {
                let runtime =
                    super::development_launch::runtime(std::env::var_os("HOME").as_deref())?;
                development_ui::run(runtime)
            }
            super::development_launch::Mode::WindowCheck(runtime) => {
                let runtime = super::development_launch::window_check_runtime(&runtime)?;
                development_ui::run(runtime)
            }
            super::development_launch::Mode::Check(runtime) => {
                // Keep the noninteractive diagnostic and its native budget intact.
                let (owner, content) = development_ui::stage(
                    "start",
                    Owner::start_profile(&runtime, ContentProfile::ExistingProfile),
                )?;
                let checked = (|| {
                    let (_, binding) =
                        development_ui::stage("content", install_context(content, &runtime))?;
                    development_ui::stage(
                        "bind",
                        owner.client.call(|reply| Work::Bind(binding, reply)),
                    )?;
                    development_ui::stage("ping", owner.client.call(Work::Ping))?;
                    development_ui::stage(
                        "profile_status",
                        owner.client.call(Work::ProfileStatus),
                    )?;
                    development_ui::stage("unix_health", super::p0_3_ipc_health())
                })();
                // Even a failed check must observe, rather than discard, cleanup.
                development_ui::stage("close", owner.close())?;
                checked?;
                println!("development_existing_profile_startup_and_cleanup=passed");
                Ok(())
            }
        }
    })())
}
