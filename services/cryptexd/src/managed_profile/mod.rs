//! Explicit backend-only managed-profile and API credential enrollment.
//!
//! No automatic setup, key generation, transport launch or peer authorization.
//! Windows creates the directory with its private DACL in the creation call;
//! other platforms refuse until their platform implementation is verified.
//! The caller supplies a native-generated API key, never a renderer credential.

use std::fmt;
#[cfg(windows)]
use std::io::{Read, Write};
use std::path::Path;

use cryptex_events::reject_duplicate_json_keys;
use serde::{Deserialize, Serialize};

use crate::secure_store::{SecureStore, SecureStoreEnvironment, SecureStoreError};
use crate::syncthing_rest::{ApiCredential, credential_labels};

#[cfg(any(windows, test))]
pub mod controller;
pub mod native;
#[cfg(windows)]
mod native_xml;
#[cfg(all(test, windows))]
mod tests;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub mod worker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileError {
    InvalidPath,
    UnsupportedPlatform,
    AlreadyExists,
    Busy,
    UnsafePermissions,
    UnsafeObject,
    InvalidState,
    CredentialUnavailable,
    CredentialConflict,
    Io,
    Platform { operation: &'static str, code: u32 },
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "managed_profile_{self:?}")
    }
}

impl std::error::Error for ProfileError {}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Identity {
    schema: u32,
    profile_id: String,
    environment: String,
    credential_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    schema: u32,
    api_key_blake3: String,
}

/// Owns an exclusive session lock and pinned non-reparse directory handles.
/// Not serializable, cloneable, or usable to adopt an arbitrary running process.
pub struct ManagedProfile {
    #[cfg(windows)]
    directory: std::sync::Arc<windows::PrivateDirectory>,
    identity: Identity,
    environment: SecureStoreEnvironment,
}

impl fmt::Debug for ManagedProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ManagedProfile(<private backend profile>)")
    }
}

impl ManagedProfile {
    /// Explicit initialization only. Parent must already exist outside Git.
    /// Leaf is a new opaque 32-character lower-case hexadecimal profile ID.
    /// Existing objects are refused; permissions are never silently repaired.
    pub fn create(path: &Path, environment: SecureStoreEnvironment) -> Result<Self, ProfileError> {
        Self::acquire(path, environment, true)
    }

    /// Reopen an existing profile. Missing/corrupt identity is never recreated.
    pub fn open(path: &Path, environment: SecureStoreEnvironment) -> Result<Self, ProfileError> {
        Self::acquire(path, environment, false)
    }

    fn acquire(
        path: &Path,
        environment: SecureStoreEnvironment,
        create: bool,
    ) -> Result<Self, ProfileError> {
        let profile_id = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| {
                name.len() == 32
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
            .ok_or(ProfileError::InvalidPath)?;
        let identity = Identity {
            schema: 1,
            profile_id: profile_id.to_owned(),
            environment: environment.service_prefix().to_owned(),
            credential_id: if create {
                let mut bytes = [0; 16];
                getrandom::getrandom(&mut bytes).map_err(|_| ProfileError::Io)?;
                bytes.iter().map(|b| format!("{b:02x}")).collect()
            } else {
                String::new()
            },
        };
        #[cfg(windows)]
        {
            let directory = windows::PrivateDirectory::acquire(path, create)?;
            let mut result = Self {
                directory: std::sync::Arc::new(directory),
                identity,
                environment,
            };
            if create {
                result.write_new(
                    "profile.json",
                    &serde_json::to_vec(&result.identity)
                        .map_err(|_| ProfileError::InvalidState)?,
                )?;
            } else {
                let bytes = result
                    .read("profile.json")?
                    .ok_or(ProfileError::InvalidState)?;
                reject_duplicate_json_keys(
                    std::str::from_utf8(&bytes).map_err(|_| ProfileError::InvalidState)?,
                )
                .map_err(|_| ProfileError::InvalidState)?;
                let actual: Identity =
                    serde_json::from_slice(&bytes).map_err(|_| ProfileError::InvalidState)?;
                if actual.schema != result.identity.schema
                    || actual.profile_id != result.identity.profile_id
                    || actual.environment != result.identity.environment
                    || actual.credential_id.len() != 32
                    || !actual
                        .credential_id
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(ProfileError::InvalidState);
                }
                result.identity = actual;
            }
            Ok(result)
        }
        #[cfg(not(windows))]
        {
            let _ = (identity, create, environment);
            Err(ProfileError::UnsupportedPlatform)
        }
    }

    /// Public opaque OS-account namespace, generated independently of the path.
    /// Identically named folders under different parents cannot collide in the
    /// SecureStore. This ID is not a Brain/device identity or peer authority.
    pub fn credential_profile_id(&self) -> &str {
        &self.identity.credential_id
    }

