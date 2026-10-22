//! Explicit isolated native provisioning; no startup hook, arbitrary filesystem
//! API, canonical writer, folder pairing or peer authorization is enabled.
use std::fmt;

use serde::{Deserialize, Serialize};

use super::{ManagedProfile, ProfileError};
use crate::owned_process::OwnedProcessError;
use crate::secure_store::SecureStore;
use crate::syncthing_owned_rest::OwnedRestError;
use crate::syncthing_package::{PackageError, PinnedSyncthingPackage};

#[cfg(windows)]
use {
    super::{native_xml, windows::PrivateDirectory},
    crate::owned_process::ExitObservation,
    crate::syncthing_owned_rest::OwnedSyncthingRest,
    crate::syncthing_package::PackagedProcess,
    std::io::{Read, Write},
    std::sync::Arc,
    std::time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeConfigError {
    UnsupportedPlatform,
    InvalidPorts,
    InvalidConfig,
    InvalidState,
    IncompleteGeneration,
    IdentityChanged,
    CredentialChanged,
    GenerationFailed,
    UncleanShutdown,
    ShutdownDiagnostic,
    StartExited,
    StartDeadline,
    UnresolvedCleanup,
    Profile(ProfileError),
    Package(PackageError),
    Process(OwnedProcessError),
    Rest(OwnedRestError),
}
impl fmt::Display for NativeConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native_config_{self:?}")
    }
}
impl std::error::Error for NativeConfigError {}
impl From<ProfileError> for NativeConfigError {
    fn from(error: ProfileError) -> Self {
        Self::Profile(error)
    }
}

/// Backend-selected nonzero numeric IPv4 loopback ports. Port collision is an
/// explicit startup failure, never port probing or adopting an existing server.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NativePorts {
    pub(super) api: u16,
    pub(super) sync: u16,
}
impl NativePorts {
    pub fn new(api: u16, sync: u16) -> Result<Self, NativeConfigError> {
        let ports = Self { api, sync };
        ports.validate()?;
        Ok(ports)
    }
    pub(super) fn validate(self) -> Result<(), NativeConfigError> {
        if self.api == 0 || self.sync == 0 || self.api == self.sync {
            return Err(NativeConfigError::InvalidPorts);
        }
        Ok(())
    }
    pub fn api_endpoint(self) -> std::net::SocketAddr {
        std::net::SocketAddr::from(([127, 0, 0, 1], self.api))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(windows)]
struct Intent {
    schema: u32,
    ports: NativePorts,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(windows)]
struct Receipt {
    schema: u32,
    ports: NativePorts,
    native_id: String,
    cert_blake3: String,
    key_blake3: String,
}

/// Holds both the root profile session and private native-home directory. A
/// launched session borrows this exclusively, preventing two concurrent starts.
pub struct PreparedNativeProfile<'profile> {
    #[cfg(windows)]
    profile: &'profile mut ManagedProfile,
    #[cfg(windows)]
    home: Arc<PrivateDirectory>,
    #[cfg(windows)]
    receipt: Receipt,
    #[cfg(windows)]
    _root_pin: Arc<PrivateDirectory>,
    #[cfg(windows)]
    in_use: bool,
    #[cfg(windows)]
    cleanup_unresolved: bool,
    #[cfg(not(windows))]
    _profile: std::marker::PhantomData<&'profile mut ManagedProfile>,
}
impl fmt::Debug for PreparedNativeProfile<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PreparedNativeProfile(<private backend state>)")
    }
}

