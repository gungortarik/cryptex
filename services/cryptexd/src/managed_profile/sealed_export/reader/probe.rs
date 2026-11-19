//! A fixed public vector, read entirely in memory. Its test key cannot resolve
//! identities in any profile and never receives input from the exchange tree.
use super::Error;
use cryptex_events::{Classification, ContentHashHex, TrustedDaemonKeyResolver};
use ed25519_dalek::VerifyingKey;
use sha2::{Digest, Sha256};
use std::{fs::File, io, sync::OnceLock};

struct VectorKey;
impl TrustedDaemonKeyResolver for VectorKey {
    fn trusted_daemon_key(&mut self, device: &str, key_id: &str) -> Option<VerifyingKey> {
        if device != "m9-test-device" || key_id != "d7e1e084be213b01e506852af8198b99" {
            return None;
        }
        VerifyingKey::from_bytes(&[
            213, 66, 7, 218, 25, 73, 119, 220, 244, 106, 219, 254, 194, 188, 46, 117, 181, 45, 90,
            138, 66, 24, 79, 237, 253, 192, 0, 36, 240, 227, 232, 218,
        ])
        .ok()
    }
}

pub(super) fn demonstrate(note: &[u8], link: &[u8]) -> Result<ContentHashHex, Error> {
    let history = cryptex_events::verify_v2_history(&[note, link], VectorKey)
        .map_err(|_| Error::LocalReader)?;
    let result =
        cryptex_events::attachment_link::resolve(&history, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")
            .map_err(|_| Error::LocalReader)?;
    if result.note_event_id != "11111111-1111-4111-8111-111111111111"
        || result.classification != Classification::Restricted
        || result.blob_sha256 != <[u8; 32]>::from(Sha256::digest(b"abc"))
    {
        return Err(Error::LocalReader);
    }
    ContentHashHex::from_bytes(&result.blob_sha256).map_err(|_| Error::LocalReader)
}

pub(super) fn local_build() -> Result<(ContentHashHex, ContentHashHex), Error> {
    static BUILD: OnceLock<(ContentHashHex, ContentHashHex)> = OnceLock::new();
    if let Some(build) = BUILD.get() {
        return Ok(build.clone());
    }
    let resolved_blob = demonstrate(
        include_str!("note.json").trim_end().as_bytes(),
        include_str!("link.json").trim_end().as_bytes(),
    )?;
    let mut executable = File::open(std::env::current_exe().map_err(|_| Error::LocalReader)?)
        .map_err(|_| Error::LocalReader)?;
    let mut hash = Sha256::new();
    io::copy(&mut executable, &mut hash).map_err(|_| Error::LocalReader)?;
    let build = ContentHashHex::from_bytes(&hash.finalize()).map_err(|_| Error::LocalReader)?;
    Ok(BUILD.get_or_init(|| (build, resolved_blob)).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damaged_probe_never_reports_reader_support() {
        let note = include_str!("note.json").trim_end().as_bytes();
        let link = include_str!("link.json").trim_end();
        assert!(demonstrate(note, link.as_bytes()).is_ok());
        let altered = link.replace("attachment.link", "attachment.linx");
        assert_eq!(
            demonstrate(note, altered.as_bytes()),
            Err(Error::LocalReader)
        );
    }
}
