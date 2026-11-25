//! Single-use credential check for a verified update. A fresh daemon checks
//! the fixed disposable profile. Neither the renderer nor the updater gets
//! a key, an owner session, or a caller-selected profile path.
use super::*;
use security_framework::os::macos::keychain::SecKeychain;

pub(super) const INTERACTIVE: u8 = 6;
pub(super) const SILENT: u8 = 7;
const VERIFIED: u8 = 8;
const REFUSED: u8 = 9;

fn budget(allow_prompt: bool) -> Duration {
    Duration::from_secs(if allow_prompt { 120 } else { 12 })
}

impl LocalDaemon {
    pub fn check_update_credential(mut self, allow_prompt: bool) -> Result<(), Error> {
        if self.bound || self.existing_profile || self.transport_mode {
            return Err(Error::Arguments);
        }
        let binding = *self.content()?.binding();
        let deadline = Instant::now() + budget(allow_prompt);
        let channel = self.channel.as_mut().ok_or(Error::Channel)?;
        let phase = if allow_prompt { INTERACTIVE } else { SILENT };
        channel.send(&frame(phase, &self.epoch, &binding), deadline)?;
        let verified =
            channel.receive(&mut [0; 54], deadline)? == frame(VERIFIED, &self.epoch, &binding);
        // The daemon has finished its one read and enters normal lifecycle
        // shutdown. This check never bound an owner request channel.
        self.content_mode = false;
        let stop = self.stop_file.clone();
        self.shutdown(&stop)?;
        if verified {
            Ok(())
        } else {
            Err(Error::Unavailable)
        }
    }
}

impl ChildGuard {
    pub(super) fn reply_to_update_key_check(
        &mut self,
        binding: &[u8; 32],
        allow_prompt: bool,
    ) -> Result<(), Error> {
        let deadline = Instant::now() + budget(allow_prompt);
        let verified = verify_existing_key(allow_prompt).is_ok();
        let result = if verified { VERIFIED } else { REFUSED };
        self._channel
            .send(&frame(result, &self.epoch, binding), deadline)?;
        Ok(())
    }
}

fn verify_existing_key(allow_prompt: bool) -> Result<(), Error> {
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
            crate::managed_profile::connection_settings::Location::mac(&root)
                .candidate(&store, environment)
                .map_err(|_| Error::Unavailable)?;
        }
        directory.validate().map_err(|_| Error::Instance)?;
    }
    namespace.validate().map_err(|_| Error::Instance)
}
