//! Desktop exchanges public files. Only the separate offline tool can sign.
use super::*;

pub(super) const OPERATION: &str = "recovery.first_device.exchange";
const CAPABILITY: &str = "recovery.first_device.owner";

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Inspect { files: backup_verify::Selection },
    Prepare { files: backup_verify::Selection },
    Export { folder: PathBuf },
    Import { approval: PathBuf },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Cancelled,
    Prepared,
    ArchiveVerified {
        brain: String,
        kit: String,
        backup_sha256: String,
    },
    Challenge {
        file: String,
        device: String,
        fingerprint: String,
    },
    Accepted,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    Unavailable,
    Unauthorized,
    InvalidSelection,
    NoPendingRecovery,
    Busy,
    Rejected,
}

pub(super) fn handle(
    bytes: &[u8],
    ctx: Option<&SearchContext>,
    desktop: bool,
) -> backup_request::Reply<Outcome, Error> {
    backup_request::handle(
        bytes,
        desktop,
        OPERATION,
        CAPABILITY,
        Error::InvalidSelection,
        Error::Unauthorized,
        |action| execute(ctx.ok_or(Error::Unavailable)?, action),
    )
}

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
fn execute(ctx: &SearchContext, action: Action) -> Result<Outcome, Error> {
    use crate::knowledge_materialization::backup::{first_device, independent_folders};
    let config = ctx.backup_export.as_ref().ok_or(Error::Unavailable)?;
    let _busy = config.busy.try_lock().map_err(|_| Error::Busy)?;
    let folder = match &action {
        Action::Inspect { files } | Action::Prepare { files } => {
            for path in [&files.backup, &files.checkpoint, &files.kit, &files.unlock] {
                if !path.is_absolute() {
                    return Err(Error::InvalidSelection);
                }
                crate::paths::assert_outside_repository(path)
                    .map_err(|_| Error::InvalidSelection)?;
                independent_folders(&ctx.runtime_root, path)
                    .map_err(|_| Error::InvalidSelection)?;
            }
            if matches!(action, Action::Prepare { .. }) {
                let permit = ctx
                    .admission
                    .exclusive(
                        &ctx.runtime_root,
                        Instant::now() + Duration::from_secs(10),
                        false,
                    )
                    .map_err(|_| Error::Busy)?;
                let prepared = first_device::prepare(
                    &ctx.runtime_root,
                    ctx.store.as_ref(),
                    ctx.environment,
                    &config.codec,
                    files,
                )
                .map_err(|_| Error::Rejected);
                // Both publication steps are exclusive and resumable. A pending
                // archive cannot authorize an ordinary read without its signed grant.
                permit.finish(None);
                return prepared.map(|()| Outcome::Prepared);
            }
            let _ctx = ctx.admitted().map_err(|_| Error::Unavailable)?;
            let (brain, kit, backup_sha256) =
                crate::knowledge_materialization::backup::inspect_first_archive(
                    &config.codec,
                    files,
                )
                .map_err(|_| Error::Rejected)?;
            return Ok(Outcome::ArchiveVerified {
                brain,
                kit,
                backup_sha256,
            });
        }
        Action::Export { folder } => folder.as_path(),
        Action::Import { approval } => approval.parent().ok_or(Error::InvalidSelection)?,
    };
    if !folder.is_absolute() {
        return Err(Error::InvalidSelection);
    }
    crate::paths::assert_outside_repository(folder).map_err(|_| Error::InvalidSelection)?;
    independent_folders(&ctx.runtime_root, folder).map_err(|_| Error::InvalidSelection)?;
    let ctx = ctx.admitted().map_err(|_| Error::Unavailable)?;
    let ceremony = first_device::bound(&ctx.runtime_root, ctx.store.as_ref(), ctx.environment)
        .map_err(|_| Error::NoPendingRecovery)?;
    match action {
        Action::Inspect { .. } | Action::Prepare { .. } => unreachable!(),
        Action::Export { folder } => {
            let (file, device, fingerprint) = ceremony
                .export_challenge(&folder)
                .map_err(|_| Error::InvalidSelection)?;
            Ok(Outcome::Challenge {
                file,
                device,
                fingerprint,
            })
        }
        Action::Import { approval } => {
            ceremony
                .accept_owner_file(&ctx.runtime_root, &approval)
                .map_err(|_| Error::Rejected)?;
            Ok(Outcome::Accepted)
        }
    }
}
#[cfg(not(any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))))]
fn execute(_: &SearchContext, _: Action) -> Result<Outcome, Error> {
    Err(Error::Unavailable)
}

pub fn desktop(action: Action) -> Result<Outcome, Error> {
    let timeout = if matches!(action, Action::Inspect { .. } | Action::Prepare { .. }) {
        660
    } else {
        120
    };
    backup_request::exchange(OPERATION, CAPABILITY, action, Duration::from_secs(timeout))
        .map_err(|_| Error::Unavailable)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_file_exchange_requires_the_owner_desktop() {
        let mut frame = serde_json::json!({"protocol_version": PROTOCOL_VERSION,
            "request_id": new_bootstrap_event_id().unwrap(), "client_role": "desktop",
            "capabilities": [CAPABILITY], "operation": OPERATION,
            "action": {"kind": "import", "approval": "file.json"}});
        let submit = |frame: &serde_json::Value, desktop| {
            handle(&serde_json::to_vec(frame).unwrap(), None, desktop)
                .result
                .unwrap_err()
        };
        assert_eq!(submit(&frame, true), Error::Unavailable);
        assert_eq!(submit(&frame, false), Error::Unauthorized);
        frame["client_role"] = "ai".into();
        assert_eq!(submit(&frame, true), Error::Unauthorized);
        frame["client_role"] = "desktop".into();
        frame["capabilities"] = serde_json::json!(["backup.verify.owner"]);
        assert_eq!(submit(&frame, true), Error::Unauthorized);
    }
}
