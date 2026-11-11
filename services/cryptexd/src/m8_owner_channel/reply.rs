//! Owned transport-neutral reply. The admitted operation stays alive through
//! the actual bounded send, including error and unwind; this grants no peer authority.

use std::io;
use std::time::Instant;

use serde::Serialize;

use crate::runtime_admission::OperationPermit;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerReply {
    pub(crate) protocol_version: u32,
    pub(crate) request_id: Option<String>,
    pub(crate) accepted: bool,
    pub(crate) error: Option<&'static str>,
    pub(crate) operation: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) approval_request_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) capability: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) snapshot: Option<crate::bootstrap_journal::ApprovalSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) signed_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) exchange_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) brain_identity_json: Option<String>,
}

pub(crate) struct AdmittedReply {
    bytes: Vec<u8>,
    _permit: OperationPermit,
    deadline: Instant,
}

impl std::fmt::Debug for AdmittedReply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AdmittedReply([redacted])")
    }
}

impl AdmittedReply {
    pub(crate) fn new(
        body: OwnerReply,
        permit: OperationPermit,
        deadline: Instant,
    ) -> io::Result<Self> {
        let bytes = serde_json::to_vec(&body)?;
        if bytes.len() > super::request::MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "owner reply frame exceeds limit",
            ));
        }
        Ok(Self {
            bytes,
            _permit: permit,
            deadline,
        })
    }

    /// Consume once. A completed operation may already be durable if delivery
    /// fails/expires: callers must report uncertainty, not cancellation or retry
    /// with an old approval grant. The transport owns newline/native framing.
    pub(crate) fn send_using(
        self,
        send: impl FnOnce(&[u8], Instant) -> io::Result<()>,
    ) -> io::Result<()> {
        if Instant::now() >= self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "owner reply deadline exceeded",
            ));
        }
        send(&self.bytes, self.deadline)?;
        if Instant::now() >= self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "owner reply deadline exceeded",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_admission::{AdmissionError, RuntimeAdmission};
    use std::time::Duration;

    fn body() -> OwnerReply {
        OwnerReply {
            protocol_version: crate::compat::DAEMON_PROTOCOL_VERSION,
            request_id: Some("request".into()),
            accepted: false,
            error: Some("held"),
            operation: "bootstrap.exchange.result",
            approval_request_digest: None,
            capability: None,
            snapshot: None,
            signed_message: None,
            exchange_message: None,
            transaction_id: None,
            brain_identity_json: None,
        }
    }

    #[test]
    fn reply_retains_admission_until_actual_send_finishes() {
        let root = std::env::temp_dir().join("cryptex-reply-admission-test");
        let admission = RuntimeAdmission::detached_for_tests(&root);
        let permit = admission.enter(&root).unwrap();
        let reply =
            AdmittedReply::new(body(), permit, Instant::now() + Duration::from_secs(2)).unwrap();
        assert!(!format!("{reply:?}").contains("request"));
        reply
            .send_using(|bytes, _| {
                assert_eq!(
                    admission.stop_and_drain(Instant::now()),
                    Err(AdmissionError::Busy)
                );
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(bytes).unwrap()["error"],
                    "held"
                );
                Ok(())
            })
            .unwrap();
        admission.stop_and_drain(Instant::now()).unwrap();
    }

    #[test]
    fn expired_failed_and_panicking_reply_release_only_after_delivery_scope() {
        for mode in ["expired", "failed", "panic"] {
            let root = std::env::temp_dir().join("cryptex-reply-failure-test");
            let admission = RuntimeAdmission::detached_for_tests(&root);
            let permit = admission.enter(&root).unwrap();
            let deadline = if mode == "expired" {
                Instant::now()
            } else {
                Instant::now() + Duration::from_secs(2)
            };
            let reply = AdmittedReply::new(body(), permit, deadline).unwrap();
            if mode == "expired" {
                assert_eq!(
                    reply
                        .send_using(|_, _| panic!("expired reply performed a send"))
                        .unwrap_err()
                        .kind(),
                    io::ErrorKind::TimedOut
                );
            } else {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    reply.send_using(|_, _| {
                        assert_eq!(
                            admission.stop_and_drain(Instant::now()),
                            Err(AdmissionError::Busy)
                        );
                        if mode == "panic" {
                            panic!("fixture send unwind")
                        }
                        Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed peer"))
                    })
                }));
                if mode == "panic" {
                    assert!(result.is_err())
                } else {
                    assert_eq!(
                        result.unwrap().unwrap_err().kind(),
                        io::ErrorKind::BrokenPipe
                    )
                }
            }
            admission.stop_and_drain(Instant::now()).unwrap();
        }
    }

    #[test]
    fn oversized_or_abandoned_reply_cannot_retain_runtime_admission() {
        let root = std::env::temp_dir().join("cryptex-reply-size-test");
        for oversized in [false, true] {
            let admission = RuntimeAdmission::detached_for_tests(&root);
            let permit = admission.enter(&root).unwrap();
            let mut response = body();
            if oversized {
                response.signed_message = Some("x".repeat(super::super::request::MAX_FRAME_BYTES));
            }
            let result =
                AdmittedReply::new(response, permit, Instant::now() + Duration::from_secs(1));
            if oversized {
                assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
            } else {
                let reply = result.unwrap();
                assert_eq!(
                    admission.stop_and_drain(Instant::now()),
                    Err(AdmissionError::Busy)
                );
                drop(reply);
            }
            admission.stop_and_drain(Instant::now()).unwrap();
        }
    }
}
