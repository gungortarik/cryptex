use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub target_id: String,
    pub outgoing_device: String,
    pub outgoing_fingerprint: String,
    pub incoming_device: String,
    pub incoming_fingerprint: String,
    pub request_sha256: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    Share,
    Request { assignment: PathBuf },
    Review { request: PathBuf },
    Approve { request: PathBuf, reviewed: Review },
    Accept { grant: PathBuf },
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResultView {
    Saved { file: String },
    Review { review: Review },
    Accepted,
}
