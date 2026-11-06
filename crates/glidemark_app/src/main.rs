use gpui::*;
use std::env;
use std::path::PathBuf;

use glidemark_app::actions::*;
use glidemark_app::assets::AppAssets;
use glidemark_app::state::AppState;
use glidemark_app::views::workspace::WorkspaceView;
use glidemark_core::document::Document;

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
        let sample_text = r#"# Welcome to GlideMark

GlideMark is a **high-performance**, GPU-accelerated Markdown reader and editor built with Rust and GPUI.

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
    println!("GlideMark: GPU Markdown Editor");
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
| GlideMark | Rust + GPUI | ~140 MB (1.5MB doc) | GPU-paced |
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
            titlebar: Some(TitlebarOptions {
                title: Some("GlideMark".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(14.), px(12.))),
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1200.), px(800.)),
                cx,
            ))),
            ..Default::default()
        };

        cx.open_window(window_options, |window, cx| {
            let state = AppState::new(doc);
            let view = cx.new(|cx| WorkspaceView::new(state, cx));
            window.focus(&view.read(cx).focus_handle);

            let view_close = view.clone();
            window.on_window_should_close(cx, move |window, cx| {
                view_close.update(cx, |this, cx| {
                    this.on_window_should_close(Some(window), cx)
                })
            });

            view
        })
        .unwrap();
    });
}