    /// Bind a native-generated key to this profile and OS credential account.
    /// Immutable binding is flushed BEFORE the SecureStore write. A retry after
    /// interruption may complete ONLY the identical key. A ready profile with a
    /// missing key fails closed; enrollment never regenerates or replaces it.
    pub fn enroll_native_api_key<S: SecureStore + ?Sized>(
        &mut self,
        store: &S,
        key: &[u8],
    ) -> Result<ApiCredential, ProfileError> {
        if key.len() < 32 || ApiCredential::from_bytes(key).is_err() {
            return Err(ProfileError::InvalidState);
        }
        let digest = blake3::hash(key).to_hex().to_string();
        let ready = self.is_ready()?;
        let binding = self.binding()?;
        let (service, account) = self.labels()?;
        let existing = match store.get(&service, &account) {
            Ok(bytes) => Some(bytes),
            Err(SecureStoreError::Missing) => None,
            Err(_) => return Err(ProfileError::CredentialUnavailable),
        };
        match binding {
            None => {
                if ready || existing.is_some() {
                    return Err(ProfileError::CredentialConflict);
                }
                let binding = Binding {
                    schema: 1,
                    api_key_blake3: digest,
                };
                self.write_new(
                    "api-binding.json",
                    &serde_json::to_vec(&binding).map_err(|_| ProfileError::InvalidState)?,
                )?;
            }
            Some(binding) if binding.api_key_blake3 == digest => {}
            Some(_) => return Err(ProfileError::CredentialConflict),
        }
        match existing {
            Some(bytes) if bytes == key => {}
            Some(_) => return Err(ProfileError::CredentialConflict),
            None if ready => return Err(ProfileError::CredentialUnavailable),
            None => {
                store
                    .put(&service, &account, key)
                    .map_err(|_| ProfileError::CredentialUnavailable)?;
            }
        }
        // A store reporting success must return the identical bytes before we
        // publish readiness. No rollback deletes someone else's credential.
        let observed = store
            .get(&service, &account)
            .map_err(|_| ProfileError::CredentialUnavailable)?;
        if observed != key {
            return Err(ProfileError::CredentialConflict);
        }
        if !ready {
            self.write_new("api-ready", b"m8-api-ready-v1\n")?;
        }
        self.load_api_credential(store)
    }

    pub fn load_api_credential<S: SecureStore + ?Sized>(
        &self,
        store: &S,
    ) -> Result<ApiCredential, ProfileError> {
        if !self.is_ready()? {
            return Err(ProfileError::InvalidState);
        }
        let binding = self.binding()?.ok_or(ProfileError::InvalidState)?;
        let (service, account) = self.labels()?;
        let key = store
            .get(&service, &account)
            .map_err(|_| ProfileError::CredentialUnavailable)?;
        if key.len() < 32 || blake3::hash(&key).to_hex().as_str() != binding.api_key_blake3 {
            return Err(ProfileError::CredentialConflict);
        }
        ApiCredential::from_bytes(&key).map_err(|_| ProfileError::CredentialConflict)
    }

    fn labels(&self) -> Result<(String, String), ProfileError> {
        credential_labels(self.environment, &self.identity.credential_id)
            .map_err(|_| ProfileError::InvalidState)
    }

    fn binding(&self) -> Result<Option<Binding>, ProfileError> {
        self.read("api-binding.json")?
            .map(|bytes| {
                reject_duplicate_json_keys(
                    std::str::from_utf8(&bytes).map_err(|_| ProfileError::InvalidState)?,
                )
                .map_err(|_| ProfileError::InvalidState)?;
                let binding: Binding =
                    serde_json::from_slice(&bytes).map_err(|_| ProfileError::InvalidState)?;
                if binding.schema != 1
                    || binding.api_key_blake3.len() != 64
                    || !binding
                        .api_key_blake3
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(ProfileError::InvalidState);
                }
                Ok(binding)
            })
            .transpose()
    }

    fn is_ready(&self) -> Result<bool, ProfileError> {
        match self.read("api-ready")? {
            None => Ok(false),
            Some(bytes) if bytes == b"m8-api-ready-v1\n" => Ok(true),
            Some(_) => Err(ProfileError::InvalidState),
        }
    }

    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, ProfileError> {
        #[cfg(windows)]
        {
            let Some(file) = self.directory.open_read(name)? else {
                return Ok(None);
            };
            let mut bytes = Vec::new();
            file.take(4097)
                .read_to_end(&mut bytes)
                .map_err(|_| ProfileError::Io)?;
            if bytes.len() > 4096 {
                return Err(ProfileError::InvalidState);
            }
            Ok(Some(bytes))
        }
        #[cfg(not(windows))]
        {
            let _ = name;
            Err(ProfileError::UnsupportedPlatform)
        }
    }

    fn write_new(&self, name: &str, bytes: &[u8]) -> Result<(), ProfileError> {
        #[cfg(windows)]
        {
            let mut file = self.directory.create_file(name)?;
            file.write_all(bytes)
                .and_then(|()| file.sync_all())
                .map_err(|_| ProfileError::Io)
        }
        #[cfg(not(windows))]
        {
            let _ = (name, bytes);
            Err(ProfileError::UnsupportedPlatform)
        }
    }
}
