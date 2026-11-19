//! Live challenges and previously witnessed reader capability have distinct
//! statuses. Exchange slots are untrusted inputs, never admission grants.
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
mod receipt;
mod retained;
#[cfg(test)]
pub(crate) use retained::tests::native_observation_phase;
mod wire;
use wire::{Body, Route};

use crate::import_staging::READER_CONTROL_FOLDER;
const FRESHNESS: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    context: ContentHashHex,
    recorded: BTreeMap<String, Body>,
}

#[derive(Default)]
pub(super) struct ReaderCheck {
    round: Option<Round>,
    receipt_written: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) enum Status {
    #[default]
    Legacy,
    Waiting,
    Rejected,
    Unavailable(Error),
    Verified(BTreeMap<String, ContentHashHex>),
    PreviouslyVerified(BTreeMap<String, ContentHashHex>),
}

impl ReaderCheck {
    #[cfg(test)]
    pub(super) fn expire_current_round(&mut self) {
        self.round.as_mut().expect("active reader round").started -= FRESHNESS;
    }

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
        let profile_lock =
            crate::import_staging::open_existing_profile_lock(&lock, "reader-m9.pid")
                .map_err(|_| Error::LocalReader)?;
        let (build, resolved_blob) = probe::local_build()?;
        let context = retained::context(snapshot, &profile_lock, &build)?;
        let (history, _) = snapshot.verified_trust_context().ok_or(Error::Authority)?;
        let bindings = peers.bindings();
        if bindings.is_empty() || bindings.len() > super::super::connection_settings::MAX_PEERS {
            return Err(Error::Authority);
        }
        if self.round.as_ref().is_none_or(|r| {
            r.started.elapsed() >= FRESHNESS
                || r.context != context
                || !r.roster.iter().eq(bindings.iter().copied())
        }) {
            let mut random = [0; 32];
            getrandom::getrandom(&mut random).map_err(|_| Error::LocalReader)?;
            self.round = Some(Round {
                started: Instant::now(),
                roster: bindings.iter().map(|p| (**p).clone()).collect(),
                nonce: ContentHashHex::from_bytes(&random).map_err(|_| Error::Message)?,
                context,
                recorded: BTreeMap::new(),
            });
        }
        let round = self.round.as_mut().ok_or(Error::LocalReader)?;
        let controls = incoming.child(READER_CONTROL_FOLDER, true)?;
        let staging = transport.child("m9-control-staging", true)?;
        let mut verified = BTreeMap::new();
        let mut rejected = false;
        let signing_key = std::cell::OnceCell::new();
        let signer = || {
            signing_key
                .get_or_init(|| {
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
                    Ok(key)
                })
                .as_ref()
                .map_err(|e| *e)
        };
        let mut retained = BTreeMap::new();
        let mut evidence_error = None;
        let mut observed = None;
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
            let raw = read(&controls, &reverse.slot()?)?;
            let other = raw
                .as_deref()
                .and_then(|msg| wire::authenticate(msg, &reverse, &peer.public_key).ok());
            let compatible = other.as_ref().filter(|m| m.resolved_blob == resolved_blob);
            rejected |= raw.is_some() && compatible.is_none();
            let slot = route.slot()?;
            let body = Body {
                route,
                nonce: round.nonce.clone(),
                echo: compatible.map(|m| m.nonce.clone()),
                daemon_sha256: build.clone(),
                resolved_blob: resolved_blob.clone(),
            };
            let unchanged = read(&controls, &slot)?.is_some_and(|raw| {
                wire::authenticate(&raw, &body.route, &history.local_public_key)
                    .is_ok_and(|previous| previous == body)
            });
            if !unchanged {
                let signed = wire::sign(body, signer()?)?;
                check().map_err(|_| Error::Authority)?;
                publish(&staging, &controls, &slot, &signed)?;
            }
            let evidence = retained::Slot::new(credentials, &reverse)?;
            check().map_err(|_| Error::Authority)?;
            if let (Some(reply), Some(bytes)) = (compatible, raw.as_deref())
                && reply.echo.as_ref() == Some(&round.nonce)
                && round.started.elapsed() < FRESHNESS
            {
                verified.insert(peer.device_id.to_string(), reply.daemon_sha256.clone());
                if !self.receipt_written && bindings.len() == 1 {
                    observed = Some(reply.clone());
                }
                if round.recorded.get(&slot) != Some(reply) {
                    let previous = evidence.load(
                        &round.context,
                        &reverse,
                        &history.local_public_key,
                        &peer.public_key,
                        &resolved_blob,
                        Some(reply),
                    );
                    let saved = match previous {
                        Ok(Some(_)) => Ok(()),
                        Ok(None) => evidence.save(&round.context, &round.nonce, bytes, signer()?),
                        Err(Error::Message) => {
                            eprintln!(
                                "component=m9_reader_check status=damaged_observation_fresh_reply"
                            );
                            evidence.save(&round.context, &round.nonce, bytes, signer()?)
                        }
                        Err(error) => Err(error),
                    };
                    match saved {
                        Ok(()) => {
                            round.recorded.insert(slot, reply.clone());
                        }
                        Err(error) => evidence_error = Some(error),
                    }
                }
            } else {
                round.recorded.remove(&slot);
                match evidence.load(
                    &round.context,
                    &reverse,
                    &history.local_public_key,
                    &peer.public_key,
                    &resolved_blob,
                    other.as_ref(),
                ) {
                    Ok(Some(build)) => {
                        retained.insert(peer.device_id.to_string(), build);
                    }
                    Ok(None) => {}
                    Err(error) => evidence_error = Some(error),
                }
            }
        }
        check().map_err(|_| Error::Authority)?;
        if rejected {
            return Ok(Status::Rejected);
        }
        if let Some(error) = evidence_error {
            return Err(error);
        }
        if verified.len() == bindings.len() && round.started.elapsed() < FRESHNESS {
            if let Some(peer) = observed {
                match receipt::write_if_requested(&staging, &round.nonce, &build, &peer) {
                    Ok(written) => self.receipt_written = written,
                    Err(error) => {
                        eprintln!("component=m9_reader_receipt status=unavailable error={error:?}")
                    }
                }
            }
            Ok(Status::Verified(verified))
        } else if !retained.is_empty() && verified.len() + retained.len() == bindings.len() {
            retained.extend(verified);
            Ok(Status::PreviouslyVerified(retained))
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
