//! Recoverable derived configuration persistence. NOT a launch/admission permit.
//!
//! The future controller must retain runtime admission, validate the original
//! native identity/OS credential, stop the owned engine and prove incoming-path
//! containment before calling this boundary. Bootstrap startup refuses its files.
//! A journal never supplies trust: every attempt recompiles against a current
//! ExchangeConfigPolicy. No canonical files are read or written here.
use std::io::Read;

use cryptex_events::reject_duplicate_json_keys;
use serde::{Deserialize, Serialize};

use super::native_xml::exchange::{ExchangeConfigError, ExchangeConfigPolicy};
use super::{ProfileError, platform::PrivateDirectory};

const MAX_JOURNAL: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Leaf {
    Config,
    Binding,
    Journal,
}
impl Leaf {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Config => "config.xml",
            Self::Binding => "exchange-binding.json",
            Self::Journal => "exchange-install.json",
        }
    }
    pub(super) fn temporary(self) -> &'static str {
        match self {
            Self::Config => "exchange-config.tmp",
            Self::Binding => "exchange-binding.tmp",
            Self::Journal => "exchange-install.tmp",
        }
    }
    pub(super) fn limit(self) -> usize {
        match self {
            Self::Config => 256 * 1024,
            Self::Binding => 64 * 1024,
            Self::Journal => MAX_JOURNAL,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InstallError {
    InvalidJournal,
    UnrecognizedState,
    CapacityExceeded,
    ReadbackMismatch,
    Policy(ExchangeConfigError),
    Storage(ProfileError),
}
impl From<ProfileError> for InstallError {
    fn from(error: ProfileError) -> Self {
        Self::Storage(error)
    }
}
impl From<ExchangeConfigError> for InstallError {
    fn from(error: ExchangeConfigError) -> Self {
        Self::Policy(error)
    }
}
type Result<T> = std::result::Result<T, InstallError>;

// Implemented only by fresh canonical policy and its private authenticated
// transport command. A disk journal can never implement this interface.
pub(super) trait InstallPolicy {
    fn prepare_bootstrap(&self, bytes: &[u8]) -> std::result::Result<Vec<u8>, ExchangeConfigError>;
    fn binding_bytes(&self, config: &[u8]) -> std::result::Result<Vec<u8>, ExchangeConfigError>;
    fn verify_binding(
        &self,
        config: &[u8],
        binding: &[u8],
    ) -> std::result::Result<(), ExchangeConfigError>;
}
impl InstallPolicy for ExchangeConfigPolicy<'_> {
    fn prepare_bootstrap(&self, bytes: &[u8]) -> std::result::Result<Vec<u8>, ExchangeConfigError> {
        self.prepare_bootstrap(bytes)
    }
    fn binding_bytes(&self, bytes: &[u8]) -> std::result::Result<Vec<u8>, ExchangeConfigError> {
        self.binding_bytes(bytes)
    }
    fn verify_binding(
        &self,
        config: &[u8],
        binding: &[u8],
    ) -> std::result::Result<(), ExchangeConfigError> {
        self.verify_binding(config, binding)
    }
}
pub(super) fn install_native(home: &PrivateDirectory, policy: &impl InstallPolicy) -> Result<()> {
    install(&mut { home }, policy)
}
pub(super) fn verify_native(home: &PrivateDirectory, policy: &impl InstallPolicy) -> Result<()> {
    let journal = Journal::decode(
        &(&home)
            .read(Leaf::Journal)?
            .ok_or(InstallError::InvalidJournal)?,
    )?;
    if journal.phase != Phase::Committed
        || journal.native_home != (&home).home()?
        || policy.prepare_bootstrap(journal.bootstrap.as_bytes())? != journal.config.as_bytes()
    {
        return Err(InstallError::InvalidJournal);
    }
    verify_committed(&home, &journal, &journal.encode()?, policy)
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Pending,
    Committed,
}

// Strings, not Vec<u8> JSON arrays: bounded XML expands by at most the JSON
// escaping factor and an oversized encoding is refused before any write.
// No Debug: native config contains its local REST credential.
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema: u32,
    phase: Phase,
    native_home: String,
    bootstrap: String,
    config: String,
    binding: String,
}
impl Journal {
    fn encode(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec(self).map_err(|_| InstallError::InvalidJournal)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_JOURNAL {
            return Err(InstallError::CapacityExceeded);
        }
        Ok(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_JOURNAL {
            return Err(InstallError::CapacityExceeded);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| InstallError::InvalidJournal)?;
        reject_duplicate_json_keys(text).map_err(|_| InstallError::InvalidJournal)?;
        let result: Self = serde_json::from_str(text).map_err(|_| InstallError::InvalidJournal)?;
        if result.schema != 1
            || result.bootstrap.len() > Leaf::Config.limit()
            || result.config.len() > Leaf::Config.limit()
            || result.binding.len() > Leaf::Binding.limit()
        {
            return Err(InstallError::InvalidJournal);
        }
        Ok(result)
    }
}

/// A private abstraction for fault injection, never an arbitrary path API.
trait Files {
    fn home(&self) -> Result<String>;
    fn read(&self, leaf: Leaf) -> Result<Option<Vec<u8>>>;
    fn stage(&self, leaf: Leaf) -> Result<Option<Vec<u8>>>;
    fn replace(&mut self, leaf: Leaf, bytes: &[u8]) -> Result<()>;
    fn sync_committed(&mut self) -> Result<()>;
}
impl Files for &PrivateDirectory {
    fn home(&self) -> Result<String> {
        self.path()
            .to_str()
            .filter(|s| s.len() <= 4096)
            .map(str::to_owned)
            .ok_or(InstallError::InvalidJournal)
    }
    fn read(&self, leaf: Leaf) -> Result<Option<Vec<u8>>> {
        read_bounded(self, leaf.name(), leaf.limit())
    }
    fn stage(&self, leaf: Leaf) -> Result<Option<Vec<u8>>> {
        read_bounded(self, leaf.temporary(), leaf.limit())
    }
    fn replace(&mut self, leaf: Leaf, bytes: &[u8]) -> Result<()> {
        if bytes.len() > leaf.limit() {
            return Err(InstallError::CapacityExceeded);
        }
        self.replace_exchange_file(leaf, bytes)?;
        if self.read(leaf)?.as_deref() != Some(bytes) {
            return Err(InstallError::ReadbackMismatch);
        }
        Ok(())
    }
    fn sync_committed(&mut self) -> Result<()> {
        self.sync_exchange_files()?;
        Ok(())
    }
}

fn read_bounded(home: &PrivateDirectory, name: &str, limit: usize) -> Result<Option<Vec<u8>>> {
    let Some(file) = home.open_read(name)? else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| InstallError::Storage(ProfileError::Io))?;
    if bytes.len() > limit {
        return Err(InstallError::CapacityExceeded);
    }
    Ok(Some(bytes))
}

