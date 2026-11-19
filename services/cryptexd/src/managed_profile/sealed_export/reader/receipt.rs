//! Opt-in observation of one successful verification by this daemon. Private
//! diagnostic state stays outside the shared folder and grants no authority.
use super::{Body, ContentHashHex, Directory, Error};
use std::io::Write;

fn valid_attempt(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || matches!(c, b'a'..=b'f'))
}

pub(super) fn write_if_requested(
    staging: &Directory,
    nonce: &ContentHashHex,
    build: &ContentHashHex,
    peer: &Body,
) -> Result<bool, Error> {
    let Ok(attempt) = std::env::var("CRYPTEX_M9_READER_ATTEMPT") else {
        return Ok(false);
    };
    if !valid_attempt(&attempt) {
        return Err(Error::Message);
    }
    let name = format!("{attempt}.receipt");
    if staging.exists(&name)? {
        return Ok(true);
    }
    let record = serde_json::json!({
        "schema":1, "purpose":"cryptex-verified-m9-reader", "attempt":attempt,
        "brain":peer.route.brain, "local":peer.route.receiver, "peer":peer.route.sender,
        "local_nonce":nonce, "peer_nonce":peer.nonce, "local_build":build,
        "peer_build":peer.daemon_sha256, "resolved_blob":peer.resolved_blob
    });
    // Publish a complete small result once. Recovery shares the bounded
    // scratch leaf already owned by this controller, outside the exchange.
    if staging.exists("segment.tmp")? {
        staging.discard_scratch(staging.open_file("segment.tmp", false)?)?;
    }
    let mut file = staging.open_file("segment.tmp", true)?;
    file.write_all(&serde_json::to_vec(&record).map_err(|_| Error::Message)?)?;
    staging.promote("segment.tmp", &file, staging, &name)?;
    Ok(true)
}

#[cfg(test)]
#[test]
fn attempt_is_only_a_lowercase_hex_leaf() {
    assert!(valid_attempt(&"1a".repeat(16)));
    for invalid in ["", "../slot", &"A".repeat(32), &"0".repeat(33)] {
        assert!(!valid_attempt(invalid));
    }
}
