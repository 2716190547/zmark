use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub struct AppAssets;

const ASSETS: &[(&str, &[u8])] = &[
    ("icons/undo.svg", include_bytes!("../assets/icons/undo.svg")),
    ("icons/arrow-up.svg", include_bytes!("../assets/icons/arrow-up.svg")),
    ("icons/arrow-down.svg", include_bytes!("../assets/icons/arrow-down.svg")),
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
    ("icons/file.svg", include_bytes!("../assets/icons/file.svg")),
    ("icons/files.svg", include_bytes!("../assets/icons/files.svg")),
    ("icons/list.svg", include_bytes!("../assets/icons/list.svg")),
    ("icons/palette.svg", include_bytes!("../assets/icons/palette.svg")),
    ("icons/languages.svg", include_bytes!("../assets/icons/languages.svg")),
    ("icons/info.svg", include_bytes!("../assets/icons/info.svg")),
    ("icons/keyboard.svg", include_bytes!("../assets/icons/keyboard.svg")),
    ("icons/pencil.svg", include_bytes!("../assets/icons/pencil.svg")),
    ("icons/refresh.svg", include_bytes!("../assets/icons/refresh.svg")),
    ("icons/minus.svg", include_bytes!("../assets/icons/minus.svg")),
    ("icons/trash.svg", include_bytes!("../assets/icons/trash.svg")),
    ("icons/x.svg", include_bytes!("../assets/icons/x.svg")),
    ("icons/maximize.svg", include_bytes!("../assets/icons/maximize.svg")),
    ("icons/search.svg", include_bytes!("../assets/icons/search.svg")),
    ("icons/image.svg", include_bytes!("../assets/icons/image.svg")),
    ("welcome/hero.svg", include_bytes!("../assets/welcome/hero.svg")),
    ("welcome/workflow.svg", include_bytes!("../assets/welcome/workflow.svg")),
    ("welcome/syntax-playground.svg", include_bytes!("../assets/welcome/syntax-playground.svg")),
    ("welcome/lightbox-badge.svg", include_bytes!("../assets/welcome/lightbox-badge.svg")),
];

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = ASSETS.iter().find(|(p, _)| *p == path) {
            return Ok(Some(Cow::Borrowed(*bytes)));
        }
        let native = gpui_kit_assets::Assets;
        // The kit reports unknown paths as errors; keep our optional lookup contract.
        if native.list(path)?.iter().any(|p| p.as_ref() == path) {
            native.load(path)
        } else {
            Ok(None)
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit_assets::Assets.list(path)?;
        paths.extend(ASSETS.iter().filter(|(p, _)| p.starts_with(path)).map(|(p, _)| SharedString::from(*p)));
        paths.sort();
        paths.dedup();
        Ok(paths)
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
        assert!(list.iter().any(|p| p.as_ref() == "icons/loader-circle.svg"));
        assert_eq!(assets.load("icons/x.svg").unwrap().unwrap().as_ref(), include_bytes!("../assets/icons/x.svg"));
        assert!(assets.load("not-an-asset").unwrap().is_none());
        assert!(assets.list("brand/").unwrap().iter().all(|path| path.starts_with("brand/")));
        let logo = assets.load("brand/logo.png").expect("logo should load");
        assert!(logo.is_some_and(|bytes| !bytes.is_empty()));

        for path in &list {
            let loaded = assets.load(path.as_ref()).expect("load should succeed");
            assert!(loaded.is_some(), "Asset {} failed to load", path);
            assert!(!loaded.unwrap().is_empty(), "Asset {} is empty", path);
        }
    }
}
