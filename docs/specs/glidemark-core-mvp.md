# Spec: GlideMark Core Architecture and MVP

> **Status note (2026-10):** The memory, 120fps, and synchronized-scroll figures below were original targets, not current measurements. See `docs/adr/0026-large-document-virtualization-parse-tiers-and-deferred-sync-scroll.md` and the performance baseline in `README.md` for implemented behavior.

## Problem Statement

Existing Markdown editors and viewers force users into severe compromises:
1. **Excessive Memory & Resource Consumption**: Most popular editors (Obsidian, Typora, VS Code, MarkText) run on Electron or WebView runtimes, idling at 200MB–500MB+ RAM and consuming noticeable CPU during background tasks.
2. **Degraded Performance on Long Documents**: Parsing and rendering large documents (thousands of lines) causes dropped frames, scrolling stutter, and typing latency.
3. **Cluttered or Unrefined Aesthetics**: Available tools either cater to heavy IDE workflows with dense toolbars, or lack polished typography (poor line-height tuning, lack of golden reading width, awkward CJK/Latin mixed typesetting).
4. **Fragile In-Place WYSIWYG**: Native attempts at Typora-style in-place rich text hybrid editing frequently suffer from cursor jumping, IME input method candidate misalignment, and fragile state synchronization.

Users need a focused, blazing-fast, aesthetic single-document Markdown reader and editor that opens instantly, uses under 50MB of RAM, delivers consistent 120fps scrolling, and honors modern typography.

---

## Solution

**GlideMark** is a GPU-accelerated single-document Markdown reader and editor built with Rust and GPUI (`gpui-component`). 

Key characteristics:
- **Instant Response & Ultra-Low Footprint**: Sub-200ms cold start, <50MB base memory, 120fps GPU-accelerated rendering.
- **Three Fluid View Modes**: Instant zero-latency switching between **Source** (syntax-highlighted code editor), **Preview** (rendered typography document), and **Split** (side-by-side synchronized view).
- **Precision Anchor Sync Scrolling**: AST-level source anchor mapping eliminates scroll drift between editor and preview panes.
- **Aesthetic Typography**: Curated reading widths, balanced CJK/Latin typesetting, elegant code blocks, and subtle macOS-native vibrancy.
- **Modular Architecture**: Multi-crate workspace cleanly isolating pure-Rust AST processing (`glidemark_core`) from GPUI windowing (`glidemark_app`).

---

## User Stories

### Document Viewing & Navigation
1. **As a reader**, I want to open any local `.md` file from Finder, terminal CLI, or file open dialog in under 200ms, so that I can inspect documents with zero waiting.
2. **As a reader**, I want to toggle smoothly between Preview, Source, and Split views via standard shortcuts (`Cmd+1`, `Cmd+2`, `Cmd+3`), so that I can switch focus between pure reading and source editing.
3. **As a reader**, I want the Preview mode to render standard GitHub Flavored Markdown (GFM)—including headings, task lists, tables, blockquotes, and fenced code blocks—with refined typography and balanced line spacing.
4. **As a reader**, I want long documents (10,000+ lines) to scroll at a locked 120fps without stuttering or memory spikes.
5. **As a reader**, I want local relative images (e.g., `![](./images/diagram.png)`) to load and decode asynchronously in the background, so that the scrolling UI never drops frames.
6. **As a reader**, I want to press `Cmd+B` to reveal or collapse a lightweight directory drawer displaying sibling markdown files and recent documents, so that I can switch between related notes without an entire project workspace.

### Editing & Synchronized Scrolling
7. **As an author**, I want syntax-highlighted source code editing powered by Tree-sitter in the Source pane, so that I get responsive typing, accurate colorization, and reliable multi-cursor/selection support.
8. **As an author**, I want the Split mode to scroll both panes in lockstep using AST source anchors rather than percentage math, so that the code block I am editing on the left stays vertically aligned with the rendered code block on the right.
9. **As an author**, I want typing in Source mode to update the Preview with a brief debounce (50ms–100ms), so that rendering does not cause input latency or CPU thrashing.

### File Lifecycle & Export
10. **As an author**, I want external file modifications (e.g., from Git checkout or external scripts) to be detected automatically via filesystem notifications, so that clean files reload silently and conflicting edits present a non-intrusive conflict prompt.
11. **As an author**, I want automatic saving on window blur and explicit saving via `Cmd+S`, so that I never lose my work.
12. **As an author**, I want to export the document into a self-contained, standalone `.html` file with embedded styles and highlighted code, so that I can share it or print it as a vector PDF from any browser.
13. **As an author**, I want my typography preferences (font family, font size, maximum reading column width, light/dark theme) to persist in a simple `config.toml` file.

