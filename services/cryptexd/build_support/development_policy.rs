//! Build-time selection only. Runtime files and environment cannot select a pin.

pub struct Policy {
    pub certificate_variable: &'static str,
    pub release_id: String,
    pub desktop_identifier: &'static str,
    pub daemon_identifier: &'static str,
}

pub fn select(product: bool, release: Option<&str>) -> Result<Policy, &'static str> {
    if !product {
        return Ok(Policy {
            certificate_variable: "CRYPTEX_M8_LOCAL_STARTUP_CERT_DER",
            release_id: "m8-local-startup-v1".into(),
            desktop_identifier: "com.cryptex.m8.local-startup.desktop",
            daemon_identifier: "com.cryptex.m8.local-startup.daemon",
        });
    }
    let release = release.ok_or("development release ID is required")?;
    if !release.starts_with("cryptex-dev-")
        || release.len() <= "cryptex-dev-".len()
        || release.len() > 96
        || !release
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
    {
        return Err("invalid development release ID");
    }
    Ok(Policy {
        certificate_variable: "CRYPTEX_M8_DEVELOPMENT_CERT_DER",
        release_id: release.into(),
        desktop_identifier: "com.cryptex.development.desktop",
        daemon_identifier: "com.cryptex.development.daemon",
    })
}

pub fn sealed_check(revision: &str) -> Result<Policy, &'static str> {
    if revision.len() != 40
        || !revision
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("sealed package requires a full source revision");
    }
    Ok(Policy {
        certificate_variable: "CRYPTEX_M17_SIGNER_DER",
        release_id: format!("cryptex-m17-{revision}"),
        desktop_identifier: "com.cryptex.desktop",
        daemon_identifier: "com.cryptex.daemon",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_identity_cannot_inherit_fixture_policy() {
        let fixture = select(false, Some("cryptex-dev-ignored")).unwrap();
        let product = select(true, Some("cryptex-dev-20261115.a")).unwrap();
        assert_eq!(fixture.release_id, "m8-local-startup-v1");
        assert_ne!(fixture.certificate_variable, product.certificate_variable);
        assert_ne!(fixture.desktop_identifier, product.desktop_identifier);
        assert_ne!(fixture.daemon_identifier, product.daemon_identifier);
        assert_ne!(product.desktop_identifier, product.daemon_identifier);
        assert_eq!(product.release_id, "cryptex-dev-20261115.a");
    }

    #[test]
    fn sealed_check_pins_its_own_signer_roles_and_revision() {
        let revision = "a".repeat(40);
        let policy = sealed_check(&revision).unwrap();
        assert_eq!(policy.release_id, format!("cryptex-m17-{revision}"));
        assert_eq!(policy.certificate_variable, "CRYPTEX_M17_SIGNER_DER");
        assert_eq!(policy.desktop_identifier, "com.cryptex.desktop");
        assert_eq!(policy.daemon_identifier, "com.cryptex.daemon");
        for unsafe_id in ["", "../outside", &"A".repeat(40), &"0".repeat(41)] {
            assert!(sealed_check(unsafe_id).is_err());
        }
    }

    #[test]
    fn missing_fixture_or_unsafe_release_identity_refuses() {
        assert!(select(true, None).is_err());
        for value in [
            "",
            "m8-local-startup-v1",
            "cryptex-dev-",
            "cryptex-dev-a/b",
            "cryptex-dev-a\\b",
            "cryptex-dev-a\nb",
            "cryptex-dev-é",
        ] {
            assert!(select(true, Some(value)).is_err());
        }
        assert!(select(true, Some(&format!("cryptex-dev-{}", "x".repeat(97)))).is_err());
    }
}