impl ManagedProfile {
    /// Worker reopen only: never enter provisioning/generation for an incomplete
    /// profile. Existing ready state is still fully validated by prepare_native.
    pub fn reopen_native<'profile, S: SecureStore + ?Sized>(
        &'profile mut self,
        package: &PinnedSyncthingPackage,
        store: &S,
        ports: NativePorts,
    ) -> Result<PreparedNativeProfile<'profile>, NativeConfigError> {
        ports.validate()?;
        let _ = package;
        #[cfg(not(windows))]
        {
            let _ = store;
            Err(NativeConfigError::UnsupportedPlatform)
        }
        #[cfg(windows)]
        {
            // No call into provisioning, including if a file disappears between
            // reads. Missing markers/receipt always refuse without creating it.
            if !self.is_ready()?
                || self.read("native-generation-started")?.as_deref()
                    != Some(b"m8-native-generation-started-v1\n")
                || self.read("native-generation-complete")?.as_deref()
                    != Some(b"m8-native-generation-complete-v1\n")
            {
                return Err(NativeConfigError::InvalidState);
            }
            let intent: Intent = decode(
                &self
                    .read("native-init.json")?
                    .ok_or(NativeConfigError::InvalidState)?,
            )?;
            let receipt: Receipt = decode(
                &self
                    .read("native-ready.json")?
                    .ok_or(NativeConfigError::InvalidState)?,
            )?;
            if intent.schema != 1
                || intent.ports != ports
                || receipt.schema != 1
                || receipt.ports != ports
            {
                return Err(NativeConfigError::InvalidState);
            }
            let home = Arc::new(self.directory.child("native", false)?);
            let prepared = PreparedNativeProfile {
                _root_pin: self.directory.clone(),
                profile: self,
                home,
                receipt,
                in_use: false,
                cleanup_unresolved: false,
            };
            prepared.validate(store)?;
            Ok(prepared)
        }
    }

    /// Explicit initialization/reopen only. No native key is generated again
    /// after an attempted generation with incomplete files. No existing config
    /// or private key is overwritten, repaired, imported or reset.
    pub fn prepare_native<'profile, S: SecureStore + ?Sized>(
        &'profile mut self,
        package: &PinnedSyncthingPackage,
        store: &S,
        ports: NativePorts,
    ) -> Result<PreparedNativeProfile<'profile>, NativeConfigError> {
        ports.validate()?;
        #[cfg(not(windows))]
        {
            let _ = (package, store);
            Err(NativeConfigError::UnsupportedPlatform)
        }
        #[cfg(windows)]
        {
            let root_pin = self.directory.clone();
            self.prepare_with_generator(store, ports, |generation| {
                let mut generated = package
                    .spawn_generate(generation.path())
                    .map_err(NativeConfigError::Package)?;
                let exit = match generated.wait_for_exit(Duration::from_secs(15)) {
                    Ok(exit) => exit,
                    Err(error) => {
                        if let Err(cleanup) = generated.force_cleanup(Duration::from_secs(3)) {
                            std::mem::forget(root_pin);
                            std::mem::forget(generation.clone());
                            eprintln!("component=managed_transport operation=generation_cleanup status=failed profile_pins=retained error={cleanup}");
                        }
                        return Err(NativeConfigError::Process(error));
                    }
                };
                if !exit.is_clean() {
                    return Err(NativeConfigError::GenerationFailed);
                }
                Ok(())
            })
        }
    }

    #[cfg(windows)]
    fn prepare_with_generator<'profile, S: SecureStore + ?Sized>(
        &'profile mut self,
        store: &S,
        ports: NativePorts,
        generate: impl FnOnce(&Arc<PrivateDirectory>) -> Result<(), NativeConfigError>,
    ) -> Result<PreparedNativeProfile<'profile>, NativeConfigError> {
        ports.validate()?;
        let intent = self.read("native-init.json")?;
        match intent {
            None => {
                if self.binding()?.is_some()
                    || self.is_ready()?
                    || self.read("native-ready.json")?.is_some()
                    || ["generation", "native"]
                        .iter()
                        .any(|name| self.directory.path().join(name).exists())
                {
                    return Err(NativeConfigError::InvalidState);
                }
                self.write_new("native-init.json", &json(&Intent { schema: 1, ports })?)?;
            }
            Some(bytes) => {
                let intent: Intent = decode(&bytes)?;
                if intent.schema != 1 || intent.ports != ports {
                    return Err(NativeConfigError::InvalidState);
                }
            }
        }
        if let Some(bytes) = self.read("native-ready.json")? {
            if self.read("native-generation-started")?.as_deref()
                != Some(b"m8-native-generation-started-v1\n")
                || self.read("native-generation-complete")?.as_deref()
                    != Some(b"m8-native-generation-complete-v1\n")
            {
                return Err(NativeConfigError::InvalidState);
            }
            let receipt: Receipt = decode(&bytes)?;
            if receipt.schema != 1 || receipt.ports != ports {
                return Err(NativeConfigError::InvalidState);
            }
            let home = Arc::new(self.directory.child("native", false)?);
            let prepared = PreparedNativeProfile {
                _root_pin: self.directory.clone(),
                profile: self,
                home,
                receipt,
                in_use: false,
                cleanup_unresolved: false,
            };
            prepared.validate(store)?;
            return Ok(prepared);
        }
        let generation = Arc::new(self.directory.child("generation", true)?);
        // Metadata-only inventory; never follow an unknown child object.
        for entry in std::fs::read_dir(generation.path()).map_err(|_| ProfileError::Io)? {
            let entry = entry.map_err(|_| ProfileError::Io)?;
            if !["session.lock", "config.xml", "cert.pem", "key.pem"]
                .iter()
                .any(|name| entry.file_name() == *name)
            {
                return Err(NativeConfigError::InvalidState);
            }
        }
        let started = self.read("native-generation-started")?;
        let completed = self.read("native-generation-complete")?;
        if let Some(marker) = &started {
            if marker != b"m8-native-generation-started-v1\n" {
                return Err(NativeConfigError::InvalidState);
            }
            if completed.as_deref() != Some(b"m8-native-generation-complete-v1\n") {
                return Err(NativeConfigError::IncompleteGeneration);
            }
        } else {
            if completed.is_some() {
                return Err(NativeConfigError::InvalidState);
            }
            for name in ["config.xml", "cert.pem", "key.pem"] {
                if generation.open_read(name)?.is_some() {
                    return Err(NativeConfigError::InvalidState);
                }
            }
            // Durable intent precedes process launch. A crash in this tiny
            // gap fails closed instead of regenerating a possible identity.
            self.write_new(
                "native-generation-started",
                b"m8-native-generation-started-v1\n",
            )?;
            generate(&generation)?;
        }
        let raw = read_required(&generation, "config.xml", 256 * 1024)?;
        let cert = read_required(&generation, "cert.pem", 16 * 1024)?;
        let key = read_required(&generation, "key.pem", 16 * 1024)?;
        if !cert.starts_with(b"-----BEGIN CERTIFICATE-----\n")
            || !key.starts_with(b"-----BEGIN PRIVATE KEY-----\n")
        {
            return Err(NativeConfigError::InvalidState);
        }
        let config = native_xml::inspect(&raw, ports, true)?;
        if completed.is_none() {
            // Completion is recorded only after clean owned-tree exit AND
            // bounded input validation. A crash before this receipt is not
            // inferred successful from the presence of three files.
            self.write_new(
                "native-generation-complete",
                b"m8-native-generation-complete-v1\n",
            )?;
        }
        // Prepared files are new immutable staging outputs. Resumption
        // accepts only byte-identical existing files with private ACLs;
        // no rename/replace race or in-place edit of generated data.
        let home = Arc::new(self.directory.child("native", true)?);
        for entry in std::fs::read_dir(home.path()).map_err(|_| ProfileError::Io)? {
            let entry = entry.map_err(|_| ProfileError::Io)?;
            if !["session.lock", "config.xml", "cert.pem", "key.pem"]
                .iter()
                .any(|name| entry.file_name() == *name)
            {
                return Err(NativeConfigError::InvalidState);
            }
        }
        ensure_same_or_new(&home, "cert.pem", &cert)?;
        ensure_same_or_new(&home, "key.pem", &key)?;
        ensure_same_or_new(&home, "config.xml", &config.bytes)?;
        self.enroll_native_api_key(store, &config.key)?;
        let receipt = Receipt {
            schema: 1,
            ports,
            native_id: config.native_id,
            cert_blake3: blake3::hash(&cert).to_hex().to_string(),
            key_blake3: blake3::hash(&key).to_hex().to_string(),
        };
        self.write_new("native-ready.json", &json(&receipt)?)?;
        let prepared = PreparedNativeProfile {
            _root_pin: self.directory.clone(),
            profile: self,
            home,
            receipt,
            in_use: false,
            cleanup_unresolved: false,
        };
        prepared.validate(store)?;
        Ok(prepared)
    }
}