/// Fail closed even when a pending install still has bootstrap config on disk.
/// Called by the existing bootstrap path, which must never recover/activate it.
pub(super) fn refuse_exchange_state(
    home: &PrivateDirectory,
) -> std::result::Result<(), ProfileError> {
    for leaf in [Leaf::Config, Leaf::Binding, Leaf::Journal] {
        if home.open_read(leaf.temporary())?.is_some()
            || (leaf != Leaf::Config && home.open_read(leaf.name())?.is_some())
        {
            return Err(ProfileError::InvalidState);
        }
    }
    Ok(())
}

fn text(bytes: Vec<u8>) -> Result<String> {
    String::from_utf8(bytes).map_err(|_| InstallError::InvalidJournal)
}

/// First bootstrap-to-exchange install or recovery of that SAME install only.
/// Updating an already active roster is deliberately not inferred from a journal.
/// This private primitive does not validate OS credentials or start a process.
fn install(files: &mut impl Files, policy: &impl InstallPolicy) -> Result<()> {
    let existing = files.read(Leaf::Journal)?;
    let current = files
        .read(Leaf::Config)?
        .ok_or(InstallError::UnrecognizedState)?;
    let mut journal = if let Some(bytes) = &existing {
        Journal::decode(bytes)?
    } else {
        if files.read(Leaf::Binding)?.is_some() {
            return Err(InstallError::UnrecognizedState);
        }
        let config = policy.prepare_bootstrap(&current)?;
        let binding = policy.binding_bytes(&config)?;
        Journal {
            schema: 1,
            phase: Phase::Pending,
            native_home: files.home()?,
            bootstrap: text(current.clone())?,
            config: text(config)?,
            binding: text(binding)?,
        }
    };
    // Recompile the saved bootstrap under CURRENT authority before ANY write.
    // Unknown fields/duplicate keys, changed profile, endpoints, peers or target
    // bytes cannot be adopted by recovering a journal.
    if journal.native_home != files.home()?
        || policy.prepare_bootstrap(journal.bootstrap.as_bytes())? != journal.config.as_bytes()
    {
        return Err(InstallError::InvalidJournal);
    }
    policy.verify_binding(journal.config.as_bytes(), journal.binding.as_bytes())?;
    let binding = files.read(Leaf::Binding)?;
    let old = current == journal.bootstrap.as_bytes();
    // The owned engine may serialize equivalent XML with different formatting.
    // Fresh policy still checks the complete normalized configuration.
    let new = policy
        .verify_binding(&current, journal.binding.as_bytes())
        .is_ok();
    let bound = binding.as_deref() == Some(journal.binding.as_bytes());
    let was_committed = journal.phase == Phase::Committed;
    journal.phase = Phase::Pending;
    let pending = journal.encode()?;
    journal.phase = Phase::Committed;
    let committed = journal.encode()?; // Both encodings bounded before mutation.
    // A fully staged replacement may survive a process exit before rename.
    // Only the next write in this transaction can have such a stage. Partial,
    // foreign, multiple or out-of-order stages are retained as an explicit error.
    let mut stages = 0;
    for leaf in [Leaf::Config, Leaf::Binding, Leaf::Journal] {
        if let Some(stage) = files.stage(leaf)? {
            stages += 1;
            let expected: Option<&[u8]> = if was_committed {
                None
            } else {
                match leaf {
                    Leaf::Journal if existing.is_none() && old && binding.is_none() => {
                        Some(&pending)
                    }
                    Leaf::Config if existing.is_some() && old && binding.is_none() => {
                        Some(journal.config.as_bytes())
                    }
                    Leaf::Binding if existing.is_some() && new && binding.is_none() => {
                        Some(journal.binding.as_bytes())
                    }
                    Leaf::Journal if existing.is_some() && new && bound => Some(&committed),
                    _ => None,
                }
            };
            if expected != Some(stage.as_slice()) {
                return Err(InstallError::UnrecognizedState);
            }
        }
    }
    if stages > 1 {
        return Err(InstallError::UnrecognizedState);
    }
    if was_committed {
        if !new || !bound {
            return Err(InstallError::UnrecognizedState);
        }
        // A rename may have become visible just before its flush failed.
        // Observing committed bytes is not a substitute for a new barrier.
        files.sync_committed()?;
        return verify_committed(files, &journal, &committed, policy);
    }
    // Reachable durable states: old/no binding, new/no binding, new/new binding.
    // In particular old/new binding and foreign/missing config are NOT recovery.
    if !((old || new) && binding.is_none() || new && bound) {
        return Err(InstallError::UnrecognizedState);
    }
    if existing.is_none() {
        files.replace(Leaf::Journal, &pending)?;
    }
    if !new {
        files.replace(Leaf::Config, journal.config.as_bytes())?;
    }
    if !bound {
        files.replace(Leaf::Binding, journal.binding.as_bytes())?;
    }
    if files.read(Leaf::Config)?.as_deref() != Some(journal.config.as_bytes())
        || files.read(Leaf::Binding)?.as_deref() != Some(journal.binding.as_bytes())
    {
        return Err(InstallError::ReadbackMismatch);
    }
    // No delete/cleanup window: a complete durable record remains for restart.
    files.replace(Leaf::Journal, &committed)?;
    files.sync_committed()?;
    verify_committed(files, &journal, &committed, policy)
}

fn verify_committed(
    files: &impl Files,
    journal: &Journal,
    committed: &[u8],
    policy: &impl InstallPolicy,
) -> Result<()> {
    let config = files
        .read(Leaf::Config)?
        .ok_or(InstallError::ReadbackMismatch)?;
    policy.verify_binding(&config, journal.binding.as_bytes())?;
    for (leaf, expected) in [
        (Leaf::Binding, journal.binding.as_bytes()),
        (Leaf::Journal, committed),
    ] {
        if files.read(leaf)?.as_deref() != Some(expected) || files.stage(leaf)?.is_some() {
            return Err(InstallError::ReadbackMismatch);
        }
    }
    if files.stage(Leaf::Config)?.is_some() {
        return Err(InstallError::ReadbackMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
