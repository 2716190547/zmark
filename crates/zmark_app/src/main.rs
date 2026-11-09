#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gpui::*;
use std::env;
use std::path::PathBuf;

use zmark_app::actions::*;
use zmark_app::assets::AppAssets;
use zmark_app::state::AppState;
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::config::Preferences;

fn main() {
    let args: Vec<String> = env::args().collect();
    let initial_path = if args.len() > 1 {
        Some(PathBuf::from(&args[1]))
    } else {
        None
    };

    let preferences = Preferences::load_or_default();
    let doc = zmark_app::welcome::initial_document(initial_path, preferences.locale);

    let app = Application::new().with_assets(AppAssets);
    app.run(move |cx: &mut App| {
        // GPUI creates its own NSApplication subclass during run(). Touching the
        // shared application before this point makes GPUI write its `platform`
        // ivar to the plain NSApplication and panic at startup.
        zmark_app::platform::register_application_icon();
        // Register keybindings
        let shortcut = zmark_app::platform::shortcut;
        cx.bind_keys([
            KeyBinding::new(&shortcut("1"), ToggleSourceMode, None),
            KeyBinding::new(&shortcut("2"), ToggleSplitMode, None),
            KeyBinding::new(&shortcut("3"), TogglePreviewMode, None),
            KeyBinding::new(&shortcut("b"), ToggleDrawer, None),
            KeyBinding::new(&shortcut("o"), OpenFileDialog, None),
            KeyBinding::new(&shortcut(","), ToggleSettings, None),
            KeyBinding::new(&shortcut("s"), SaveFile, None),
            KeyBinding::new(&shortcut("shift-e"), ExportHtml, None),
        ]);

        let window_options = WindowOptions {
            titlebar: zmark_app::platform::titlebar_options("ZMark", 12.0),
            window_decorations: zmark_app::platform::window_decorations(),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1200.), px(800.)),
                cx,
            ))),
            ..Default::default()
        };

        cx.open_window(window_options, |window, cx| {
            let state = AppState::new_with_preferences(doc, preferences);
            let update_config = zmark_app::updater::UpdateConfig::production();
            let view = cx.new(|cx| {
                WorkspaceView::new_with_update_config(state, update_config, cx)
            });
            window.focus(&view.read(cx).focus_handle);

            let view_close = view.clone();
            window.on_window_should_close(cx, move |window, cx| {
                view_close.update(cx, |this, cx| this.on_window_should_close(Some(window), cx))
            });

            view.update(cx, |this, cx| {
                this.install_window_activation_observer(window, cx);
            });

            zmark_app::frame_probe::start(window, view.entity_id());
            if std::env::var("ZMARK_FRAME_PROBE").is_ok() {
                // Drive the mode-switch animations while the probe records.
                let probe_view = view.clone();
                cx.spawn(async move |app| {
                    let modes = [
                        zmark_app::state::ViewMode::Split,
                        zmark_app::state::ViewMode::Preview,
                        zmark_app::state::ViewMode::Source,
                        zmark_app::state::ViewMode::Split,
                        zmark_app::state::ViewMode::Preview,
                        zmark_app::state::ViewMode::Source,
                    ];
                    for mode in modes {
                        app.background_executor()
                            .timer(std::time::Duration::from_millis(300))
                            .await;
                        let _ = probe_view.update(app, |workspace, cx| {
                            workspace.set_view_mode(mode, cx);
                        });
                    }
                })
                .detach();
            }

            view
        })
        .unwrap();
    });
}
