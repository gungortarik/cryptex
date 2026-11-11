//! Tauri reads only the retained verified frontend. Its own HTML/index routing
//! may fall back within this map; there is no disk, embedded or dev-server fallback.
use cryptexd::ProtectedContent;
use std::borrow::Cow;
use tauri::utils::assets::{AssetKey, AssetsIter, CspHash};

pub struct SnapshotAssets(ProtectedContent);
impl<R: tauri::Runtime> tauri::Assets<R> for SnapshotAssets {
    fn get(&self, key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        self.0.get(key.as_ref()).map(Cow::Borrowed)
    }
    fn iter(&self) -> Box<AssetsIter<'_>> {
        Box::new(
            self.0
                .iter()
                .map(|(key, value)| (Cow::Borrowed(key), Cow::Borrowed(value))),
        )
    }
    fn csp_hashes(&self, _: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        Box::new(std::iter::empty())
    }
}

pub fn install<R: tauri::Runtime>(
    context: &mut tauri::Context<R>,
    content: ProtectedContent,
    runtime: &std::path::Path,
) {
    // Both debug and embedded-protocol builds must use the same compiled CSP.
    let config = context.config_mut();
    config.identifier = "com.cryptex.m8.native-ui-fixture".into();
    config.app.app_directories_override = Some(tauri::utils::config::AppDirectoriesOverride::Root(
        runtime.join("ui-data"),
    ));
    config.build.dev_url = None;
    config.build.frontend_dist = Some(tauri::utils::config::FrontendDist::Directory(
        "__verified_memory_only__".into(),
    ));
    config.app.security.dev_csp = config.app.security.csp.clone();
    for window in &mut config.app.windows {
        window.url = tauri::WebviewUrl::App("index.html".into());
        window.incognito = true;
    }
    drop(context.set_assets(Box::new(SnapshotAssets(content))));
}
