//! Reader evidence is local to a transport session. Slots in the disposable
//! exchange tree are untrusted inputs, never canonical events or admission grants.
use super::Directory;
use crate::knowledge_materialization::VerifiedCanonicalSnapshot;
use crate::managed_profile::exchange_policy::ExchangePeers;
use crate::secure_store::{SecureStore, SecureStoreEnvironment};
use crate::trust_reducer::ApprovedPeerBinding;
use cryptex_events::ContentHashHex;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

mod probe;
mod wire;
use wire::{Body, Route};

use crate::import_staging::READER_CONTROL_FOLDER;
const FRESHNESS: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Error {
    Message,
    Storage,
    LocalReader,
    Authority,
}
impl From<crate::import_staging::export_fs::Error> for Error {
    fn from(_: crate::import_staging::export_fs::Error) -> Self {
        Self::Storage
    }
}
impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Self::Storage
    }
}

struct Round {
    started: Instant,
    roster: Vec<ApprovedPeerBinding>,
    nonce: ContentHashHex,
}

#[derive(Default)]
pub(super) struct ReaderCheck {
    round: Option<Round>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) enum Status {
    #[default]
    Legacy,
    Waiting,
    Rejected,
    Unavailable(Error),
    Verified(BTreeMap<String, ContentHashHex>),
}

impl ReaderCheck {
    pub(super) fn tick(
        &mut self,
        snapshot: &VerifiedCanonicalSnapshot,
        peers: &ExchangePeers<'_>,
        incoming: &Directory,
        transport: &Directory,
        credentials: (&dyn SecureStore, SecureStoreEnvironment),
        check: &impl Fn() -> Result<(), super::Error>,
    ) -> Result<Status, Error> {
        let root = peers.runtime_root();
        let lock = root.join("daemon.lock");
        if lock.symlink_metadata()?.is_file() {
            self.round = None;
            return Ok(Status::Legacy);
        }
        crate::import_staging::open_existing_profile_lock(&lock, "reader-m9.pid")
            .map_err(|_| Error::LocalReader)?;
        let (build, resolved_blob) = probe::local_build()?;
        let (history, _) = snapshot.verified_trust_context().ok_or(Error::Authority)?;
        let bindings = peers.bindings();
        if bindings.is_empty() || bindings.len() > super::super::connection_settings::MAX_PEERS {
            return Err(Error::Authority);
        }
        if self.round.as_ref().is_none_or(|r| {
            r.started.elapsed() >= FRESHNESS || !r.roster.iter().eq(bindings.iter().copied())
        }) {
            let mut random = [0; 32];
            getrandom::getrandom(&mut random).map_err(|_| Error::LocalReader)?;
            self.round = Some(Round {
                started: Instant::now(),
                roster: bindings.iter().map(|p| (**p).clone()).collect(),
                nonce: ContentHashHex::from_bytes(&random).map_err(|_| Error::Message)?,
            });
        }
        let round = self.round.as_ref().ok_or(Error::LocalReader)?;
        let controls = incoming.child(READER_CONTROL_FOLDER, true)?;
        let staging = transport.child("m9-control-staging", true)?;
        let mut verified = BTreeMap::new();
        let mut rejected = false;
        let mut signer = None;
        for peer in bindings {
            check().map_err(|_| Error::Authority)?;
            let route = Route {
                brain: peers.brain_id().into(),
                sender: history.local_device.as_str().into(),
                receiver: peer.device_id.as_str().into(),
                sender_transport: peers.local_transport().into(),
                receiver_transport: peer.transport_device_id.clone(),
            };
            let reverse = route.reversed();
            let other = match read(&controls, &reverse.slot()?)? {
                Some(raw) => match wire::authenticate(&raw, &reverse, &peer.public_key) {
                    Ok(body) if body.resolved_blob == resolved_blob => Some(body),
                    _ => {
                        rejected = true;
                        None
                    }
                },
                None => None,
            };
            if let Some(other) = &other
                && other.echo.as_ref() == Some(&round.nonce)
            {
                verified.insert(route.receiver.clone(), other.daemon_sha256.clone());
            }
            let slot = route.slot()?;
            let body = Body {
                route,
                nonce: round.nonce.clone(),
                echo: other.map(|m| m.nonce),
                daemon_sha256: build.clone(),
                resolved_blob: resolved_blob.clone(),
            };
            if let Some(raw) = read(&controls, &slot)?
                && wire::authenticate(&raw, &body.route, &history.local_public_key)
                    .is_ok_and(|previous| previous == body)
            {
                continue;
            }
            if signer.is_none() {
                let key = crate::event_signing_key::load_existing_event_signing_key(
                    credentials.0,
                    credentials.1,
                    root,
                    &history.local_device,
                )
                .map_err(|_| Error::Authority)?
                .signing_key;
                if key.verifying_key().to_bytes() != history.local_public_key {
                    return Err(Error::Authority);
                }
                signer = Some(key);
            }
            let signed = wire::sign(body, signer.as_ref().ok_or(Error::Authority)?)?;
            check().map_err(|_| Error::Authority)?;
            publish(&staging, &controls, &slot, &signed)?;
        }
        check().map_err(|_| Error::Authority)?;
        if rejected {
            return Ok(Status::Rejected);
        }
        if verified.len() == bindings.len() && round.started.elapsed() < FRESHNESS {
            Ok(Status::Verified(verified))
        } else {
            Ok(Status::Waiting)
        }
    }
}

fn read(dir: &Directory, slot: &str) -> Result<Option<Vec<u8>>, Error> {
    if !dir.exists(slot)? {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    dir.open_file(slot, false)?
        .take(wire::LIMIT + 1)
        .read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

fn publish(
    scratch: &Directory,
    controls: &Directory,
    slot: &str,
    bytes: &[u8],
) -> Result<(), Error> {
    if scratch.exists("segment.tmp")? {
        scratch.discard_scratch(scratch.open_file("segment.tmp", false)?)?;
    }
    let mut staged = scratch.open_file("segment.tmp", true)?;
    staged.write_all(bytes)?;
    staged.sync_all()?;
    // Replace only our own direction's derived slot. A crash in the gap
    // means evidence is missing; the next scheduled tick republishes it.
    if controls.exists(slot)? {
        controls.remove_reader_slot(slot, controls.open_file(slot, false)?)?;
    }
    scratch.promote("segment.tmp", &staged, controls, slot)?;
    Ok(())
}
