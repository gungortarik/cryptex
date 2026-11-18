//! Default-off, native Desktop acceptance driver. No renderer command, arbitrary
//! text, runtime override, PID adoption, canonical write or trust mutation.
//! A queued-work crash is NOT evidence of interruption in the middle of a byte.
use cryptexd::ipc::{self, DesktopCaptureBody, DesktopCaptureResolveBody, DesktopSearchQuery};
use cryptexd::managed_profile::connection_settings::{PeerAddress, State, Update, View};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const FLAG: &str = "--cryptex-device-recovery-check";
#[cfg(target_os = "macos")]
const CUT_FLAG: &str = "--cryptex-device-recovery-desktop-loss";
const IPC: Duration = Duration::from_secs(3);

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Failure {
    Arguments,
    ExistingProfileRequired,
    Startup,
    Identity,
    Connections,
    CaptureUncertain,
    Search,
    PeerDeadline,
    Interruption,
    Cleanup,
}
type Result<T> = std::result::Result<T, Failure>;

#[derive(Debug)]
struct Plan {
    run: String,
    brain: String,
    local: String,
    peer: String,
}
fn uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
        && value != "00000000-0000-0000-0000-000000000000"
}
impl Plan {
    fn parse(args: &[String]) -> Result<Self> {
        let [flag, run, brain, local, peer] = args else {
            return Err(Failure::Arguments);
        };
        if flag != FLAG || ![run, brain, local, peer].iter().all(|v| uuid(v)) || local == peer {
            return Err(Failure::Arguments);
        }
        Ok(Self {
            run: run.clone(),
            brain: brain.clone(),
            local: local.clone(),
            peer: peer.clone(),
        })
    }
    fn marker(&self, device: &str, ack: bool) -> String {
        format!(
            "M8 recovery {} {} {}",
            self.run,
            device,
            if ack { "received" } else { "queued" }
        )
    }
    fn capture_id(&self, ack: bool) -> String {
        // Stable per-device receipt IDs. The journal already scopes IDs by
        // device. This is an identifier transformation, not cryptography.
        let mut id = self.run.clone();
        if ack {
            let last = u8::from_str_radix(&id[34..], 16).expect("validated UUID");
            id.replace_range(34.., &format!("{:02x}", last ^ 1));
        }
        id
    }
    fn check_view(&self, view: &View) -> Result<()> {
        if view.error.is_some()
            || view.listen_address.is_none()
            || view.local_transport_port.is_none_or(|p| p == 0)
            || view.devices.len() != 1
            || view.devices[0].device_id != self.peer
            || view.devices[0].endpoint.is_none()
            || !matches!(view.state, State::Paused | State::ReadyLocally)
        {
            return Err(Failure::Connections);
        }
        Ok(())
    }
}

