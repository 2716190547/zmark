use zmark_app::views::settings::shortcut_rows;
use zmark_core::i18n::{get_strings, SupportedLocale};

#[gpui::test]
fn settings_click_persists_only_the_workspace_owned_preferences(cx: &mut gpui::TestAppContext) {
    use gpui::{AppContext, Modifiers};
    use zmark_app::{state::{AppState, SettingsTab}, views::{settings::SettingsWindowView, workspace::WorkspaceView}};
    use zmark_core::{config::{Preferences, ThemeMode}, document::Document};
    let mut fixture = None;
    let (_root, cx) = cx.add_window_view(|window, cx| {
        let workspace = cx.new(|cx| WorkspaceView::new(AppState::new_isolated(Document::new_scratch()), cx));
        let state = &workspace.read(cx).state;
        let preferences = state.preferences.clone();
        let theme = state.theme.clone();
        let strings = state.strings;
        let settings = cx.new(|cx| SettingsWindowView::new(workspace.downgrade(), SettingsTab::Appearance, preferences, theme, strings, cx));
        let root = gpui_component::Root::new(settings.clone(), window, cx);
        fixture = Some((workspace, settings, strings.theme_dark));
        root
    });
    let (workspace, settings, label) = fixture.unwrap();
    let path = workspace.read_with(cx, |ws, _| ws.state.preferences_path.clone());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let button = cx.debug_bounds(label).expect("real theme button").center();
    cx.simulate_click(button, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(workspace.read_with(cx, |ws, _| ws.state.preferences.theme), ThemeMode::Dark);
    assert_eq!(settings.read_with(cx, |settings, _| settings.preferences.theme), ThemeMode::Dark);
    assert_eq!(Preferences::load_or_default_from_path(&path).theme, ThemeMode::Dark);
}

#[test]
fn shortcut_guide_has_the_six_canonical_entries() {
    let strings = get_strings(SupportedLocale::En);
    let rows = shortcut_rows(strings);
    assert_eq!(rows.len(), 6);
    let expected = |key: &str| zmark_app::platform::shortcut_hint(key);
    assert_eq!(rows[0].0, expected("O"));
    assert_eq!(rows[2].0, format!("⇧{}", expected("E")));
    assert_eq!(rows[5].0, expected(","));
    if cfg!(target_os = "macos") {
        assert_eq!(rows[0].0, "⌘O");
    } else {
        assert_eq!(rows[0].0, "Ctrl+O");
    }
}
