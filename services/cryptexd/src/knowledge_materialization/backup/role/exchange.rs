//! Public files for moving the scheduled writer role on an existing target.
use super::*;
use crate::ipc::backup_writer::handoff::{ResultView, Review, Step};
use crate::knowledge_materialization::backup::{manual, restore};
use serde::de::DeserializeOwned;

const MAX_FILE: usize = 1024 * 1024;
const REQUEST_DOMAIN: &[u8] = b"cryptex/backup/writer-request/v1\0";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedRequest {
    request: Request,
    signature: Vec<u8>,
}

fn request_bytes(request: &Request) -> Result<Vec<u8>, Error> {
    Ok([REQUEST_DOMAIN, &encoded(request)?].concat())
}

fn verified_request(target: &Path, path: &Path) -> Result<Request, Error> {
    let signed: SignedRequest = selected(target, path)?;
    let key = VerifyingKey::from_bytes(&signed.request.next.key).map_err(|_| Error::Signature)?;
    let signature = Signature::from_slice(&signed.signature).map_err(|_| Error::Signature)?;
    key.verify_strict(&request_bytes(&signed.request)?, &signature)
        .map_err(|_| Error::Signature)?;
    Ok(signed.request)
}

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
                let key = load_existing_event_signing_key(
                    self.store,
                    self.env,
                    profile,
                    self.local.device_id(),
                )
                .map_err(|_| Error::Authority)?;
                if key.signing_key.verifying_key().as_bytes() != &request.next.key {
                    return Err(Error::Authority);
                }
                let signature = key
                    .signing_key
                    .sign(&request_bytes(&request)?)
                    .to_bytes()
                    .to_vec();
                save(
                    &dir,
                    &SignedRequest { request, signature },
                    "writer-request.json",
                )
            }
            Step::Review { request } => {
                let review = self.review_request(
                    profile,
                    &dir,
                    &self.reopen(target)?,
                    &verified_request(target, &request)?,
                )?;
                Ok(ResultView::Review { review })
            }
            Step::Approve { request, reviewed } => {
                let request = verified_request(target, &request)?;
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
