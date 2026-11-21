//! Store only the normalized public certificate. The independent device
//! credential pins its digest and the immutable Brain identity bytes.
use super::{SearchContext, backup_export::Error};
use crate::import_staging::{export_fs::Directory, open_recovery_source};
use crate::knowledge_materialization::backup;
use crate::local_device_identity::LocalDeviceKeyResolver;
use crate::secure_store::{AccountLabel, SecureStoreError, ServiceLabel};
use cryptex_events::TrustedDaemonKeyResolver;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;

const CACHE: &str = "backup-certificates";

fn read(path: &Path, limit: usize) -> Result<Vec<u8>, Error> {
    let (file, _) = open_recovery_source(path).map_err(|_| Error::InvalidRecipient)?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::InvalidRecipient)?;
    if bytes.is_empty() || bytes.len() > limit {
        return Err(Error::InvalidRecipient);
    }
    Ok(bytes)
}

pub(super) struct Binding {
    brain: [u8; 32],
    service: ServiceLabel,
    account: AccountLabel,
    pin: Option<[u8; 32]>,
}
impl Binding {
    pub(super) fn load(ctx: &SearchContext) -> Result<Self, Error> {
        let bytes = read(
            &crate::identity_runtime::brain_identity_path(&ctx.runtime_root),
            4096,
        )?;
        let identity = cryptex_core::BrainIdentity::from_json(
            std::str::from_utf8(&bytes).map_err(|_| Error::Unavailable)?,
        )
        .map_err(|_| Error::Unavailable)?;
        let anchor = LocalDeviceKeyResolver::load(
            &ctx.runtime_root,
            &identity.brain_id,
            ctx.store.as_ref(),
            ctx.environment,
        )
        .map_err(|_| Error::Unavailable)?
        .local_trust_anchor()
        .ok_or(Error::Unavailable)?;
        let brain = Sha256::digest(bytes).into();
        let service = ServiceLabel::device_credentials(ctx.environment);
        let account = AccountLabel::backup_recipient(anchor.device_id().as_str());
        let pin = match ctx.store.get(service.as_str(), account.as_str()) {
            Ok(value) if value.len() == 64 && value[..32] == brain => Some(
                value[32..]
                    .try_into()
                    .map_err(|_| Error::InvalidRecipient)?,
            ),
            Ok(_) => return Err(Error::InvalidRecipient),
            Err(SecureStoreError::Missing) => None,
            Err(_) => return Err(Error::Unavailable),
        };
        Ok(Self {
            brain,
            service,
            account,
            pin,
        })
    }

    pub(super) fn existing(&self, ctx: &SearchContext) -> Result<Option<Vec<u8>>, Error> {
        let Some(pin) = self.pin else { return Ok(None) };
        let name = filename(&pin);
        let bytes = read(&ctx.runtime_root.join(CACHE).join(name), 65536)?;
        if <[u8; 32]>::from(Sha256::digest(&bytes)) != pin {
            return Err(Error::InvalidRecipient);
        }
        Ok(Some(bytes))
    }

    pub(super) fn inspect(&self, folder: &Path, codec: &Path) -> Result<Vec<u8>, Error> {
        if !folder.is_absolute() || read(&folder.join("brain-identity.sha256"), 32)? != self.brain {
            return Err(Error::InvalidRecipient);
        }
        let cert = read(&folder.join("recovery.pub"), 65536)?;
        backup::public_recipient(codec, &cert).map_err(|_| Error::InvalidRecipient)
    }

    pub(super) fn enroll(
        &self,
        ctx: &SearchContext,
        cert: &[u8],
        confirmed: &str,
    ) -> Result<(), Error> {
        let digest = Sha256::digest(cert);
        if confirmed != format!("{digest:x}") || self.pin.is_some() {
            return Err(Error::InvalidRecipient);
        }
        let dir = Directory::open(&ctx.runtime_root)
            .and_then(|dir| dir.child(CACHE, true))
            .map_err(|_| Error::Unavailable)?;
        let name = filename(&digest.into());
        if !dir.exists(&name).map_err(|_| Error::Unavailable)? {
            let stage = format!(
                "recipient-{}.partial",
                super::new_bootstrap_event_id().map_err(|_| Error::Unavailable)?
            );
            let mut out = dir
                .open_file(&stage, true)
                .map_err(|_| Error::Unavailable)?;
            out.write_all(cert).map_err(|_| Error::Unavailable)?;
            dir.promote(&stage, &out, &dir, &name)
                .map_err(|_| Error::Unavailable)?;
        }
        // A retry after an interruption may find a complete, as yet unpinned certificate.
        if read(&ctx.runtime_root.join(CACHE).join(name), 65536)? != cert {
            return Err(Error::InvalidRecipient);
        }
        let mut value = self.brain.to_vec();
        value.extend_from_slice(&digest);
        ctx.store
            .put(self.service.as_str(), self.account.as_str(), &value)
            .map_err(|_| Error::Uncertain)
    }
}

fn filename(digest: &[u8; 32]) -> String {
    format!("{:x}.asc", sha2::digest::Output::<Sha256>::from(*digest))
}
