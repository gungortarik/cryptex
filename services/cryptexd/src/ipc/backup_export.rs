use super::backup_request;
use super::*;

pub(super) const OPERATION: &str = "backup.export.manual";
const CAPABILITY: &str = "backup.export.owner";

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
pub(super) struct Config {
    pub(super) codec: PathBuf,
    pub(super) recipient: Vec<u8>,
    pub(super) busy: Mutex<()>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Inspect,
    Export {
        target: PathBuf,
        checkpoint: PathBuf,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Ready,
    Cancelled,
    Written { backup: String, checkpoint: String },
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    Unauthorized,
    InvalidRequest,
    Unavailable,
    Busy,
    UnsafeDestination,
    HistoryHeld,
    EncryptionFailed,
    Uncertain,
}

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
fn execute(ctx: &SearchContext, action: Action) -> Result<Outcome, Error> {
    use crate::backup_inventory::Error as BackupError;
    use crate::knowledge_materialization::backup;

    let ctx = ctx.admitted().map_err(|_| Error::Unavailable)?;
    let config = ctx.backup_export.as_ref().ok_or(Error::Unavailable)?;
    let Action::Export { target, checkpoint } = action else {
        return Ok(Outcome::Ready);
    };
    let _job = config.busy.try_lock().map_err(|_| Error::Busy)?;
    for directory in [&target, &checkpoint] {
        if !directory.is_absolute() || !directory.is_dir() {
            return Err(Error::UnsafeDestination);
        }
        crate::paths::assert_outside_repository(directory).map_err(|_| Error::UnsafeDestination)?;
        backup::independent_folders(&ctx.runtime_root, directory)
            .map_err(|_| Error::UnsafeDestination)?;
    }
    // Manual exports have their own namespace; each is a standalone generation.
    let prepared = backup::prepare(&ctx.runtime_root, ctx.store.as_ref(), ctx.environment, 1)
        .map_err(|_| Error::HistoryHeld)?;
    let (backup, checkpoint) = prepared
        .export_manual_bundle(&config.codec, &config.recipient, &target, &checkpoint)
        .map_err(|e| match e {
            BackupError::ExportDestination => Error::UnsafeDestination,
            BackupError::ExportPublication => Error::Uncertain,
            _ => Error::EncryptionFailed,
        })?;
    let name = |file: PathBuf| {
        file.file_name()
            .and_then(|s| s.to_str())
            .map(str::to_owned)
            .ok_or(Error::Uncertain)
    };
    Ok(Outcome::Written {
        backup: name(backup)?,
        checkpoint: name(checkpoint)?,
    })
}

#[cfg(not(any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))))]
fn execute(_: &SearchContext, _: Action) -> Result<Outcome, Error> {
    Err(Error::Unavailable)
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
        Error::InvalidRequest,
        Error::Unauthorized,
        |action| execute(ctx.ok_or(Error::Unavailable)?, action),
    )
}

pub fn desktop(action: Action) -> Result<Outcome, Error> {
    let writing = matches!(action, Action::Export { .. });
    let timeout = if writing {
        Duration::from_secs(360)
    } else {
        IPC_RESPONSE_DEADLINE
    };
    backup_request::exchange(OPERATION, CAPABILITY, action, timeout).map_err(|_| {
        if writing {
            Error::Uncertain
        } else {
            Error::Unavailable
        }
    })?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_export_requires_desktop_and_disallows_recipient_substitution() {
        let mut frame = serde_json::json!({
            "request_id": new_bootstrap_event_id().unwrap(), "protocol_version": PROTOCOL_VERSION,
            "operation": OPERATION, "client_role": "desktop", "capabilities": [CAPABILITY],
            "action": {"kind":"export", "target":"a", "checkpoint":"b"}
        });
        let send = |f: &serde_json::Value, desktop| {
            handle(&serde_json::to_vec(f).unwrap(), None, desktop).result
        };
        assert_eq!(send(&frame, false), Err(Error::Unauthorized));
        assert_eq!(send(&frame, true), Err(Error::Unavailable));
        frame["action"]["recipient"] = "another key".into();
        assert_eq!(send(&frame, true), Err(Error::InvalidRequest));
    }
}
