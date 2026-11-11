//! Owned frontend bytes. Construction requires the completed inventory snapshot;
//! this value is not request authority and never reads a URL from disk.
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct ProtectedContent {
    files: Arc<BTreeMap<String, Arc<[u8]>>>,
    binding: [u8; 32],
}
impl ProtectedContent {
    pub(super) fn from_snapshot(
        files: impl Iterator<Item = (String, Vec<u8>)>,
        inventory: &[u8; 32],
    ) -> Result<Self, &'static str> {
        let mut owned = BTreeMap::new();
        for (name, bytes) in files {
            let Some(relative) = name.strip_prefix("Contents/Resources/frontend/") else {
                continue;
            };
            if relative.is_empty()
                || relative
                    .split('/')
                    .any(|s| s.is_empty() || s == "." || s == "..")
                || !relative
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
                || owned
                    .insert(format!("/{relative}"), Arc::<[u8]>::from(bytes))
                    .is_some()
            {
                return Err("protected_frontend_path");
            }
        }
        if !owned.contains_key("/index.html") {
            return Err("protected_frontend_missing");
        }
        let mut hash = Sha256::new();
        hash.update(b"cryptex-protected-frontend-v1");
        hash.update(inventory);
        for (name, bytes) in &owned {
            hash.update((name.len() as u64).to_be_bytes());
            hash.update(name.as_bytes());
            hash.update((bytes.len() as u64).to_be_bytes());
            hash.update(bytes.as_ref());
        }
        Ok(Self {
            files: Arc::new(owned),
            binding: hash.finalize().into(),
        })
    }
    pub fn binding(&self) -> &[u8; 32] {
        &self.binding
    }
    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.files.get(key).map(AsRef::as_ref)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.files
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_ref()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protected_frontend_has_no_disk_or_unlisted_fallback_and_binds_inventory() {
        let source = vec![
            (
                "Contents/Resources/frontend/index.html".into(),
                b"verified".to_vec(),
            ),
            ("Contents/Resources/secret".into(), b"excluded".to_vec()),
        ];
        let content =
            ProtectedContent::from_snapshot(source.clone().into_iter(), &[1; 32]).unwrap();
        assert_eq!(content.get("/index.html"), Some(b"verified".as_slice()));
        for key in ["/secret", "/../index.html", "/missing.js", "index.html"] {
            assert!(content.get(key).is_none());
        }
        assert_eq!(content.iter().count(), 1);
        let retained = content.clone();
        assert!(std::ptr::eq(
            content.get("/index.html").unwrap(),
            retained.get("/index.html").unwrap()
        ));
        assert_ne!(
            content.binding(),
            ProtectedContent::from_snapshot(source.into_iter(), &[2; 32])
                .unwrap()
                .binding()
        );
        assert!(ProtectedContent::from_snapshot(std::iter::empty(), &[1; 32]).is_err());
        drop(content);
        assert_eq!(retained.get("/index.html"), Some(b"verified".as_slice()));
    }
}
