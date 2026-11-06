use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub struct AppAssets;

const ASSETS: &[(&str, &[u8])] = &[
    ("brand/logo.png", include_bytes!("../assets/brand/logo.png")),
    ("icons/panel-left.svg", include_bytes!("../assets/icons/panel-left.svg")),
    ("icons/folder.svg", include_bytes!("../assets/icons/folder.svg")),
    ("icons/folder-open.svg", include_bytes!("../assets/icons/folder-open.svg")),
    ("icons/folder-plus.svg", include_bytes!("../assets/icons/folder-plus.svg")),
    ("icons/columns-2.svg", include_bytes!("../assets/icons/columns-2.svg")),
    ("icons/book-open.svg", include_bytes!("../assets/icons/book-open.svg")),
    ("icons/code.svg", include_bytes!("../assets/icons/code.svg")),
    ("icons/download.svg", include_bytes!("../assets/icons/download.svg")),
    ("icons/save.svg", include_bytes!("../assets/icons/save.svg")),
    ("icons/settings.svg", include_bytes!("../assets/icons/settings.svg")),
    ("icons/copy.svg", include_bytes!("../assets/icons/copy.svg")),
    ("icons/check.svg", include_bytes!("../assets/icons/check.svg")),
    ("icons/files.svg", include_bytes!("../assets/icons/files.svg")),
    ("icons/list.svg", include_bytes!("../assets/icons/list.svg")),
    ("icons/chevron-right.svg", include_bytes!("../assets/icons/chevron-right.svg")),
    ("icons/palette.svg", include_bytes!("../assets/icons/palette.svg")),
    ("icons/languages.svg", include_bytes!("../assets/icons/languages.svg")),
    ("icons/info.svg", include_bytes!("../assets/icons/info.svg")),
    ("icons/x.svg", include_bytes!("../assets/icons/x.svg")),
    ("icons/maximize.svg", include_bytes!("../assets/icons/maximize.svg")),
    ("icons/search.svg", include_bytes!("../assets/icons/search.svg")),
    ("icons/image.svg", include_bytes!("../assets/icons/image.svg")),
];

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let bytes = ASSETS
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, b)| Cow::Borrowed(*b));
        Ok(bytes)
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(ASSETS.iter().map(|(p, _)| SharedString::from(*p)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assets_load_and_list() {
        let assets = AppAssets;
        let list = assets.list("").expect("should list assets");
        assert!(list.iter().any(|p| p.as_ref() == "icons/x.svg"));
        let logo = assets.load("brand/logo.png").expect("logo should load");
        assert!(logo.is_some_and(|bytes| !bytes.is_empty()));

        for path in &list {
            let loaded = assets.load(path.as_ref()).expect("load should succeed");
            assert!(loaded.is_some(), "Asset {} failed to load", path);
            assert!(!loaded.unwrap().is_empty(), "Asset {} is empty", path);
        }
    }
}
