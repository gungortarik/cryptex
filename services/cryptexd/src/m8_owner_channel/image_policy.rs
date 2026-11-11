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
pub(super) struct PublisherDefinition {
    pub(super) team: &'static str,
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
}

impl ExpectedImage {
    pub(super) fn requirement(&self) -> &CStr {
        &self.requirement
    }

    pub(super) fn cdhash(&self) -> &[u8; 20] {
        &self.cdhash
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
    if release.publisher.team.len() != 10
        || !release
            .publisher
            .team
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return Err(PolicyError::InvalidPublisher);
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
    // Values are validated before interpolation. Both a Developer ID
    // intermediate and an application leaf are required by this candidate.
    let requirement = CString::new(format!(
        "anchor apple generic and identifier \"{identifier}\" and certificate leaf[subject.OU] = \"{}\" and certificate 1[field.1.2.840.113635.100.6.2.6] exists and certificate leaf[field.1.2.840.113635.100.6.1.13] exists",
        release.publisher.team
    )).map_err(|_| PolicyError::InvalidIdentifier)?;
    Ok(ExpectedImage {
        requirement,
        cdhash,
    })
}

#[cfg(target_os = "macos")]
pub(super) fn inventory_image(
    inventory: &super::release_inventory::VerifiedInventory,
    role: Role,
) -> Result<ExpectedImage, PolicyError> {
    let (desktop_cdhash, daemon_cdhash) = inventory.image_hashes();
    let publisher = inventory.publisher();
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
                team: "TESTTEAM01",
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
            policy.publisher.team = team;
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
