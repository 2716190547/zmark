use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let bytes: Option<&'static [u8]> = match path {
            "icons/panel-left.svg" => Some(include_bytes!("../assets/icons/panel-left.svg")),
            "icons/folder.svg" => Some(include_bytes!("../assets/icons/folder.svg")),
            "icons/columns-2.svg" => Some(include_bytes!("../assets/icons/columns-2.svg")),
            "icons/book-open.svg" => Some(include_bytes!("../assets/icons/book-open.svg")),
            "icons/code.svg" => Some(include_bytes!("../assets/icons/code.svg")),
            "icons/download.svg" => Some(include_bytes!("../assets/icons/download.svg")),
            "icons/save.svg" => Some(include_bytes!("../assets/icons/save.svg")),
            "icons/settings.svg" => Some(include_bytes!("../assets/icons/settings.svg")),
            "icons/copy.svg" => Some(include_bytes!("../assets/icons/copy.svg")),
            "icons/check.svg" => Some(include_bytes!("../assets/icons/check.svg")),
            "icons/files.svg" => Some(include_bytes!("../assets/icons/files.svg")),
            "icons/list.svg" => Some(include_bytes!("../assets/icons/list.svg")),
            "icons/chevron-right.svg" => Some(include_bytes!("../assets/icons/chevron-right.svg")),
            "icons/palette.svg" => Some(include_bytes!("../assets/icons/palette.svg")),
            "icons/languages.svg" => Some(include_bytes!("../assets/icons/languages.svg")),
            "icons/info.svg" => Some(include_bytes!("../assets/icons/info.svg")),
            "icons/x.svg" => Some(include_bytes!("../assets/icons/x.svg")),
            _ => None,
        };
        Ok(bytes.map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![
            "icons/panel-left.svg".into(),
            "icons/folder.svg".into(),
            "icons/columns-2.svg".into(),
            "icons/book-open.svg".into(),
            "icons/code.svg".into(),
            "icons/download.svg".into(),
            "icons/save.svg".into(),
            "icons/settings.svg".into(),
            "icons/copy.svg".into(),
            "icons/check.svg".into(),
            "icons/files.svg".into(),
            "icons/list.svg".into(),
            "icons/chevron-right.svg".into(),
            "icons/palette.svg".into(),
            "icons/languages.svg".into(),
            "icons/info.svg".into(),
            "icons/x.svg".into(),
        ])
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

        for path in &list {
            let loaded = assets.load(path.as_ref()).expect("load should succeed");
            assert!(loaded.is_some(), "Asset {} failed to load", path);
            assert!(!loaded.unwrap().is_empty(), "Asset {} is empty", path);
        }
    }
}
