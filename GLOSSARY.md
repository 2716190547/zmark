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
A specialized visual element rendering Mermaid flowcharts and structural diagrams as GPU-native vector graphics via an in-memory memoized, 100% pure-Rust rendering pipeline (`rusty-mermaid`) with Sugiyama layered graph layout (`rusty-mermaid-dagre`), compound subgraphs, database shapes, and dark mode support.
_Avoid_: WebKit iframe, external JS runner, emoji flowchart, kaomoji card, unmemoized render-loop diagram

**Asset Catalog**:
The compiled macOS asset container (`Assets.car`) compiled via `actool` specifying `CFBundleIconName` to provide native dock integration without fallback framing.
_Avoid_: Raw icns, icon directory, tile wrapper

**Default Palette**:
The primary warm terracotta (`#D9532F`) and golden-amber color scheme with warm stone parchment (`#F5F4F0`) and delicate warm-gray dividers (`#E6E4DE`), harmonizing with the GlideMark brand identity.

**Workspace Folder**:
The root local directory selected by the user containing related markdown documents, scanned to populate the navigation drawer.
_Avoid_: Project vault, root repo, workspace path

**Document Tab**:
An interactive horizontal tab element representing an active in-memory document session within the single-window multi-document workspace.
_Avoid_: File pill, document button, window tag
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

**Diagram Texture Budget**:
A clamped dimension bounding policy applied to rendered SVG diagrams so rasterized textures never exceed the display column width, preventing multi-megabyte uncompressed RGBA pixel buffers.
_Avoid_: Arbitrary SVG scaling, unbounded bitmap rasterization

**Bounded Image Cache**:
A Least Recently Used (LRU) eviction pipeline restricting the number of active GPU textures in GPUI memory and actively dropping stale surfaces via `cx.drop_image`.
_Avoid_: Retain-all cache, infinite texture pool, leaky image map

**Diagram Panic Shield**:
The runtime exception boundary wrapping third-party diagram engines in unwind assertions to intercept thread-level panics and downgrade them into non-fatal inline diagnostic cards without process termination.
_Avoid_: Catch block, crash suppressor, silent drop

**Physical Line**:
An unsegmented, horizontally scrollable source line in the editor whose vertical row height is strictly uniform and whose character coordinate matches buffer offsets 1:1.
_Avoid_: Soft-wrap line, visual row, editor line

**Reading Column**:
A centered typography column bounded by the user's maximum width preference, providing optimal character line lengths for ergonomic reading.
_Avoid_: Centered box, reading container, preview margin

**Syntax Highlighter**:
A lightweight pure-Rust lexical tokenizer mapping language keywords, strings, comments, and numbers to semantic palette colors in code blocks without external binary syntax databases.
_Avoid_: Colorizer, syntect wrapper, code parser

**Nested List**:
A hierarchical list structure supporting recursive indentations, preserved inline formatting (bold, italic, links, code), and tiered bullet markers (solid disc `•` to hollow ring `◦`).
_Avoid_: Multi-level bullet, sublist, deep list

**Parse Tier**:
The size-based rule deciding when an edited Buffer is re-parsed: 100ms debounce below 2MB, 300ms from 2–10MB, and no automatic parse above 10MB.
_Avoid_: Parse throttle, render debounce, auto-refresh policy

**Large Document Pause**:
The Preview state above 10MB where automatic re-parsing stops until the user renders manually, saves, or re-enters Preview/Split.
_Avoid_: Read-only mode, frozen preview, disabled parsing

**Virtualized Preview**:
The preview renderer that lays out and paints only Block Elements near the viewport, keeping memory bounded on long documents.
_Avoid_: Lazy list, windowed renderer, chunked HTML

**Cursor Capsule**:
A small floating indicator at the lower-right of the Source/Split editor showing the current line and column after the pane header is removed.
_Avoid_: Status bar, footer, line badge

**Dual-Pane Seamless Canvas**:
A unified application window layout eliminating the global horizontal topbar and horizontal dividers, strictly separating the workspace into a left Navigation Pane (file tree & outline) and a right Content Canvas (header tabs, mode switcher, and editor/preview), connected by a single vertical hairline.
_Avoid_: Topbar layout, divided window, tiled dashboard

**Dynamic Window Inset**:
The adaptive window control positioning system that places macOS traffic lights in the left sidebar header when open, and smoothly migrates traffic light padding (76px) and sidebar expand controls to the right canvas header when the sidebar is collapsed to 0px, with caption button padding reserved on Windows/Linux.
_Avoid_: Fixed titlebar, hidden traffic lights, drifting controls

**Subtle Warm Pill**:
A low-contrast, muted warm-stone rounded rectangular indicator (`#EBE7DE` in light mode, `#2A2826` in dark mode) for active navigation and document items, preserving standard dark/light typography without solid saturated color blocks or high-contrast accent fills.
_Avoid_: Solid accent highlight, saturated badge, colored button fill

**Vector Lightbox**:
A modal overlay with semi-transparent backdrop triggered on diagram click, rendering pure-vector SVG diagrams at 100% DPI with pan, zoom (0.2x–5.0x), reset controls, and Esc key dismissal.
_Avoid_: Inline zoom, external image viewer, popup preview

**Workspace Tree**:
A hierarchical, collapsible filesystem tree in the navigation pane representing the active workspace folder and nested subdirectories with chevron disclosures, replacing flat folder buttons.
_Avoid_: Flat file list, folder picker drawer, file browser