#[cfg(windows)]
fn json(value: &impl Serialize) -> Result<Vec<u8>, NativeConfigError> {
    serde_json::to_vec(value).map_err(|_| NativeConfigError::InvalidState)
}
#[cfg(windows)]
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, NativeConfigError> {
    cryptex_events::reject_duplicate_json_keys(
        std::str::from_utf8(bytes).map_err(|_| NativeConfigError::InvalidState)?,
    )
    .map_err(|_| NativeConfigError::InvalidState)?;
    serde_json::from_slice(bytes).map_err(|_| NativeConfigError::InvalidState)
}
#[cfg(windows)]
fn read_required(
    home: &PrivateDirectory,
    name: &str,
    max: usize,
) -> Result<Vec<u8>, NativeConfigError> {
    let file = home
        .open_read(name)?
        .ok_or(NativeConfigError::IncompleteGeneration)?;
    let mut bytes = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ProfileError::Io)?;
    if bytes.is_empty() || bytes.len() > max {
        return Err(NativeConfigError::InvalidState);
    }
    Ok(bytes)
}
#[cfg(windows)]
fn ensure_same_or_new(
    home: &PrivateDirectory,
    name: &str,
    bytes: &[u8],
) -> Result<(), NativeConfigError> {
    if home.open_read(name)?.is_some() {
        if read_required(home, name, bytes.len())? != bytes {
            return Err(NativeConfigError::InvalidState);
        }
    } else {
        let mut file = home.create_file(name)?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| ProfileError::Io)?;
    }
    Ok(())
}