---

## Implementation Decisions

### 1. Multi-Crate Workspace Layout

The repository is partitioned into two crates to isolate domain logic from GPUI upstream API churn (per ADR-0004):

```
GlideMark/
├── Cargo.toml                  # Workspace manifest
├── docs/                       # Specifications and ADRs
│   ├── adr/
│   └── specs/
├── crates/
│   ├── glidemark_core/         # Pure Rust: AST, anchors, file I/O, export, config
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── ast.rs          # pulldown-cmark parsing & BlockElement types
│   │       ├── anchor.rs       # Source line mapping & sync scroll interpolation
│   │       ├── config.rs       # Preferences model & TOML serialization
│   │       ├── document.rs     # Document session, buffer tracking & dirty state
│   │       ├── export.rs       # Standalone HTML generator
│   │       ├── watcher.rs      # Notify-based file system event stream
│   │       └── lib.rs
│   └── glidemark_app/          # GPUI application & desktop presentation
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs         # Entry point, app initialization & window setup
│           ├── state.rs        # App state & document session model
│           ├── theme.rs        # Color palettes, typography & gpui-component styling
│           ├── views/
│           │   ├── workspace.rs    # Root view orchestrating layout & mode switches
│           │   ├── editor.rs       # gpui-component editor integration
│           │   ├── preview.rs      # Virtualized block list renderer
│           │   └── drawer.rs       # Collapsible sibling file drawer
│           ├── assets.rs       # Async image loader & GPU texture cache
│           └── lib.rs
```

### 2. Core Interfaces and Data Contracts (`glidemark_core`)

#### AST & Source Anchor Representation
```rust
/// Top-level Markdown block element generated during tokenization.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockElement {
    Heading { level: u8, content: InlineGroup },
    Paragraph(InlineGroup),
    FencedCode { lang: Option<String>, code: String },
    BlockQuote(Vec<BlockElement>),
    List { ordered: bool, items: Vec<ListItem> },
    Table { headers: Vec<InlineGroup>, rows: Vec<Vec<InlineGroup>> },
    Rule,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceAnchor {
    pub block_index: usize,
    pub source_line_start: usize,
    pub source_line_end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDocument {
    pub blocks: Vec<BlockElement>,
    pub anchors: Vec<SourceAnchor>,
}
```

#### Sync-Scroll Interpolation Algorithm
```rust
pub struct SyncScrollMap {
    anchors: Vec<SourceAnchor>,
}

impl SyncScrollMap {
    pub fn new(anchors: Vec<SourceAnchor>) -> Self;
    
    /// Translates source line number to target block index and normalized inner progress [0.0, 1.0].
    pub fn source_line_to_preview_progress(&self, line: usize) -> (usize, f32);
    
    /// Translates preview block index and inner progress back to estimated source line.
    pub fn preview_to_source_line(&self, block_index: usize, progress: f32) -> usize;
}
```

#### Document Lifecycle & Dirty Tracking
```rust
pub enum DocumentStatus {
    Clean,
    Modified,
    ConflictOnDisk,
}

pub struct Document {
    pub path: Option<PathBuf>,
    pub buffer: String,
    pub status: DocumentStatus,
    pub parsed: ParsedDocument,
}
```

### 3. Rendering & Presentation (`glidemark_app`)

- **Virtualized Preview List**: The preview pane treats each top-level `BlockElement` as a distinct item rendered into GPUI's virtualized list (`UniformList` or measured block delegate). Only blocks inside the visible viewport + overscan margin are measured and painted.
- **Tree-sitter Highlighting**: Fenced code blocks in the preview and the entire buffer in the editor pane use Tree-sitter grammars for CommonMark and nested code languages (Rust, TypeScript, Python, JSON, etc.).
- **Async Image Loading**: `AssetPipeline` resolves relative file paths against `document.path.parent()`. Decoding runs on background worker threads using the `image` crate, uploading completed textures to GPUI's image cache on the main thread.

### 4. Failure Modes and Error Handling Strategies

