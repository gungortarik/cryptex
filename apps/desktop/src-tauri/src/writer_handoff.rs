use super::{WriterCommand, backup_input_file};
use cryptexd::ipc::backup_writer::{
    self, Action, Error, Outcome,
    handoff::{ResultView, Review, Step},
};
use std::path::PathBuf;

#[cfg(not(feature = "m11-installed-search-check"))]
fn confirm(target: &std::path::Path, review: &Review) -> bool {
    let details = format!(
        "Compare both device IDs and key fingerprints. After signing, this device gives up scheduled backup and retention for this target.\n\nFolder {}\nTarget ID {}\n\nCurrent device {}\nKey SHA-256 {}\n\nNew device {}\nKey SHA-256 {}",
        target.display(),
        review.target_id,
        review.outgoing_device,
        review.outgoing_fingerprint,
        review.incoming_device,
        review.incoming_fingerprint
    );
    rfd::MessageDialog::new()
        .set_title("Transfer scheduled backup and retention?")
        .set_description(details)
        .set_buttons(rfd::MessageButtons::OkCancelCustom(
            "Sign transfer".into(),
            "Keep current writer".into(),
        ))
        .show()
        == rfd::MessageDialogResult::Custom("Sign transfer".into())
}

#[cfg(feature = "m11-installed-search-check")]
fn confirm(_target: &std::path::Path, review: &Review) -> bool {
    std::env::var("CRYPTEX_M11_INSTALLED_EXPORT").as_deref() == Ok("writer-transfer-sign")
        && std::env::var("CRYPTEX_M11_EXPECT_PUBLIC_DIGEST").as_deref()
            == Ok(review.incoming_fingerprint.as_str())
}

pub(super) fn exchange(mode: WriterCommand, target: PathBuf) -> Result<Outcome, Error> {
    let send = |exchange| {
        backup_writer::desktop(Action::Handoff {
            target: target.clone(),
            exchange,
        })
    };
    let step = match mode {
        WriterCommand::ShareWriter => Step::Share,
        WriterCommand::RequestWriter => {
            let Some(assignment) =
                backup_input_file("Select the writer assignment in the target folder")
            else {
                return Ok(Outcome::Cancelled);
            };
            Step::Request { assignment }
        }
        WriterCommand::ReleaseWriter => {
            let Some(request) =
                backup_input_file("Select the new device's writer request in the target folder")
            else {
                return Ok(Outcome::Cancelled);
            };
            let Outcome::Handoff {
                exchange: ResultView::Review { review },
            } = send(Step::Review {
                request: request.clone(),
            })?
            else {
                return Err(Error::InvalidRequest);
            };
            if !confirm(&target, &review) {
                return Ok(Outcome::Cancelled);
            }
            Step::Approve {
                request,
                reviewed: review,
            }
        }
        WriterCommand::AcceptWriter => {
            let Some(grant) =
                backup_input_file("Select the signed writer grant in the target folder")
            else {
                return Ok(Outcome::Cancelled);
            };
            Step::Accept { grant }
        }
        _ => return Err(Error::InvalidRequest),
    };
    send(step)
}
