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



