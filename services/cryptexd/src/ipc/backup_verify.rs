//! Desktop-only manual backup rehearsal. The renderer never supplies paths.
use super::*;

pub(super) const OPERATION: &str = "backup.verify.manual";
const CAPABILITY: &str = "backup.verify.owner";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub backup: PathBuf,
    pub checkpoint: PathBuf,
    pub kit: PathBuf,
    pub unlock: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    Unavailable,
    InvalidSelection,
    Unauthorized,
    VerificationFailed,
    Busy,
}

pub fn desktop(files: Selection) -> Result<(), Error> {
    backup_request::exchange(OPERATION, CAPABILITY, files, Duration::from_secs(360))
        .map_err(|_| Error::Unavailable)?
}

pub(super) fn handle(
    bytes: &[u8],
    ctx: Option<&SearchContext>,
    desktop: bool,
) -> backup_request::Reply<(), Error> {
    backup_request::handle(
        bytes,
        desktop,
        OPERATION,
        CAPABILITY,
        Error::InvalidSelection,
        Error::Unauthorized,
        |files| verify(ctx.ok_or(Error::Unavailable)?, files),
    )
}

#[cfg(any(windows, all(target_os = "macos", feature = "m8-local-startup-check")))]
fn verify(ctx: &SearchContext, files: Selection) -> Result<(), Error> {
    let ctx = ctx.admitted().map_err(|_| Error::Unavailable)?;
    let config = ctx.backup_export.as_ref().ok_or(Error::Unavailable)?;
    let _guard = config.busy.try_lock().map_err(|_| Error::Busy)?;
    for path in [&files.backup, &files.checkpoint, &files.kit, &files.unlock] {
        if !path.is_absolute() {
            return Err(Error::InvalidSelection);
        }
        crate::paths::assert_outside_repository(path).map_err(|_| Error::InvalidSelection)?;
    }
    crate::knowledge_materialization::backup::verify_manual(
        &config.codec,
        &files.backup,
        &files.checkpoint,
        &files.kit,
        &files.unlock,
    )
    .map_err(|_| Error::VerificationFailed)
}

#[cfg(not(any(windows, all(target_os = "macos", feature = "m8-local-startup-check"))))]
fn verify(_: &SearchContext, _: Selection) -> Result<(), Error> {
    Err(Error::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restore_verification_requires_its_own_desktop_capability() {
        let mut frame = serde_json::json!({"protocol_version":PROTOCOL_VERSION,
            "request_id":new_bootstrap_event_id().unwrap(),"client_role":"desktop",
            "capabilities":[CAPABILITY],"operation":OPERATION,
            "action":{"backup":"a","kit":"b","unlock":"c","checkpoint":"d"}});
        let submit = |f: &serde_json::Value, desktop| {
            handle(&serde_json::to_vec(f).unwrap(), None, desktop).result
        };
        assert_eq!(submit(&frame, false), Err(Error::Unauthorized));
        assert_eq!(submit(&frame, true), Err(Error::Unavailable));
        frame["capabilities"] = serde_json::json!(["backup.export.owner"]);
        assert_eq!(submit(&frame, true), Err(Error::Unauthorized));
    }
}
