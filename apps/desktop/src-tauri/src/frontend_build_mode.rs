//! Prevent a release probe from silently depending on a development server.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FrontendBuildModeError {
    ReleaseUsesDevelopmentServer,
}

fn validate(release: bool, tauri_development: bool) -> Result<(), FrontendBuildModeError> {
    if release && tauri_development {
        Err(FrontendBuildModeError::ReleaseUsesDevelopmentServer)
    } else {
        Ok(())
    }
}

pub(crate) fn ensure_current() -> Result<(), FrontendBuildModeError> {
    validate(!cfg!(debug_assertions), tauri::is_dev())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_requires_embedded_frontend_but_debug_allows_development() {
        assert_eq!(
            validate(true, true),
            Err(FrontendBuildModeError::ReleaseUsesDevelopmentServer)
        );
        assert_eq!(validate(true, false), Ok(()));
        assert_eq!(validate(false, true), Ok(()));
        assert_eq!(validate(false, false), Ok(()));
    }

    #[test]
    fn compiled_mode_matches_tauri_feature_and_rust_profile() {
        let expected = if !cfg!(debug_assertions) && tauri::is_dev() {
            Err(FrontendBuildModeError::ReleaseUsesDevelopmentServer)
        } else {
            Ok(())
        };
        assert_eq!(ensure_current(), expected);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn supported_release_suite_requires_embedded_assets() {
        assert!(
            !tauri::is_dev(),
            "release tests require tauri/custom-protocol"
        );
        assert_eq!(ensure_current(), Ok(()));
    }
}