struct Session {
    root: PathBuf,
    pid: u32,
    #[cfg(windows)]
    child: Option<std::process::Child>,
    #[cfg(all(target_os = "macos", feature = "m8-development-product"))]
    native: Option<cryptexd::local_signed_startup::LocalDaemon>,
}
impl Session {
    fn start(root: &Path, plan: &Plan) -> Result<Self> {
        if !root.is_dir() {
            return Err(Failure::ExistingProfileRequired);
        }
        #[cfg(windows)]
        let session = {
            let path = std::env::current_exe().map_err(|_| Failure::Startup)?;
            let daemon = path.parent().ok_or(Failure::Startup)?.join("cryptexd.exe");
            let (pid, child) =
                super::start_daemon_inner(daemon.to_str().ok_or(Failure::Startup)?, root)
                    .map_err(|_| Failure::Startup)?;
            Self {
                root: root.into(),
                pid,
                child: Some(child),
            }
        };
        #[cfg(all(target_os = "macos", feature = "m8-development-product"))]
        let mut session = {
            let native = super::candidate_ui::recovery_start(root).map_err(|_| Failure::Startup)?;
            let pid = native.owned_child_id().ok_or(Failure::Startup)?;
            Self {
                root: root.into(),
                pid,
                native: Some(native),
            }
        };
        #[cfg(not(any(windows, all(target_os = "macos", feature = "m8-development-product"))))]
        compile_error!("device recovery requires Windows or the signed Mac development product");
        let health = ipc::desktop_health_ping(IPC).map_err(|_| Failure::Identity)?;
        if !health.accepted || health.error.is_some() || health.daemon_pid != session.pid {
            return Err(Failure::Identity);
        }
        #[cfg(windows)]
        let raw = ipc::desktop_read_bootstrap_brain_identity(IPC).map_err(|_| Failure::Identity)?;
        #[cfg(all(target_os = "macos", feature = "m8-development-product"))]
        let raw = session
            .native
            .as_mut()
            .ok_or(Failure::Identity)?
            .read_pairing_brain_identity()
            .map_err(|_| Failure::Identity)?;
        let brain: serde_json::Value = serde_json::from_str(&raw).map_err(|_| Failure::Identity)?;
        if brain.get("brain_id").and_then(|v| v.as_str()) != Some(&plan.brain) {
            return Err(Failure::Identity);
        }
        Ok(session)
    }
    fn connections(&mut self, update: Option<Update>) -> Result<View> {
        #[cfg(windows)]
        {
            ipc::desktop_connection_settings(update).map_err(|_| Failure::Connections)
        }
        #[cfg(all(target_os = "macos", feature = "m8-development-product"))]
        {
            self.native
                .as_mut()
                .ok_or(Failure::Connections)?
                .connection_settings(update)
                .map_err(|_| Failure::Connections)
        }
    }
    fn pause(&mut self, plan: &Plan, paused: bool) -> Result<()> {
        let view = self.connections(None)?;
        plan.check_view(&view)?;
        let update = Update {
            expected_revision: view.revision,
            paused,
            listen_address: view.listen_address,
            peers: view
                .devices
                .into_iter()
                .map(|d| PeerAddress {
                    device_id: d.device_id,
                    endpoint: d.endpoint.expect("validated endpoint"),
                })
                .collect(),
        };
        let applied = self.connections(Some(update))?;
        plan.check_view(&applied)?;
        if applied.paused != paused
            || applied.state
                != if paused {
                    State::Paused
                } else {
                    State::ReadyLocally
                }
        {
            return Err(Failure::Connections);
        }
        Ok(())
    }
    fn capture(&self, plan: &Plan, ack: bool) -> Result<()> {
        let id = plan.capture_id(ack);
        let response = ipc::desktop_capture(
            DesktopCaptureBody {
                capture_id: id.clone(),
                text: plan.marker(&plan.local, ack),
                classification: "public".into(),
            },
            IPC,
        );
        // Never repeat an append on timeout/uncertainty. The existing durable
        // receipt resolver is the only fallback and is itself read-only.
        let response = match response {
            Ok(r) if r.accepted && r.outcome.as_deref() == Some("committed") => r,
            _ => ipc::desktop_capture_resolve(
                DesktopCaptureResolveBody {
                    capture_id: id.clone(),
                },
                IPC,
            )
            .map_err(|_| Failure::CaptureUncertain)?,
        };
        if !response.accepted
            || response.error.is_some()
            || response.daemon_pid != self.pid
            || response.outcome.as_deref() != Some("committed")
            || response.capture_id.as_deref() != Some(&id)
            || response.classification.as_deref() != Some("public")
        {
            return Err(Failure::CaptureUncertain);
        }
        Ok(())
    }
    fn heartbeat(&mut self) -> Result<()> {
        #[cfg(all(target_os = "macos", feature = "m8-development-product"))]
        self.native
            .as_mut()
            .ok_or(Failure::Identity)?
            .profile_status()
            .map_err(|_| Failure::Identity)?;
        let health = ipc::desktop_health_ping(IPC).map_err(|_| Failure::Identity)?;
        if !health.accepted || health.error.is_some() || health.daemon_pid != self.pid {
            return Err(Failure::Identity);
        }
        Ok(())
    }
    fn found(&mut self, plan: &Plan, device: &str, ack: bool) -> Result<bool> {
        self.heartbeat()?;
        let marker = plan.marker(device, ack);
        let result = ipc::desktop_search(
            DesktopSearchQuery {
                text: Some(marker.clone()),
                classification: Some("public".into()),
                device_id: Some(device.into()),
                limit: Some(20),
                ..Default::default()
            },
            IPC,
        )
        .map_err(|_| Failure::Search)?;
        search_observation(result, self.pid, plan, device, &marker)
    }
    fn wait_found(&mut self, plan: &Plan, device: &str, ack: bool, seconds: u64) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            if self.found(plan, device, ack)? {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Failure::PeerDeadline);
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }
    #[cfg(windows)]
    fn interrupt(mut self) -> Result<()> {
        {
            let child = self.child.as_mut().ok_or(Failure::Interruption)?;
            // Retained Child only. Never accept a process name, PID or status
            // response as authority to terminate a process.
            cryptexd::daemon_startup::interrupt_owned_child(child)
                .map_err(|_| Failure::Interruption)?;
            self.child.take();
        }
        Ok(())
    }
    fn close(mut self) -> Result<()> {
        self.close_inner()
    }
    fn close_inner(&mut self) -> Result<()> {
        #[cfg(windows)]
        if let Some(child) = self.child.as_mut() {
            use std::io::Write;
            let mut stop = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.root.join("stop"))
                .map_err(|_| Failure::Cleanup)?;
            stop.write_all(b"stop\n")
                .and_then(|()| stop.sync_all())
                .map_err(|_| Failure::Cleanup)?;
            let deadline = Instant::now() + Duration::from_secs(45);
            loop {
                if let Some(status) = child.try_wait().map_err(|_| Failure::Cleanup)? {
                    self.child.take();
                    return if status.success() {
                        Ok(())
                    } else {
                        Err(Failure::Cleanup)
                    };
                }
                if Instant::now() >= deadline {
                    return Err(Failure::Cleanup);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        #[cfg(all(target_os = "macos", feature = "m8-development-product"))]
        if let Some(native) = self.native.take() {
            native
                .shutdown(&self.root.join("stop"))
                .map_err(|_| Failure::Cleanup)?;
        }
        Ok(())
    }
}

fn search_observation(
    result: ipc::DesktopSearchResponse,
    pid: u32,
    plan: &Plan,
    device: &str,
    marker: &str,
) -> Result<bool> {
    if !result.accepted || result.error.is_some() || result.daemon_pid != pid {
        return Err(Failure::Search);
    }
    if let Some(reason) = result.hold_reason.as_deref() {
        // The ordinary route itself requests a derived rebuild for these
        // two states. Wait within the same deadline; never expose stale
        // hits or treat a policy/integrity hold as an empty result.
        return if matches!(
            reason,
            "materialization_state_missing_or_stale" | "projection_does_not_match_canonical"
        ) && result.hits.is_none()
            && !result.has_more
        {
            Ok(false)
        } else {
            Err(Failure::Search)
        };
    }
    let hits = result.hits.ok_or(Failure::Search)?;
    if hits.len() > 1 || result.has_more {
        return Err(Failure::Search);
    }
    match hits.first() {
        None => Ok(false),
        Some(hit)
            if hit.text == marker
                && hit.brain_id == plan.brain
                && hit.device_id == device
                && hit.classification == "public" =>
        {
            Ok(true)
        }
        _ => Err(Failure::Identity),
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if self.close_inner().is_err() {
            eprintln!("recovery_cleanup_unconfirmed");
        }
    }
}

fn stage(name: &str) {
    println!("{}", serde_json::json!({"recovery_stage":name}));
}
fn cut(plan: &Plan, root: &Path) -> Result<bool> {
    stage("start_existing_profile");
    let mut session = Session::start(root, plan)?;
    let initial = session.connections(None)?;
    plan.check_view(&initial)?;
    session.pause(plan, true)?;
    // Peer cannot receive this newly committed note while transport is paused.
    stage("capture_while_disconnected");
    session.capture(plan, false)?;
    session.wait_found(plan, &plan.local, false, 60)?;
    session.pause(plan, false)?;
    stage("interrupt_owned_process_after_start");
    #[cfg(windows)]
    {
        session.interrupt()?;
        Ok(initial.paused)
    }
    #[cfg(target_os = "macos")]
    {
        // Do not kill the Mac daemon: only that retained supervisor can prove
        // helper/group cleanup and remove its active marker. Abrupt Desktop
        // exit drops native rights in the OS, bypassing Session/LocalDaemon
        // Drop. The normal daemon observes owner loss and performs cleanup.
        use std::io::Write;
        std::io::stdout()
            .flush()
            .map_err(|_| Failure::Interruption)?;
        std::process::exit(if initial.paused { 77 } else { 75 });
    }
}

#[cfg(target_os = "macos")]
fn cut_desktop_child(plan: &Plan) -> Result<bool> {
    let mut child =
        std::process::Command::new(std::env::current_exe().map_err(|_| Failure::Startup)?)
            .args([CUT_FLAG, &plan.run, &plan.brain, &plan.local, &plan.peer])
            .spawn()
            .map_err(|_| Failure::Startup)?;
    let deadline = Instant::now() + Duration::from_secs(420);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.code() == Some(75) => return Ok(false),
            Ok(Some(status)) if status.code() == Some(77) => return Ok(true),
            Ok(Some(_)) => return Err(Failure::Interruption),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                // This exact Desktop child only. Leave the daemon alive long
                // enough to complete ordinary native owner-loss cleanup.
                let _ = child.kill();
                let _ = child.wait();
                std::thread::sleep(Duration::from_secs(40));
                return Err(Failure::Interruption);
            }
        }
    }
}

