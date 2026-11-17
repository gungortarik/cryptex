//! Windows product receive pump, owned by the existing connection lifecycle.
//! Uses that daemon's instance/admission and the same verified publisher as Mac.
use super::*;
use crate::import_staging::windows_inbox::Inbox;
use crate::incoming_publication::IncomingPublicationError;
use crate::managed_profile::incoming::{IncomingController, IncomingControllerError, IncomingTick};
use crate::runtime_admission::AdmissionError;

pub(super) struct Pump {
    inbox: Inbox,
    controller: IncomingController,
}
impl Pump {
    fn new(root: &Path, environment: SecureStoreEnvironment) -> Result<Self, Error> {
        Ok(Self {
            inbox: Inbox::new(root).map_err(|_| Error::Storage)?,
            controller: IncomingController::new(root.to_owned(), environment)
                .map_err(|_| Error::Authority)?,
        })
    }
    fn tick(
        &mut self,
        reader: &Reader,
        instance: &mut crate::instance::InstanceLock,
        store: &dyn SecureStore,
    ) -> Result<(), Error> {
        let paths = self.inbox.discover().map_err(|error| {
            eprintln!("component=incoming_discovery status=refused error={error:?}");
            Error::Storage
        })?;
        if paths.is_empty() {
            return Ok(());
        }
        self.controller.enqueue(paths).map_err(|_| Error::Storage)?;
        match self.controller.poll(reader, instance, store) {
            Ok(IncomingTick::Published(report)) => {
                self.inbox.consumed().map_err(|_| Error::Storage)?;
                eprintln!(
                    "component=incoming_publication status=committed segments={} events={} identical={}",
                    report.published_segments, report.published_events, report.identical_segments
                );
                Ok(())
            }
            Ok(IncomingTick::Idle | IncomingTick::Deferred | IncomingTick::WaitingForTransport) => {
                Ok(())
            }
            Err(IncomingControllerError::Publication(IncomingPublicationError::Rejected(_))) => {
                self.inbox.consumed().map_err(|_| Error::Storage)?;
                // Untrusted input is not a canonical hold. Preserve it, report
                // the refusal without raw paths/content and avoid a hot retry.
                eprintln!("component=incoming_publication status=rejected");
                Ok(())
            }
            Err(error) => {
                eprintln!("component=incoming_publication status=refused error={error}");
                Err(Error::Held)
            }
        }
    }
}
impl Controller {
    pub(super) fn check_incoming(&mut self) {
        if self.view.state != State::ReadyLocally
            || self.view.error.is_some()
            || self.stop.load(Ordering::Acquire)
            || Instant::now() < self.next_incoming
        {
            return;
        }
        self.next_incoming = Instant::now() + Duration::from_secs(5);
        let result = self.receive_incoming();
        if let Err(error) = result {
            self.view.error = Some(self.close().err().unwrap_or(error));
            self.view.state = State::Attention;
        }
    }
    fn receive_incoming(&mut self) -> Result<(), Error> {
        let owner = self.recovery_owner.as_ref().ok_or(Error::Authority)?;
        let mut instance = match owner.try_lock() {
            Ok(instance) => instance,
            Err(std::sync::TryLockError::WouldBlock) => return Ok(()),
            Err(_) => return Err(Error::Held),
        };
        // No filesystem discovery during pause, drain, recovery or policy hold.
        // Drop the ordinary lease before the publisher requests exclusivity.
        match self.admission.enter(&self.root) {
            Ok(permit) => drop(permit),
            Err(AdmissionError::Busy) => return Ok(()),
            Err(_) => return Err(Error::Held),
        }
        let reader = self
            .reader
            .lock()
            .map_err(|_| Error::Unavailable)?
            .clone()
            .ok_or(Error::Unavailable)?;
        if reader.try_transport_device_id().is_err() {
            return Ok(());
        }
        if self.incoming.is_none() {
            self.incoming = Some(Pump::new(&self.root, self.environment)?);
        }
        self.incoming.as_mut().ok_or(Error::Unavailable)?.tick(
            &reader,
            &mut instance,
            self.store.as_ref(),
        )
    }
}

#[cfg(test)]
mod tests;
