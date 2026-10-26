# GlideMark

A high-performance, GPU-accelerated single-document Markdown reader and editor designed for modern desktops.

## Language

**Document**:
A single local markdown file loaded into an active viewing and editing session.
_Avoid_: Note, file, vault item, article

**View Mode**:
The active presentation state of the document, toggled between Source (code editor), Preview (rendered document), and Split (side-by-side).
_Avoid_: Layout mode, tab style, preview window

**Buffer**:
The in-memory text data structure holding the raw markdown source text with line and offset indexing.
_Avoid_: Text store, string cache, content state

**Render Tree**:
The GPU-accelerated GPUI element tree generated from the parsed Markdown AST for high-fps viewing.
_Avoid_: DOM, webview layer, HTML output

**Block Element**:
A discrete top-level markdown node (paragraph, heading, table, or fenced code block) rendered as an item within the virtualized preview list.
_Avoid_: Widget, row, chunk, card

**Source Anchor**:
The line-range metadata attached to each block element mapping its rendered height to source buffer line coordinates.
_Avoid_: Line tag, offset mark, bookmark

**Sync Scroll**:
The piecewise linear interpolation mechanism that accurately locks the editor and preview viewports together using source anchors.
_Avoid_: Auto scroll, scroll bind, sticky scroll

**Drawer**:
A collapsible navigation panel toggled on demand to switch among sibling markdown files in the document's working directory or recent history.
_Avoid_: Sidebar, explorer, project tree, workspace pane

**File Watcher**:
A background filesystem notification listener detecting external changes to the opened document on disk.
_Avoid_: File poll, disk sync, watcher daemon

**Asset Pipeline**:
An asynchronous background decoding service resolving relative image paths against the document path into GPU textures.
_Avoid_: Image loader, media cache, asset manager

**Standalone Export**:
A pipeline generating a self-contained single `.html` document bundling rendered content, typography styles, and syntax highlights.
_Avoid_: PDF generator, print renderer, web exporter

**Preferences**:
User-defined layout, typography, and theme settings persisted in an OS-standard TOML configuration file.
_Avoid_: Settings DB, registry, user state

**Palette**:
A curated semantic color scheme (e.g. MiaoYan Pure, Claude Warm, macOS Vibrant, Tokyo Night, Catppuccin, Nord, Gruvbox, GitHub Modern, Solarized) providing light and dark styling variants.
_Avoid_: Theme file, skin, color pack

**Editor Engine**:
The responsive in-memory line buffer tracking cursor position, text selection, keystrokes, and undo/redo history.
_Avoid_: Text box, input widget, textarea

**Window Inset**:
Platform-specific layout padding reserving top-bar space for native window management controls (macOS traffic lights on the left, Windows caption buttons on the right).
_Avoid_: Titlebar margin, top padding, button offset

**Locale**:
The active regional language translation dictionary mapped across 10 supported languages (zh-CN, zh-TW, en, ja, ko, fr, de, es, ru, pt).
_Avoid_: Language pack, string bundle, i18n file

**Split Ratio**:
The dynamic proportional divider coordinate (clamped between 0.20 and 0.80) that allocates horizontal viewport space between the Source Editor and Preview panes.
_Avoid_: Split position, pane width, column size

**Inline Rename**:
In-place document title editing state enabling file path renaming directly on disk without disrupting the active reading or editing session.
_Avoid_: Save-as prompt, title edit dialog, rename modal

**External Drop**:
The operating system file drop mechanism delivering native path collections (`ExternalPaths`) to open dropped markdown documents directly.
_Avoid_: Drag file, file import, drop listener

**App Bundle**:
The self-contained macOS desktop bundle (`.app`) containing native arm64 binary, document association metadata (`Info.plist`), and icon resources (`AppIcon.icns`).
_Avoid_: Executable zip, dmg package, installer archive

**Outline Navigation**:
The hierarchical table of contents extracted from markdown headings (H1–H6), enabling instant anchor navigation and visual section tracking in the drawer panel.
_Avoid_: TOC menu, header list, bookmark list

**Vector Diagram**:
A specialized visual element rendering Mermaid flowcharts and structural diagrams as GPU-native vector graphics (via pure-Rust SVG pipeline) with full semantic layout, directional connectors, and source code inspection.
_Avoid_: Webview iframe, emoji flowchart, kaomoji card

**Asset Catalog**:
The compiled macOS asset container (`Assets.car`) compiled via `actool` specifying `CFBundleIconName` to provide native dock integration without fallback framing.
_Avoid_: Raw icns, icon directory, tile wrapper

**Default Palette**:
The primary warm terracotta (`#D9532F`) and golden-amber color scheme with pure white canvas, harmonizing with the GlideMark brand identity.
_Avoid_: MiaoYan theme, custom skin, orange theme

**Tooltip**:
A contextual floating micro-capsule surfaced on icon hover after a brief delay, detailing the action name and its keyboard shortcut.
_Avoid_: Hint box, hover popup, title tag

**Borderless Hierarchy**:
A flattened, breathable visual design system removing redundant boundary borders and multi-level outlines in favor of clean whitespace and gentle background separation.
_Avoid_: Frameless mode, line-free UI, flat theme

**Micro-animations**:
Short, hardware-accelerated transitions (150–200ms) with easing curves applied to drawer toggles, modal dialogues, and control highlights to deliver smooth tactile feedback.
_Avoid_: Heavy animations, visual effects, transitions pack

**Full-Bleed Icon**:
A 1024x1024 edge-to-edge application icon artwork without baked-in padding or inner squircle cards, relying exclusively on macOS LaunchServices to apply the native system squircle mask.
_Avoid_: Padded icon, card tile, framed icon

**Segmented Control**:
A concentric, borderless or hairline-pill multi-segment switcher (Source / Split / Preview) with uniform recessed track and flat active indicator capsule.
_Avoid_: Tab bar, button row, toggle strip

**Auto Save**:
A silent, debounced background task automatically writing dirty document buffers to disk after 1000ms of user typing inactivity, alongside immediate saves on blur.
_Avoid_: Auto sync, periodic save, timer poll

**Split Settings Modal**:
A two-column preferences dialog organizing configuration domains into a left category navigation sidebar (Appearance, Editor & Save, Language, About) and a right card panel.
_Avoid_: Settings page, preferences window, config popover

**Decoupled Settings Window**:
A dedicated native macOS window opened asynchronously via `cx.defer` with two-column split layout (160px category nav + scrollable settings content) decoupled from WorkspaceView to eliminate borrow panics.
_Avoid_: In-window drawer, settings popup, config modal

**Continuous Surface**:
An edge-to-edge, full-height writing and preview canvas separated by delicate 1px hairlines and tonal background distinction, eliminating floating card gutters for immersive drafting.
_Avoid_: Floating card, dashboard box, rounded tile

**Icon Composer Architecture**:
Apple's modern `.icon` bundle format combining manifest declarations (`icon.json`) with asset layers, compiled via `actool` into multi-layer `IconImageStack` assets in `Assets.car`. This provides native squircle fill and liquid material handling in macOS Tahoe without falling into legacy "Squircle Jail" gray tiles.
_Avoid_: Flat bitmap iconset, unlayered png, squircle jail workaround

**Bidirectional Drawer Animation**:
A smooth 180ms ease-in-out transition animating the directory drawer both when expanding and when collapsing.
_Avoid_: Instant snap, zero-duration close, unmount flash