| Failure Scenario | Impact | Mitigation Strategy |
| :--- | :--- | :--- |
| **Malformed Markdown Syntax** | Visual artifact | CommonMark parser is tolerant and never fails; invalid constructs degrade gracefully to plain text paragraphs. |
| **Missing / Unreadable Image Asset** | Broken layout | Show an inline fallback card with a subtle warning border, image path, and tooltip instead of crashing or leaving a blank gap. |
| **External Modification Conflict** | Potential data loss | If the document buffer has unsaved changes and disk content changes, do not overwrite; display a floating non-modal banner: `[Keep My Edits]` or `[Reload From Disk]`. |
| **Massive File Load (>50MB)** | Memory / lag | If a file exceeds 10MB, display a warning prompt before parsing, disable live debounce in split mode, and parse only on explicit save/idle. |
| **Font Not Found** | Layout failure | Fallback font chain: `Preferences.font_family` -> `PingFang SC / SF Pro Text` -> `system-ui` -> `sans-serif`. |

---

## Testing Decisions

### Test Seams

```
[Markdown String / File] 
        │
        ▼ ─── Seam 1: Core AST & Anchor Tokenizer (Headless unit tests)
[ParsedDocument & SourceAnchors]
        │
        ▼ ─── Seam 2: SyncScrollMap Piecewise Interpolation (Unit tests)
[Block Index & Progress]
        │
        ▼ ─── Seam 3: Document State Machine & File Watcher (Integration tests)
[Document Lifecycle: Clean / Dirty / Conflict]
        │
        ▼ ─── Seam 4: HTML Exporter Snapshot Verification (Headless snapshot tests)
[Self-Contained HTML Output]
        │
        ▼ ─── Seam 5: GPUI Viewport & Layout (TestAppContext headless GUI tests)
[GPUI Window & View Rendering]
```

### Verification Boundaries

1. **Seam 1 (`glidemark_core::ast`)**:
   - Verify GFM syntax variants (tables, multiline code fences, task lists) produce expected `BlockElement` trees.
   - Verify that every `BlockElement` receives exact 1-indexed line start and end ranges that correspond to the input source buffer.
2. **Seam 2 (`glidemark_core::anchor`)**:
   - Feed irregular line distributions (e.g. 100-line code block followed by 1-line paragraph) and verify that `source_line_to_preview_progress` produces continuous, non-drifting coordinates without out-of-bounds panics.
3. **Seam 3 (`glidemark_core::document`)**:
   - Test transitions:
     - `Clean` -> modify buffer -> `Modified` -> save -> `Clean`.
     - `Clean` -> external disk modify -> auto-reload -> `Clean`.
     - `Modified` -> external disk modify -> `ConflictOnDisk`.
4. **Seam 4 (`glidemark_core::export`)**:
   - Snapshot test: verify that exporting a standard sample Markdown file generates well-formed HTML containing embedded CSS reset, typography variables, and inline-highlighted code.
5. **Seam 5 (`glidemark_app::views`)**:
   - Use GPUI's headless `TestAppContext` to assert that shortcut actions (`ToggleMode`, `ToggleDrawer`, `SaveDocument`) fire correctly and alter the active `ViewMode`.

---

## Out of Scope

The following items are deliberately excluded from this MVP:
- **In-place WYSIWYG / Live Preview**: No Typora/Notion-style inline rich-text editing where Markdown syntax markers disappear on focus loss.
- **Full Knowledge Base Features**: No multi-folder vaults, bidirectional backlink graphs, local SQLite indexing, or tag databases.
- **Embedded Headless PDF Engines**: No Chromium, WebKit, or Typst PDF rendering bundled into the application binary.
- **Complex Diagrams & Math**: Native KaTeX / LaTeX math layout and Mermaid diagram rendering (deferred to post-MVP modular plugins).
- **Cloud Sync & Collaboration**: No CRDTs, WebSocket servers, or remote cloud storage integrations.

---

## Further Notes & Risks

- **GPUI Dependency Pinning**: Because GPUI does not follow strict SemVer on crates.io, the workspace will pin to a verified stable git rev from `zed-industries/zed` or `longbridge/gpui-kit`.
- **macOS CJK IME Handling**: In split mode, the editor pane must preserve native macOS IME marked text styling. Leveraging `gpui-component`'s editor ensures upstream IME bug fixes are inherited directly.
- **Variable Height Virtualization**: Unlike simple table rows, Markdown blocks have varying heights (a short title vs a 50-line code block). The preview list delegate will store measured heights per block to guarantee jitter-free scrolling during scrollbar drag.
