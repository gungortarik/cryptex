//! Bounded, non-authoritative first worker failure; never read by admission.
use super::*;
use crate::owned_process::{OwnedProcessError, endpoint::EndpointError};
use crate::syncthing_owned_rest::OwnedRestError;
use serde::{Deserialize, Serialize};
const LEAF: &str = "worker-first-failure-v1.json";
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: u32,
    code: u16,
    os_code: Option<i64>,
}
// No Debug/Display strings: platform operations, paths and panic payloads stay private.
pub(super) fn worker(e: WorkerError) -> (u16, Option<i64>) {
    match e {
        WorkerError::Spawn => (1, None),
        WorkerError::Panicked => (2, None),
        WorkerError::StatusUnavailable => (3, None),
        WorkerError::InvalidDeadline => (4, None),
        WorkerError::ShutdownDeadline => (5, None),
        WorkerError::Profile(e) => profile(e),
        WorkerError::Package(e) => package(e),
        WorkerError::Native(e) => native(e),
        WorkerError::Controller(ControllerError::Native(e)) => native(e),
        WorkerError::Controller(ControllerError::RestartLimit) => (700, None),
    }
}
fn profile(e: ProfileError) -> (u16, Option<i64>) {
    match e {
        ProfileError::InvalidPath => (100, None),
        ProfileError::UnsupportedPlatform => (101, None),
        ProfileError::AlreadyExists => (102, None),
        ProfileError::Busy => (103, None),
        ProfileError::UnsafePermissions => (104, None),
        ProfileError::UnsafeObject => (105, None),
        ProfileError::InvalidState => (106, None),
        ProfileError::CredentialUnavailable => (107, None),
        ProfileError::CredentialConflict => (108, None),
        ProfileError::Io => (109, None),
        ProfileError::Platform { code, .. } => (110, Some(i64::from(code))),
    }
}
fn package(e: PackageError) -> (u16, Option<i64>) {
    match e {
        PackageError::UnsupportedTarget => (200, None),
        PackageError::InvalidPath => (201, None),
        PackageError::UnsafeObject => (202, None),
        PackageError::SizeMismatch => (203, None),
        PackageError::DigestMismatch => (204, None),
        PackageError::Io => (205, None),
        PackageError::Launch(e) => process(e),
    }
}
fn native(e: NativeConfigError) -> (u16, Option<i64>) {
    match e {
        NativeConfigError::UnsupportedPlatform => (300, None),
        NativeConfigError::InvalidPorts => (301, None),
        NativeConfigError::InvalidConfig => (302, None),
        NativeConfigError::InvalidState => (303, None),
        NativeConfigError::IncompleteGeneration => (304, None),
        NativeConfigError::IdentityChanged => (305, None),
        NativeConfigError::CredentialChanged => (306, None),
        NativeConfigError::GenerationFailed => (307, None),
        NativeConfigError::UncleanShutdown => (308, None),
        NativeConfigError::ShutdownDiagnostic => (309, None),
        NativeConfigError::StartExited => (310, None),
        NativeConfigError::StartDeadline => (311, None),
        NativeConfigError::UnresolvedCleanup => (312, None),
        NativeConfigError::ExchangeHomeMismatch => (313, None),
        NativeConfigError::ExchangeInvalidJournal => (314, None),
        NativeConfigError::ExchangeUnrecognizedState => (315, None),
        NativeConfigError::ExchangeCapacityExceeded => (316, None),
        NativeConfigError::ExchangeReadbackMismatch => (317, None),
        NativeConfigError::ExchangePolicyMismatch => (318, None),
        NativeConfigError::Profile(e) => profile(e),
        NativeConfigError::Package(e) => package(e),
        NativeConfigError::Process(e) => process(e),
        NativeConfigError::Rest(e) => rest(e),
    }
}
fn rest(e: OwnedRestError) -> (u16, Option<i64>) {
    match e {
        OwnedRestError::InvalidIdentity => (400, None),
        OwnedRestError::RuntimeUnavailable => (401, None),
        OwnedRestError::Deadline => (402, None),
        OwnedRestError::Unavailable => (403, None),
        OwnedRestError::InvalidResponse => (404, None),
        OwnedRestError::ResponseTooLarge => (405, None),
        OwnedRestError::RedirectRefused => (406, None),
        OwnedRestError::VersionMismatch => (407, None),
        OwnedRestError::IdentityMismatch => (408, None),
        OwnedRestError::ListenerUnavailable => (409, None),
        OwnedRestError::Endpoint(e) => endpoint(e),
        OwnedRestError::Unauthorized(code) => (410, Some(i64::from(code))),
        OwnedRestError::HttpStatus(code) => (411, Some(i64::from(code))),
    }
}
fn endpoint(e: EndpointError) -> (u16, Option<i64>) {
    match e {
        EndpointError::InvalidEndpoint => (500, None),
        EndpointError::UnsupportedPlatform => (501, None),
        EndpointError::Unavailable => (502, None),
        EndpointError::Deadline => (503, None),
        EndpointError::OwnerExited => (504, None),
        EndpointError::PeerNotEstablished => (505, None),
        EndpointError::PeerNotOwned => (506, None),
        EndpointError::PeerChanged => (507, None),
        EndpointError::TableUnavailable => (508, None),
        EndpointError::TableTooLarge => (509, None),
        EndpointError::InvalidTable => (510, None),
    }
}
fn process(e: OwnedProcessError) -> (u16, Option<i64>) {
    match e {
        OwnedProcessError::InvalidLaunch => (600, None),
        OwnedProcessError::UnsafeWorkingDirectory => (601, None),
        OwnedProcessError::UnsupportedPlatform => (602, None),
        OwnedProcessError::Deadline => (603, None),
        OwnedProcessError::Platform { code, .. } => (604, Some(code)),
    }
}