#[cfg(windows)]
impl<'profile> PreparedNativeProfile<'profile> {
    fn require_idle(&self) -> Result<(), NativeConfigError> {
        if self.cleanup_unresolved {
            return Err(NativeConfigError::UnresolvedCleanup);
        }
        if self.in_use {
            return Err(NativeConfigError::Profile(ProfileError::Busy));
        }
        Ok(())
    }
    fn validate<S: SecureStore + ?Sized>(&self, store: &S) -> Result<(), NativeConfigError> {
        let cert = read_required(&self.home, "cert.pem", 16 * 1024)?;
        let key = read_required(&self.home, "key.pem", 16 * 1024)?;
        if blake3::hash(&cert).to_hex().as_str() != self.receipt.cert_blake3
            || blake3::hash(&key).to_hex().as_str() != self.receipt.key_blake3
        {
            return Err(NativeConfigError::IdentityChanged);
        }
        let config = native_xml::inspect(
            &read_required(&self.home, "config.xml", 256 * 1024)?,
            self.receipt.ports,
            false,
        )?;
        if config.native_id != self.receipt.native_id {
            return Err(NativeConfigError::IdentityChanged);
        }
        let binding = self
            .profile
            .binding()?
            .ok_or(NativeConfigError::InvalidState)?;
        if blake3::hash(&config.key).to_hex().as_str() != binding.api_key_blake3 {
            return Err(NativeConfigError::CredentialChanged);
        }
        self.profile.load_api_credential(store)?;
        Ok(())
    }

    pub fn start<'session, S: SecureStore + ?Sized>(
        &'session mut self,
        package: &PinnedSyncthingPackage,
        store: &S,
    ) -> Result<NativeSession<'session, 'profile>, NativeConfigError> {
        self.require_idle()?;
        self.validate(store)?;
        let owned = package
            .spawn_serve(self.home.path())
            .map_err(NativeConfigError::Package)?;
        self.in_use = true;
        let mut session = NativeSession {
            owned,
            _root_pin: self.profile.directory.clone(),
            _home_pin: self.home.clone(),
            prepared: self,
        };
        // A returned session has passed local version/identity/key checks on
        // the owned socket. This is transport readiness, never peer trust or
        // incoming canonical-data authorization. Use a blocking backend worker.
        let deadline = std::time::Instant::now() + Duration::from_secs(12);
        loop {
            if session.try_exit()?.is_some() {
                return Err(NativeConfigError::StartExited);
            }
            let remaining = deadline
                .checked_duration_since(std::time::Instant::now())
                .filter(|d| !d.is_zero())
                .ok_or(NativeConfigError::StartDeadline)?;
            match session
                .rest(store)?
                .observe_with_budget(remaining.min(Duration::from_secs(3)))
            {
                Ok(_) => return Ok(session),
                Err(
                    OwnedRestError::Endpoint(
                        crate::syncthing_owned_rest::EndpointError::Unavailable,
                    )
                    | OwnedRestError::Unavailable
                    | OwnedRestError::Deadline,
                ) => {
                    let remaining = deadline
                        .checked_duration_since(std::time::Instant::now())
                        .filter(|d| !d.is_zero())
                        .ok_or(NativeConfigError::StartDeadline)?;
                    std::thread::sleep(remaining.min(Duration::from_millis(50)));
                }
                Err(error) => return Err(NativeConfigError::Rest(error)),
            }
        }
    }
}

