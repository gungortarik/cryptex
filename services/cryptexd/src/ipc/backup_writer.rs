//! Owner-selected filesystem targets for the scheduled backup writer.
use super::*;

pub(super) const OPERATION: &str = "backup.writer.target";
const CAPABILITY: &str = "backup.writer.owner";

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Create { parent: PathBuf },
    Inspect { target: PathBuf },
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    Cancelled,
    Created { folder: String },
    LocalWriter,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    Unauthorized,
    InvalidRequest,
    Unavailable,
    Busy,
    UnsafeTarget,
    Authority,
    History,
    Uncertain,
}

pub fn desktop(action: Action) -> Result<Outcome, Error> {
    let uncertain = matches!(action, Action::Create { .. });
    backup_request::exchange(OPERATION, CAPABILITY, action, Duration::from_secs(90)).map_err(
        |_| {
            if uncertain {
                Error::Uncertain
            } else {
                Error::Unavailable
            }
        },
    )?
}

pub(super) fn handle(
    frame: &[u8],
    ctx: Option<&SearchContext>,
    desktop: bool,
) -> backup_request::Reply<Outcome, Error> {
    backup_request::handle(
        frame,
        desktop,
        OPERATION,
        CAPABILITY,
        Error::InvalidRequest,
        Error::Unauthorized,
        |action| execute(ctx.ok_or(Error::Unavailable)?, action),
    )
}

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
fn execute(ctx: &SearchContext, action: Action) -> Result<Outcome, Error> {
    use crate::knowledge_materialization::backup::{independent_folders, role};
    use crate::local_device_identity::LocalDeviceKeyResolver;
    use cryptex_events::TrustedDaemonKeyResolver;

    let ctx = ctx.admitted().map_err(|_| Error::Unavailable)?;
    let config = ctx.backup_export.as_ref().ok_or(Error::Unavailable)?;
    let _guard = config.busy.try_lock().map_err(|_| Error::Busy)?;
    let path = match &action {
        Action::Create { parent } => parent,
        Action::Inspect { target } => target,
    };
    if !path.is_absolute() || !path.is_dir() {
        return Err(Error::UnsafeTarget);
    }
    crate::paths::assert_outside_repository(path).map_err(|_| Error::UnsafeTarget)?;
    independent_folders(&ctx.runtime_root, path).map_err(|_| Error::UnsafeTarget)?;
    let brain = crate::identity_runtime::load_brain_identity(&ctx.runtime_root)
        .map_err(|_| Error::History)?;
    let anchor = LocalDeviceKeyResolver::load(
        &ctx.runtime_root,
        &brain.brain_id,
        ctx.store.as_ref(),
        ctx.environment,
    )
    .map_err(|_| Error::Authority)?
    .local_trust_anchor()
    .ok_or(Error::Authority)?;
    let writer = role::DeviceRole {
        local: &anchor,
        store: ctx.store.as_ref(),
        env: ctx.environment,
    };
    let map = |e| match e {
        role::Error::Target => Error::UnsafeTarget,
        role::Error::Authority => Error::Authority,
        role::Error::Signature | role::Error::History => Error::History,
        role::Error::Store => Error::Uncertain,
    };
    match action {
        Action::Create { parent } => {
            let (target, _) = writer.create(&ctx.runtime_root, &parent).map_err(map)?;
            writer
                .verify_local_writer(&ctx.runtime_root, &target)
                .map_err(|_| Error::Uncertain)?;
            let folder = target
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(Error::Uncertain)?;
            Ok(Outcome::Created {
                folder: folder.to_owned(),
            })
        }
        Action::Inspect { target } => {
            writer
                .verify_local_writer(&ctx.runtime_root, &target)
                .map_err(map)?;
            Ok(Outcome::LocalWriter)
        }
    }
}

#[cfg(not(any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))))]
fn execute(_: &SearchContext, _: Action) -> Result<Outcome, Error> {
    Err(Error::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writing_requires_an_authenticated_desktop_with_the_writer_capability() {
        let mut frame = serde_json::json!({"protocol_version":PROTOCOL_VERSION,
            "request_id":new_bootstrap_event_id().unwrap(),"client_role":"desktop",
            "capabilities":[CAPABILITY],"operation":OPERATION,"action":{"kind":"create","parent":"test"}});
        let submit = |f: &serde_json::Value, peer| {
            handle(&serde_json::to_vec(f).unwrap(), None, peer).result
        };
        assert_eq!(submit(&frame, false), Err(Error::Unauthorized));
        assert_eq!(submit(&frame, true), Err(Error::Unavailable));
        frame["capabilities"] = serde_json::json!(["backup.export.owner"]);
        assert_eq!(submit(&frame, true), Err(Error::Unauthorized));
    }
}
