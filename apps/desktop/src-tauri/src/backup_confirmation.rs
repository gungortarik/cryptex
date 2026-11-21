use rfd::MessageDialogResult;

// The Windows MessageBox backend uses the standard OK result even when
// OkCancelCustom supplies labels. Other backends return the chosen label.
fn approved(result: MessageDialogResult, action: &str) -> bool {
    match result {
        MessageDialogResult::Ok => true,
        MessageDialogResult::Custom(label) => label == action,
        _ => false,
    }
}

#[cfg(not(feature = "m11-installed-search-check"))]
pub(super) fn confirm(title: &str, description: String, action: &str, cancel: &str) -> bool {
    let result = rfd::MessageDialog::new()
        .set_title(title)
        .set_description(description)
        .set_buttons(rfd::MessageButtons::OkCancelCustom(
            action.into(),
            cancel.into(),
        ))
        .show();
    approved(result, action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_affirmative_ok_cancel_choice_authorizes_a_backup_action() {
        let action = "Sign transfer";
        assert!(approved(MessageDialogResult::Ok, action));
        assert!(approved(MessageDialogResult::Custom(action.into()), action));
        for result in [
            MessageDialogResult::Cancel,
            MessageDialogResult::Custom("Keep current writer".into()),
            MessageDialogResult::Custom(String::new()),
            MessageDialogResult::Yes,
            MessageDialogResult::No,
        ] {
            assert!(!approved(result, action));
        }
    }
}
