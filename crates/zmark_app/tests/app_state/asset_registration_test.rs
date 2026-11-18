use zmark_app::assets::AppAssets;
use gpui::AssetSource;

const PRODUCTION_ICONS: &[&str] = &[
    "icons/search.svg",
    "icons/image.svg",
    "icons/panel-left.svg",
    "icons/folder.svg",
    "icons/folder-open.svg",
    "icons/folder-plus.svg",
    "icons/columns-2.svg",
    "icons/book-open.svg",
    "icons/code.svg",
    "icons/download.svg",
    "icons/save.svg",
    "icons/settings.svg",
    "icons/copy.svg",
    "icons/check.svg",
    "icons/files.svg",
    "icons/list.svg",
    "icons/palette.svg",
    "icons/languages.svg",
    "icons/info.svg",
    "icons/keyboard.svg",
    "icons/pencil.svg",
    "icons/refresh.svg",
    "icons/x.svg",
    "icons/maximize.svg",
];

#[test]
fn test_all_production_icons_load_and_non_empty() {
    let assets = AppAssets;

    for icon in PRODUCTION_ICONS {
        let loaded = assets
            .load(icon)
            .unwrap_or_else(|e| panic!("Error loading asset {icon}: {e}"));
        assert!(
            loaded.is_some(),
            "Production asset {icon} failed to load (returned None)"
        );
        let bytes = loaded.unwrap();
        assert!(
            !bytes.is_empty(),
            "Production asset {icon} loaded but is empty"
        );
    }
}

#[test]
fn test_all_production_icons_listed() {
    let assets = AppAssets;
    let list = assets
        .list("")
        .expect("AppAssets::list should return successfully");
    let listed_set: std::collections::HashSet<&str> =
        list.iter().map(|s| s.as_ref()).collect();

    for icon in PRODUCTION_ICONS {
        assert!(
            listed_set.contains(icon),
            "Production asset {icon} is not present in AppAssets::list"
        );
    }
}
