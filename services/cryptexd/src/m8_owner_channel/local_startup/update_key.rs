//! Single-use credential check for a verified update. A fresh daemon checks
//! the fixed disposable profile. Neither the renderer nor the updater gets
//! a key, an owner session, or a caller-selected profile path.
use super::*;
use security_framework::os::macos::keychain::SecKeychain;

pub(super) const INTERACTIVE: u8 = 6;
pub(super) const SILENT: u8 = 7;
const VERIFIED: u8 = 8;
const REFUSED: u8 = 9;
const TRANSPORT_STAGED: u8 = 10;

// Both signed peers must allow time for the user's decision or a cold database
// copy, including when only the packaged daemon enables transport migration.
const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(120);

impl LocalDaemon {
    pub fn check_update_credential(
        mut self,
        allow_prompt: bool,
    ) -> Result<Option<[u8; 32]>, Error> {
        if self.bound || self.existing_profile || self.transport_mode {
            return Err(Error::Arguments);
        }
        let binding = *self.content()?.binding();
        let deadline = Instant::now() + UPDATE_CHECK_TIMEOUT;
        let channel = self.channel.as_mut().ok_or(Error::Channel)?;
        let phase = if allow_prompt { INTERACTIVE } else { SILENT };
        channel.send(&frame(phase, &self.epoch, &binding), deadline)?;
        let mut message = [0; 86];
        let reply = channel.receive(&mut message, deadline)?;
        let result = if reply == frame(VERIFIED, &self.epoch, &binding) {
            Ok(None)
        } else if !allow_prompt
            && reply.len() == 86
            && reply[..54] == frame(TRANSPORT_STAGED, &self.epoch, &binding)
        {
            Ok(Some(reply[54..].try_into().map_err(|_| Error::Channel)?))
        } else {
            Err(Error::Unavailable)
        };
        // The daemon has finished its one read and enters normal lifecycle
        // shutdown. This check never bound an owner request channel.
        self.content_mode = false;
        let stop = self.stop_file.clone();
        self.shutdown(&stop)?;
        result
    }
}

impl ChildGuard {
    pub(super) fn reply_to_update_key_check(
        &mut self,
        binding: &[u8; 32],
        allow_prompt: bool,
    ) -> Result<(), Error> {
        let deadline = Instant::now() + UPDATE_CHECK_TIMEOUT;
        let result = match verify_existing_key(allow_prompt) {
            Ok(None) => frame(VERIFIED, &self.epoch, binding),
            Ok(Some(fingerprint)) => {
                let mut reply = frame(TRANSPORT_STAGED, &self.epoch, binding);
                reply.extend_from_slice(&fingerprint);
                reply
            }
            Err(_) => frame(REFUSED, &self.epoch, binding),
        };
        self._channel.send(&result, deadline)?;
        Ok(())
    }
}

fn verify_existing_key(allow_prompt: bool) -> Result<Option<[u8; 32]>, Error> {
    use crate::import_staging::macos_fs::RuntimeIdentity;
    let root = sealed_trial_root()?;
    require_content_runtime(&root, ContentProfile::ExistingProfile)?;
    let namespace = RuntimeIdentity::capture(&root).map_err(|_| Error::Instance)?;
    namespace
        .pin(&root.join("canonical/events"))
        .map_err(|_| Error::Instance)?;
    let lock = crate::instance::try_acquire_instance(&root).map_err(|_| Error::Instance)?;
    // Process-local Security Framework policy in this short-lived daemon.
    // The unattended check gets no opportunity to ask the user a second time.
    let _silence = if allow_prompt {
        None
    } else {
        Some(SecKeychain::disable_user_interaction().map_err(|_| Error::Unavailable)?)
    };
    let store = crate::MacosKeychainStore::new();
    let environment = crate::secure_store::SecureStoreEnvironment::Dev;
    crate::ipc::native_profile::bind_verified_namespace(
        &root,
        &lock,
        &namespace,
        &store,
        environment,
    )
    .map_err(|_| Error::Unavailable)?;
    if !allow_prompt {
        // An enrolled profile also needs the existing transport credential.
        // Use its ordinary read-only reopen check in the noninteractive daemon.
        let directory = crate::import_staging::export_fs::Directory::open(&root)
            .map_err(|_| Error::Instance)?;
        if directory
            .exists("managed-transport")
            .map_err(|_| Error::Instance)?
        {
            namespace
                .pin(&root.join("managed-transport"))
                .map_err(|_| Error::Instance)?;
            use crate::managed_profile::connection_settings::Location;
            let location = Location::mac(&root);
            #[cfg(feature = "m17-owner-transport")]
            if location.pin_enrolled_package().is_err() {
                let previous = root.join("managed-transport/package");
                crate::syncthing_package::verify_previous_owner_package(&previous)
                    .map_err(|_| Error::Unavailable)?;
                let source = tempfile::tempdir().map_err(|_| Error::Instance)?;
                let release =
                    Release::current(Role::Daemon, Instant::now() + UPDATE_CHECK_TIMEOUT)?;
                let package =
                    crate::m8_owner_channel::transport_helper::package(&release, source.path())
                        .map_err(|_| Error::Unavailable)?;
                drop(lock);
                let prepared =
                    Location::stage_stopped_owner_transport(&root, &package, &store, environment)
                        .map_err(|_| Error::Unavailable)?;
                directory.validate().map_err(|_| Error::Instance)?;
                namespace.validate().map_err(|_| Error::Instance)?;
                return Ok(Some(prepared.fingerprint()));
            }
            location
                .candidate(&store, environment)
                .map_err(|_| Error::Unavailable)?;
        }
        directory.validate().map_err(|_| Error::Instance)?;
    }
    namespace.validate().map_err(|_| Error::Instance)?;
    Ok(None)
}