fn run(plan: &Plan, root: &Path) -> Result<()> {
    #[cfg(windows)]
    let originally_paused = cut(plan, root)?;
    #[cfg(target_os = "macos")]
    let originally_paused = cut_desktop_child(plan)?;
    // Give the existing helper's owner-loss cleanup its full budget. No marker
    // deletion, PID adoption, forced unlock or recovery repair is permitted.
    std::thread::sleep(Duration::from_secs(40));
    stage("restart_and_receive");
    let mut session = Session::start(root, plan)?;
    let restarted = session.connections(None)?;
    plan.check_view(&restarted)?;
    if restarted.paused {
        return Err(Failure::Connections);
    }
    session.wait_found(plan, &plan.local, false, 60)?;
    session.wait_found(plan, &plan.peer, false, 1200)?;
    // Fixed receipt note is ordinary Public memory, signed only by the daemon.
    // It proves the peer's queued note was searchable on this actual device.
    session.capture(plan, true)?;
    session.wait_found(plan, &plan.local, true, 60)?;
    session.wait_found(plan, &plan.peer, true, 1200)?;
    stage("allow_peer_receipt_delivery");
    // This is only delivery grace, never evidence of remote durability. Both
    // final daemon digests and all four notes must still match independently.
    for _ in 0..30 {
        session.heartbeat()?;
        std::thread::sleep(Duration::from_secs(2));
    }
    session.pause(plan, true)?;
    session.close()?;
    stage("offline_restart");
    let mut session = Session::start(root, plan)?;
    let paused = session.connections(None)?;
    plan.check_view(&paused)?;
    if !paused.paused || paused.state != State::Paused {
        return Err(Failure::Connections);
    }
    for device in [&plan.local, &plan.peer] {
        for ack in [false, true] {
            session.wait_found(plan, device, ack, 60)?;
        }
    }
    session.pause(plan, originally_paused)?;
    session.close()?;
    stage("completed");
    Ok(())
}

