# Unified Topbar, Custom Line-Buffer Editor, and Multi-Palette i18n System

To resolve OS window control collisions, enable high-performance text editing, and support globalized multi-palette styling, we integrate native traffic light insets directly into the top tab bar, implement a dedicated lightweight line-buffer editor engine in `glidemark_app`, and introduce 8 curated palettes with zero-overhead static i18n.

## Considered Options

- **External Heavy Text Editor Dependency (e.g. `gpui-editor` / full Zed buffer)**: Provides extensive IDE capabilities, but introduces complex dependency cascades and potential build incompatibilities with GPUI releases.
- **Traditional Separated Window Titlebar**: Places a standard OS titlebar above the GlideMark toolbar, but wastes valuable vertical screen space and looks dated compared to modern macOS applications.
- **Unified Tab Bar with Platform Insets + Custom Line-Buffer Editor**: Uses macOS-left (80px) and Windows-right (140px) safe insets directly inside a single 40px tab bar. The editor engine directly tracks cursor `(line, col)`, text selections, and keystrokes on `glidemark_core::Document`, delivering sub-millisecond typing response and total lifecycle control.

## Consequences

- Completely eliminates UI overlap with macOS traffic light buttons and Windows window controls.
- Enables native file open dialogs (`Cmd+O`), trackpad scrolling with `ScrollHandle`, and reliable keyboard text editing.
- Embeds 8 curated design palettes (Claude Warm, macOS Vibrant, Tokyo Night, Catppuccin, Nord, Gruvbox, GitHub Modern, Solarized) with Auto/Light/Dark switching and zero-runtime-cost 10-language translations.