fn record(profile: &ManagedProfile, error: WorkerError) -> Result<(), ProfileError> {
    // Preserve the first record even across restart. Never truncate an existing leaf.
    if let Some(bytes) = profile.read(LEAF)? {
        let saved: Record =
            serde_json::from_slice(&bytes).map_err(|_| ProfileError::InvalidState)?;
        if saved.schema != 1 {
            return Err(ProfileError::InvalidState);
        }
        return Ok(());
    }
    let (code, os_code) = worker(error);
    let bytes = serde_json::to_vec(&Record {
        schema: 1,
        code,
        os_code,
    })
    .map_err(|_| ProfileError::InvalidState)?;
    profile.write_new(LEAF, &bytes)
}
pub(super) fn finish(
    profile: &ManagedProfile,
    result: Result<Outcome, WorkerError>,
) -> Result<Outcome, WorkerError> {
    if let Err(error) = result
        && record(profile, error).is_err()
    {
        eprintln!("component=managed_transport operation=first_failure_record status=unavailable");
    }
    // Recording failure must never replace the original failure or turn it into success.
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_failure_survives_later_shutdown_and_reopen() {
        let parent = crate::test_temp::exclusive_temp_dir("worker-first-failure");
        let path = parent.join("00000000000000000000000000000001");
        let profile = ManagedProfile::create(&path, SecureStoreEnvironment::Dev).unwrap();
        let first = WorkerError::Native(NativeConfigError::Rest(OwnedRestError::Endpoint(
            EndpointError::PeerChanged,
        )));
        assert_eq!(finish(&profile, Err(first)), Err(first));
        let bytes = profile.read(LEAF).unwrap().unwrap();
        let saved: Record = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(saved.code, 507);
        drop(profile);
        let profile = ManagedProfile::open(&path, SecureStoreEnvironment::Dev).unwrap();
        assert_eq!(
            finish(&profile, Err(WorkerError::ShutdownDeadline)),
            Err(WorkerError::ShutdownDeadline)
        );
        assert_eq!(profile.read(LEAF).unwrap().unwrap(), bytes);
        drop(profile);
        std::fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn malformed_record_does_not_mask_failure_or_get_overwritten() {
        let parent = crate::test_temp::exclusive_temp_dir("worker-bad-diagnostic");
        let path = parent.join("00000000000000000000000000000001");
        let profile = ManagedProfile::create(&path, SecureStoreEnvironment::Dev).unwrap();
        profile.write_new(LEAF, b"invalid").unwrap();
        assert_eq!(
            finish(&profile, Err(WorkerError::Panicked)),
            Err(WorkerError::Panicked)
        );
        assert_eq!(profile.read(LEAF).unwrap().unwrap(), b"invalid");
        drop(profile);
        std::fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn platform_operation_strings_never_leave_mapping() {
        let error = WorkerError::Native(NativeConfigError::Process(OwnedProcessError::Platform {
            operation: "PRIVATE SENTINEL",
            code: 123,
        }));
        assert_eq!(worker(error), (604, Some(123)));
        assert_eq!(
            worker(WorkerError::Profile(ProfileError::Platform {
                operation: "PRIVATE",
                code: 5
            })),
            (110, Some(5))
        );
    }
}
