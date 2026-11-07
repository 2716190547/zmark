#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gpui::*;
use std::env;
use std::path::PathBuf;

use zmark_app::actions::*;
use zmark_app::assets::AppAssets;
use zmark_app::state::AppState;
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;

fn main() {
    let args: Vec<String> = env::args().collect();
    let initial_path = if args.len() > 1 {
        Some(PathBuf::from(&args[1]))
    } else {
        None
    };

    let doc = if let Some(path) = initial_path {
        Document::from_file(path).unwrap_or_else(|_| Document::new_scratch())
    } else {
        let sample_text = r#"# Welcome to ZMark

ZMark is a **high-performance**, GPU-accelerated Markdown reader and editor built with Rust and GPUI.

## Key Features

- **Blazing Fast**: Virtualized editor and preview keep a 1.5MB document under 150MB of RAM.
- **Three View Modes**: 
  - `Cmd+1` Source Editor
  - `Cmd+2` Split View
  - `Cmd+3` Typography Reading View
- **GFM Compliant**: Tables, task lists, code blocks, blockquotes.

### Sample Code Block

```rust
fn main() {
    println!("ZMark: GPU Markdown Editor");
}
```

### Task Checklist

- [x] High precision AST parsing
- [x] Virtualized preview and editor
- [x] Standalone single-file HTML export
- [ ] Modular KaTeX / LaTeX plugins

### Comparison Table

| Editor | Engine | Memory (Idle) | Scroll FPS |
|---|---|---|---|
| ZMark | Rust + GPUI | ~140 MB (1.5MB doc) | GPU-paced |
| Typora | Electron | ~220 MB | 60 fps |
| Obsidian | Electron | ~350 MB | 60 fps |

> "The best code is the code that never drops a frame."

Enjoy writing and reading with pure speed!
"#;
        let mut d = Document::new_scratch();
        d.update_buffer(sample_text.to_string());
        d
    };

    let app = Application::new().with_assets(AppAssets);
    app.run(move |cx: &mut App| {
        // Register keybindings
        cx.bind_keys([
            KeyBinding::new("cmd-1", ToggleSourceMode, None),
            KeyBinding::new("cmd-2", ToggleSplitMode, None),
            KeyBinding::new("cmd-3", TogglePreviewMode, None),
            KeyBinding::new("cmd-b", ToggleDrawer, None),
            KeyBinding::new("cmd-o", OpenFileDialog, None),
            KeyBinding::new("cmd-,", ToggleSettings, None),
            KeyBinding::new("cmd-s", SaveFile, None),
            KeyBinding::new("cmd-shift-e", ExportHtml, None),
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
            let state = AppState::new(doc);
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
