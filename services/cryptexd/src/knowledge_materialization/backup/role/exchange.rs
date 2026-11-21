//! Public files for moving the scheduled writer role on an existing target.
use super::*;
use crate::ipc::backup_writer::handoff::{ResultView, Review, Step};
use crate::knowledge_materialization::backup::{manual, restore};
use serde::de::DeserializeOwned;

const MAX_FILE: usize = 1024 * 1024;

fn selected<T: DeserializeOwned>(target: &Path, path: &Path) -> Result<T, Error> {
    let parent = path.parent().ok_or(Error::Target)?;
    if !path.is_absolute() || !same_file::is_same_file(parent, target).map_err(|_| Error::Target)? {
        return Err(Error::Target);
    }
    let bytes = restore::read_selected(path, MAX_FILE as u64).map_err(|_| Error::Target)?;
    serde_json::from_slice(&bytes).map_err(|_| Error::History)
}

fn save(dir: &Directory, value: &impl Serialize, extension: &str) -> Result<ResultView, Error> {
    let bytes = encoded(value)?;
    if bytes.len() > MAX_FILE {
        return Err(Error::History);
    }
    let id = manual::export_id().map_err(|_| Error::Target)?;
    let file =
        manual::publish_file(dir, id, extension, bytes.as_slice()).map_err(|_| Error::Target)?;
    Ok(ResultView::Saved { file })
}

impl DeviceRole<'_> {
    fn review_request(
        &self,
        profile: &Path,
        dir: &Directory,
        assignment: &Assignment,
        request: &Request,
    ) -> Result<Review, Error> {
        let id = marker(dir)?;
        let (head, current) = assignment.verify(id)?;
        if *current != Writer::from(self.local)
            || request.prior != head
            || current.device == request.next.device
            || current.key == request.next.key
        {
            return Err(Error::Authority);
        }
        self.check_authority(profile, &assignment.origin.brain, Some(&request.next))?;
        Ok(Review {
            target_id: format!("{:x}", sha2::digest::Output::<Sha256>::from(id)),
            outgoing_device: current.device.as_str().to_owned(),
            outgoing_fingerprint: format!("{:x}", Sha256::digest(current.key)),
            incoming_device: request.next.device.as_str().to_owned(),
            incoming_fingerprint: format!("{:x}", Sha256::digest(request.next.key)),
            request_sha256: format!("{:x}", Sha256::digest(encoded(request)?)),
        })
    }

    pub fn exchange(&self, profile: &Path, target: &Path, step: Step) -> Result<ResultView, Error> {
        let dir = Directory::open(target).map_err(|_| Error::Target)?;
        match step {
            Step::Share => {
                self.verify_local_writer(profile, target)?;
                save(&dir, &self.reopen(target)?, "writer-assignment.json")
            }
            Step::Request { assignment } => {
                let request = self.request(profile, target, &selected(target, &assignment)?)?;
                save(&dir, &request, "writer-request.json")
            }
            Step::Review { request } => {
                let review = self.review_request(
                    profile,
                    &dir,
                    &self.reopen(target)?,
                    &selected(target, &request)?,
                )?;
                Ok(ResultView::Review { review })
            }
            Step::Approve { request, reviewed } => {
                let request: Request = selected(target, &request)?;
                let assignment = self.reopen(target)?;
                if self.review_request(profile, &dir, &assignment, &request)? != reviewed {
                    return Err(Error::Authority);
                }
                // Reserve the maximum encoded Ed25519 signature size before relinquishing.
                let mut bounded = assignment.clone();
                bounded.grants.push(Grant {
                    request: request.clone(),
                    reserved_through: Some(u64::MAX),
                    signature: vec![255; 64],
                });
                if encoded(&bounded)?.len() > MAX_FILE {
                    return Err(Error::History);
                }
                let key =
                    VerifyingKey::from_bytes(&request.next.key).map_err(|_| Error::Signature)?;
                let next = LocalTrustAnchor::new(
                    request.next.device.clone(),
                    cryptex_events::signing_key_id_from_verifying_key(&key)
                        .map_err(|_| Error::Signature)?,
                    request.next.key,
                );
                let grant = self.release(profile, target, &assignment, &next, &request)?;
                save(&dir, &grant, "writer-grant.json")
            }
            Step::Accept { grant } => {
                self.accept(profile, target, &selected(target, &grant)?)?;
                Ok(ResultView::Accepted)
            }
        }
    }
}