pub(super) fn entry() -> Option<std::result::Result<(), &'static str>> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    #[cfg(target_os = "macos")]
    let is_cut = args.first().is_some_and(|v| v == CUT_FLAG);
    #[cfg(target_os = "macos")]
    if is_cut {
        args[0] = FLAG.into();
    }
    #[cfg(windows)]
    let _ = &mut args;
    if args.first().is_none_or(|v| v != FLAG) {
        return None;
    }
    Some((|| {
        let plan = Plan::parse(&args).map_err(|_| "recovery_arguments")?;
        #[cfg(windows)]
        let root = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("recovery_home")?)
            .join("Cryptex-Dev/runtime/p0-2");
        #[cfg(target_os = "macos")]
        let root = super::development_launch::runtime(std::env::var_os("HOME").as_deref())?;
        #[cfg(target_os = "macos")]
        if is_cut {
            // A successful cut exits 75/77 inside cut(), without running drops;
            // the status carries only the original public pause preference.
            // On failure, normal Rust cleanup runs before this exit instead.
            let _ = cut(&plan, &root);
            std::process::exit(76);
        }
        let result = run(&plan, &root);
        println!(
            "{}",
            serde_json::json!({"schema":1,"purpose":"cryptex-device-recovery",
            "run_id":plan.run,"brain_id":plan.brain,"local_device_id":plan.local,"peer_device_id":plan.peer,
            "outcome":if result.is_ok(){"local_steps_passed"}else{"incomplete"},
            "interrupted_component":if cfg!(target_os="macos"){"desktop"}else{"daemon"},
            "error":result.as_ref().err(),"midstream_bytes_interrupted":false,
            "remote_durability_verified":false})
        );
        result.map_err(|_| "recovery_incomplete")
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args() -> Vec<String> {
        [
            FLAG,
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
            "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
        ]
        .map(String::from)
        .to_vec()
    }
    #[test]
    fn closed_arguments_refuse_paths_extra_controls_and_equal_identities() {
        let valid = args();
        assert!(Plan::parse(&valid).is_ok());
        for index in 0..5 {
            let mut invalid = valid.clone();
            invalid[index] = "../../runtime".into();
            assert!(Plan::parse(&invalid).is_err());
        }
        let mut invalid = valid.clone();
        invalid.push("--pid=123".into());
        assert!(Plan::parse(&invalid).is_err());
        let mut invalid = valid;
        invalid[4] = invalid[3].clone();
        assert!(Plan::parse(&invalid).is_err());
    }
    #[test]
    fn stable_capture_ids_and_markers_are_distinct_and_not_user_supplied() {
        let plan = Plan::parse(&args()).unwrap();
        assert!(uuid(&plan.capture_id(false)) && uuid(&plan.capture_id(true)));
        assert_ne!(plan.capture_id(false), plan.capture_id(true));
        assert_eq!(plan.capture_id(false), plan.run);
        let markers: std::collections::BTreeSet<_> = [&plan.local, &plan.peer]
            .iter()
            .flat_map(|id| [false, true].map(|ack| plan.marker(id, ack)))
            .collect();
        assert_eq!(markers.len(), 4);
    }
    #[test]
    fn derived_catchup_can_wait_but_integrity_holds_or_foreign_processes_cannot() {
        let plan = Plan::parse(&args()).unwrap();
        let held = ipc::DesktopSearchResponse {
            protocol_version: 1,
            request_id: None,
            accepted: true,
            error: None,
            operation: Some("content.search.result".into()),
            daemon_pid: 123,
            hits: None,
            hold_reason: Some("materialization_state_missing_or_stale".into()),
            next_cursor: None,
            has_more: false,
        };
        assert!(!search_observation(held.clone(), 123, &plan, &plan.local, "marker").unwrap());
        let mut invalid = held.clone();
        invalid.hits = Some(vec![]);
        assert!(search_observation(invalid, 123, &plan, &plan.local, "marker").is_err());
        assert!(search_observation(held.clone(), 456, &plan, &plan.local, "marker").is_err());
        for reason in [
            "canonical_verification_unavailable",
            "unresolved_dependency",
            "unsupported_semantics",
            "operation_deadline_exceeded",
        ] {
            let mut invalid = held.clone();
            invalid.hold_reason = Some(reason.into());
            assert!(search_observation(invalid, 123, &plan, &plan.local, "marker").is_err());
        }
    }
    #[test]
    fn only_exact_verified_marker_from_expected_device_counts_as_received() {
        let plan = Plan::parse(&args()).unwrap();
        let marker = plan.marker(&plan.peer, false);
        let hit = ipc::DesktopSearchHit {
            event_id: plan.run.clone(),
            brain_id: plan.brain.clone(),
            device_id: plan.peer.clone(),
            device_seq: "7".into(),
            created_at: "2026-11-17T00:00:00Z".into(),
            event_type: "memory.note.created".into(),
            classification: "public".into(),
            content_hash: "a".repeat(64),
            signing_key_id: "a".repeat(32),
            text: marker.clone(),
        };
        let valid = ipc::DesktopSearchResponse {
            protocol_version: 1,
            request_id: None,
            accepted: true,
            error: None,
            operation: Some("content.search.result".into()),
            daemon_pid: 123,
            hits: Some(vec![hit.clone()]),
            hold_reason: None,
            next_cursor: None,
            has_more: false,
        };
        assert!(search_observation(valid.clone(), 123, &plan, &plan.peer, &marker).unwrap());
        let mut duplicate = valid.clone();
        duplicate.hits.as_mut().unwrap().push(hit);
        assert!(search_observation(duplicate, 123, &plan, &plan.peer, &marker).is_err());
        let mut wrong = valid.clone();
        wrong.hits.as_mut().unwrap()[0].device_id = plan.local.clone();
        assert!(search_observation(wrong, 123, &plan, &plan.peer, &marker).is_err());
        let mut wrong = valid;
        wrong.hits.as_mut().unwrap()[0].text.push('x');
        assert!(search_observation(wrong, 123, &plan, &plan.peer, &marker).is_err());
    }
    #[test]
    fn missing_changed_or_unready_peer_never_changes_settings() {
        use cryptexd::managed_profile::connection_settings::Device;
        let plan = Plan::parse(&args()).unwrap();
        let good = View {
            revision: "1".into(),
            paused: false,
            listen_address: Some("192.168.1.2".parse().unwrap()),
            local_transport_port: Some(22001),
            devices: vec![Device {
                device_id: plan.peer.clone(),
                transport_id: "public_transport_id".into(),
                endpoint: Some("192.168.1.3:22002".parse().unwrap()),
            }],
            state: State::ReadyLocally,
            error: None,
        };
        assert!(plan.check_view(&good).is_ok());
        let mut paused = good.clone();
        paused.paused = true;
        paused.state = State::Paused;
        assert!(plan.check_view(&paused).is_ok());
        let mut bad = good.clone();
        bad.devices.clear();
        assert!(plan.check_view(&bad).is_err());
        let mut bad = good.clone();
        bad.devices[0].device_id = plan.local.clone();
        assert!(plan.check_view(&bad).is_err());
        let mut bad = good.clone();
        bad.devices[0].endpoint = None;
        assert!(plan.check_view(&bad).is_err());
        let mut bad = good.clone();
        bad.state = State::Applying;
        assert!(plan.check_view(&bad).is_err());
        let mut bad = good;
        bad.error = Some(cryptexd::managed_profile::connection_settings::Error::Authority);
        assert!(plan.check_view(&bad).is_err());
    }
}