#[cfg(windows)]
pub struct NativeSession<'session, 'profile> {
    owned: PackagedProcess,
    // Independent ownership, not just a borrowed profile: mem::forget of this
    // session retains the exclusion locks rather than permitting another owner.
    _root_pin: Arc<PrivateDirectory>,
    _home_pin: Arc<PrivateDirectory>,
    prepared: &'session mut PreparedNativeProfile<'profile>,
}
#[cfg(windows)]
impl NativeSession<'_, '_> {
    /// Test-only fault injection targets this retained owned job, never a PID
    /// lookup or an unrelated/user-global process. Not exposed in product builds.
    #[cfg(test)]
    pub(super) fn crash_owned_job_for_test(&mut self) -> Result<(), NativeConfigError> {
        let exit = self
            .owned
            .force_cleanup(Duration::from_secs(3))
            .map_err(NativeConfigError::Process)?;
        assert!(exit.forced && !exit.is_clean());
        Ok(())
    }

    pub fn pid(&self) -> u32 {
        self.owned.process().pid()
    }
    pub fn try_exit(&mut self) -> Result<Option<ExitObservation>, NativeConfigError> {
        self.owned.try_exit().map_err(NativeConfigError::Process)
    }
    fn rest<S: SecureStore + ?Sized>(
        &self,
        store: &S,
    ) -> Result<OwnedSyncthingRest<'_>, NativeConfigError> {
        let credential = self.prepared.profile.load_api_credential(store)?;
        OwnedSyncthingRest::new(
            self.owned.process(),
            self.prepared.receipt.ports.api_endpoint(),
            &self.prepared.receipt.native_id,
            credential,
        )
        .map_err(NativeConfigError::Rest)
    }
    pub fn observe<S: SecureStore + ?Sized>(
        &self,
        store: &S,
    ) -> Result<crate::syncthing_rest::TransportObservation, NativeConfigError> {
        self.rest(store)?.observe().map_err(NativeConfigError::Rest)
    }
    pub fn stop<S: SecureStore + ?Sized>(
        &mut self,
        store: &S,
    ) -> Result<ExitObservation, NativeConfigError> {
        self.rest(store)?
            .request_shutdown()
            .map_err(NativeConfigError::Rest)?;
        let exit = self
            .owned
            .wait_for_exit(Duration::from_secs(25))
            .map_err(NativeConfigError::Process)?;
        if !exit.is_clean() {
            return Err(NativeConfigError::UncleanShutdown);
        }
        let log = read_required(&self.prepared.home, "native.log", 1024 * 1024)?;
        let text = std::str::from_utf8(&log).map_err(|_| NativeConfigError::ShutdownDiagnostic)?;
        if text.contains("Database failed to stop within 10s") {
            return Err(NativeConfigError::ShutdownDiagnostic);
        }
        Ok(exit)
    }
}
#[cfg(windows)]
impl Drop for NativeSession<'_, '_> {
    fn drop(&mut self) {
        if matches!(self.owned.try_exit(), Ok(Some(_))) {
            self.prepared.in_use = false;
            return;
        }
        match self.owned.force_cleanup(Duration::from_secs(3)) {
            Ok(exit) => {
                self.prepared.in_use = false;
                eprintln!(
                    "component=managed_transport operation=native_drop_cleanup status=forced exit_code={:?}",
                    exit.exit_code
                );
            }
            Err(error) => {
                self.prepared.cleanup_unresolved = true;
                // Retain BOTH private session locks on unresolved OS cleanup, not
                // only the executable pin. A new owner cannot reopen this profile.
                std::mem::forget(self.prepared.home.clone());
                std::mem::forget(self.prepared.profile.directory.clone());
                eprintln!(
                    "component=managed_transport operation=native_drop_cleanup status=failed profile_pins=retained error={error}"
                );
            }
        }
    }
}

#[cfg(all(test, windows))]
#[path = "native_tests.rs"]
pub(super) mod tests;
