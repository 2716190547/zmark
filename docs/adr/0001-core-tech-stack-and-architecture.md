# Tech Stack and Interaction Paradigm: Rust + GPUI with Split/Toggle View

We need sub-second startup, 120fps smooth scrolling, and minimal memory (<50MB) for markdown editing and reading. We chose Rust with GPUI and `gpui-component`, using a Source / Preview / Split toggle interaction paradigm rather than in-place WYSIWYG or a Webview shell.

## Considered Options

- **Tauri 2.0 + Webview (Svelte 5 / ProseMirror)**: Lower implementation cost for rich-text inline editing, but dependent on system WebKit/WebView2 (~60-90MB baseline memory) and non-GPU-native render latency.
- **In-place WYSIWYG in GPUI (Typora-style)**: Highest implementation complexity; native inline mixed layout with cursor projection and CJK IME in Rust is currently immature and error-prone.
- **Split / Toggle in Rust + GPUI**: Leverages Zed's proven GPU text buffer and Tree-sitter for editing, while rendering GFM directly to GPUI element trees, achieving sub-50MB memory and 120fps scrolling.

## Consequences

- Focuses initial effort on macOS desktop ergonomics and typography excellence.
- Isolates Markdown AST parsing and rendering from GPUI windowing APIs to insulate against upstream framework breaking changes.
- Deferred LaTeX/Mermaid support to modular post-launch extensions.
