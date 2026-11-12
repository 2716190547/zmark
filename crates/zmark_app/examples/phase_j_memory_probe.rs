use zmark_app::{assets::AppAssets, state::AppState, views::workspace::WorkspaceView};
use zmark_core::{config::Preferences, document::Document};
use gpui::*;
use std::{path::PathBuf, time::Duration};
fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("isolated fixture directory"));
    let document = Document::from_file(root.join("note.md")).expect("fixture document");
    let config = root.join("config.toml");
    let mut preferences = Preferences::load_or_default_from_path(&config);
    preferences.workspace_roots = vec![root.clone()];
    gpui_platform::application()
        .with_assets(AppAssets)
        .run(move |cx: &mut App| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1200.), px(800.)),
                    cx,
                ))),
                ..Default::default()
            };
            cx.open_window(options, |window, cx| {
                let view = cx.new(|cx| {
                    WorkspaceView::new(
                        AppState::new_with_preferences_at(document, preferences, config),
                        cx,
                    )
                });
                let focus_handle = view.read(cx).focus_handle.clone();
                window.focus(&focus_handle, cx);
                view.update(cx, |_, cx| {
                    cx.spawn(async move |this, app| {
                        app.background_executor()
                            .timer(Duration::from_secs(5))
                            .await;
                        eprintln!("PROBE start");
                        for step in 0..80 {
                            let _ = this.update(app, |view, cx| {
                                let count = view.state.doc().parsed.blocks.len();
                                if count > 0 {
                                    view.preview_list.scroll_to(ListOffset {
                                        item_ix: (step * 23) % count,
                                        offset_in_item: px(0.),
                                    });
                                }
                                cx.notify();
                            });
                            app.background_executor()
                                .timer(Duration::from_millis(200))
                                .await;
                        }
                        eprintln!("PROBE settle");
                    })
                    .detach()
                });
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .unwrap();
        });
}
