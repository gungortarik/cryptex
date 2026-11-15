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
