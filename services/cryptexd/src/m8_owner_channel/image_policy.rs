//! Candidate release-image policy. Inputs require independently authenticated
//! release metadata, never a peer claim or an unverified adjacent executable.
//! This module supplies image expectations, not stable bundle evidence.

use std::ffi::{CStr, CString};

#[derive(Clone, Copy)]
pub(super) enum Role {
    Desktop,
    Daemon,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PolicyError {
    Unconfigured,
    InvalidPublisher,
    InvalidIdentifier,
    InvalidImageHash,
    AmbiguousRoles,
}

/// Candidate distribution constraint. Choosing this for a release still
/// requires a reviewed publisher decision and signed product acceptance.
#[derive(Clone, Copy)]
pub(super) enum SigningProfile {
    DeveloperIdApplication { team: &'static str },
    LocalCertificate { der: &'static [u8] },
}

#[derive(Clone, Copy)]
pub(super) struct PublisherDefinition {
    pub(super) signing: SigningProfile,
    pub(super) desktop_identifier: &'static str,
    pub(super) daemon_identifier: &'static str,
}

// Final image hashes require a future independently authenticated inventory.
// This representation has no production constructor.
struct ReleaseDefinition {
    publisher: PublisherDefinition,
    desktop_cdhash: [u8; 20],
    daemon_cdhash: [u8; 20],
}

// No authenticated release-inventory factory or publisher has been approved.
// In particular, embedding mutual final CDHashes in both signed executables
// would create a signing cycle. The eventual inventory provenance must be
// reviewed independently; this placeholder has no runtime/file override.
pub(super) fn configured_image(_role: Role) -> Result<ExpectedImage, PolicyError> {
    // A future sealed inventory verifier must supply the expected final hashes.
    // Filling a compiled table of mutual CDHashes cannot enable this factory.
    Err(PolicyError::Unconfigured)
}

pub(super) struct ExpectedImage {
    requirement: CString,
    cdhash: [u8; 20],
    signer_der: Option<&'static [u8]>,
}

impl ExpectedImage {
    pub(super) fn requirement(&self) -> &CStr {
        &self.requirement
    }

    pub(super) fn cdhash(&self) -> &[u8; 20] {
        &self.cdhash
    }
    pub(super) fn signer_der(&self) -> Option<&'static [u8]> {
        self.signer_der
    }
}

fn valid_identifier(id: &str) -> bool {
    id.len() <= 255
        && id.contains('.')
        && id.split('.').all(|part| {
            !part.is_empty() && part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
}

fn expected_image(release: &ReleaseDefinition, role: Role) -> Result<ExpectedImage, PolicyError> {
    match release.publisher.signing {
        SigningProfile::DeveloperIdApplication { team }
            if team.len() != 10
                || !team
                    .bytes()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) =>
        {
            return Err(PolicyError::InvalidPublisher);
        }
        SigningProfile::LocalCertificate { der } if der.is_empty() || der.len() > 65536 => {
            return Err(PolicyError::InvalidPublisher);
        }
        _ => {}
    }
    if !valid_identifier(release.publisher.desktop_identifier)
        || !valid_identifier(release.publisher.daemon_identifier)
    {
        return Err(PolicyError::InvalidIdentifier);
    }
    if release.desktop_cdhash == [0; 20] || release.daemon_cdhash == [0; 20] {
        return Err(PolicyError::InvalidImageHash);
    }
    if release.publisher.desktop_identifier == release.publisher.daemon_identifier
        || release.desktop_cdhash == release.daemon_cdhash
    {
        return Err(PolicyError::AmbiguousRoles);
    }
    let (identifier, cdhash) = match role {
        Role::Desktop => (release.publisher.desktop_identifier, release.desktop_cdhash),
        Role::Daemon => (release.publisher.daemon_identifier, release.daemon_cdhash),
    };
    let (text, signer_der) = match release.publisher.signing {
        SigningProfile::DeveloperIdApplication { team } => (
            format!(
                "anchor apple generic and identifier \"{identifier}\" and certificate leaf[subject.OU] = \"{team}\" and certificate 1[field.1.2.840.113635.100.6.2.6] exists and certificate leaf[field.1.2.840.113635.100.6.1.13] exists"
            ),
            None,
        ),
        // Native validation additionally requires this exact leaf certificate
        // on every message. An identifier alone never admits a local peer.
        SigningProfile::LocalCertificate { der } => {
            (format!("identifier \"{identifier}\""), Some(der))
        }
    };
    let requirement = CString::new(text).map_err(|_| PolicyError::InvalidIdentifier)?;
    Ok(ExpectedImage {
        requirement,
        cdhash,
        signer_der,
    })
}

#[cfg(target_os = "macos")]
pub(super) fn inventory_image(
    bundle: &super::release_inventory::VerifiedBundle,
    role: Role,
) -> Result<ExpectedImage, PolicyError> {
    let (desktop_cdhash, daemon_cdhash) = bundle.image_hashes();
    let publisher = bundle.publisher();
    expected_image(
        &ReleaseDefinition {
            publisher,
            desktop_cdhash,
            daemon_cdhash,
        },
        role,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release() -> ReleaseDefinition {
        ReleaseDefinition {
            publisher: PublisherDefinition {
                signing: SigningProfile::DeveloperIdApplication { team: "TESTTEAM01" },
                desktop_identifier: "com.example.cryptex.desktop",
                daemon_identifier: "com.example.cryptex.daemon",
            },
            desktop_cdhash: [1; 20],
            daemon_cdhash: [2; 20],
        }
    }

    #[test]
    fn product_policy_has_no_configured_publisher() {
        for role in [Role::Desktop, Role::Daemon] {
            assert!(matches!(
                configured_image(role),
                Err(PolicyError::Unconfigured)
            ));
        }
    }

    #[test]
    fn local_profile_preserves_exact_certificate_pin_without_team_membership() {
        const CERT: &[u8] = b"opaque certificate bytes validated by native Security";
        let mut release = release();
        release.publisher.signing = SigningProfile::LocalCertificate { der: CERT };
        for role in [Role::Desktop, Role::Daemon] {
            let image = expected_image(&release, role).unwrap();
            assert_eq!(image.signer_der(), Some(CERT));
            assert!(
                image
                    .requirement()
                    .to_str()
                    .unwrap()
                    .starts_with("identifier ")
            );
        }
        release.publisher.signing = SigningProfile::LocalCertificate { der: &[] };
        assert!(matches!(
            expected_image(&release, Role::Desktop),
            Err(PolicyError::InvalidPublisher)
        ));
    }

    #[test]
    fn roles_select_both_expected_identifier_and_exact_image() {
        let release = release();
        for (role, id, hash) in [
            (
                Role::Desktop,
                release.publisher.desktop_identifier,
                release.desktop_cdhash,
            ),
            (
                Role::Daemon,
                release.publisher.daemon_identifier,
                release.daemon_cdhash,
            ),
        ] {
            let image = expected_image(&release, role).unwrap();
            assert_eq!(image.cdhash(), &hash);
            let text = image.requirement().to_str().unwrap();
            assert!(text.contains(&format!("identifier \"{id}\"")));
            assert!(text.contains("anchor apple generic"));
            assert!(text.contains("certificate 1[field.1.2.840.113635.100.6.2.6] exists"));
            assert!(text.contains("certificate leaf[field.1.2.840.113635.100.6.1.13] exists"));
        }
    }

    #[test]
    fn invalid_or_ambiguous_policy_cannot_select_either_image() {
        for team in [
            "",
            "TESTTEAM0",
            "testteam01",
            "AAAA\" or true",
            "TESTTEAM0\0",
        ] {
            let mut policy = release();
            policy.publisher.signing = SigningProfile::DeveloperIdApplication { team };
            assert!(matches!(
                expected_image(&policy, Role::Desktop),
                Err(PolicyError::InvalidPublisher)
            ));
        }
        for id in [
            "",
            "single",
            ".first",
            "last.",
            "a..b",
            "a.b\0",
            "a.\" or true",
            "a/b.c",
        ] {
            let mut policy = release();
            policy.publisher.desktop_identifier = id;
            assert!(matches!(
                expected_image(&policy, Role::Daemon),
                Err(PolicyError::InvalidIdentifier)
            ));
        }
        let mut policy = release();
        policy.desktop_cdhash = [0; 20];
        assert!(matches!(
            expected_image(&policy, Role::Daemon),
            Err(PolicyError::InvalidImageHash)
        ));
        let mut policy = release();
        policy.daemon_cdhash = policy.desktop_cdhash;
        assert!(matches!(
            expected_image(&policy, Role::Desktop),
            Err(PolicyError::AmbiguousRoles)
        ));
        let mut policy = release();
        policy.publisher.daemon_identifier = policy.publisher.desktop_identifier;
        assert!(matches!(
            expected_image(&policy, Role::Daemon),
            Err(PolicyError::AmbiguousRoles)
        ));
    }
}
