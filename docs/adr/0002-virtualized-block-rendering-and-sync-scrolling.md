# Virtualized Block Rendering and Anchor-Based Sync Scrolling

> **Status note (2026-10):** Preview virtualization is implemented with GPUI's `list`. Split view sync scrolling is not wired in the app yet; see ADR 0026.

To guarantee 120fps scrolling on documents of any size and prevent scroll drift in Split mode, we render the preview as a virtualized list of top-level Block Elements and bind the editor and preview viewports using AST Source Anchors.

## Considered Options

- **Full GPUI Element Tree Reconstruction**: Renders the whole document in a single scroll view. Trivial to build, but drops frames and increases memory footprint on documents exceeding a few thousand lines.
- **Percentage-Based Scroll Syncing**: Proportionally offsets the preview based on editor scroll position. Simple, but drifts significantly whenever blocks have uneven height ratios (e.g. dense lists vs large code blocks or images).
- **Virtualized Block Rendering + Source Anchor Syncing**: Parses markdown into top-level Block Elements with source line ranges; only measures and paints blocks inside the active viewport, and linearly interpolates scroll offset between adjacent anchors.

## Consequences

- Requires `pulldown-cmark` (or equivalent parser) to emit byte offset / line mappings during block tokenization.
- Preview viewport can handle massive documents without layout thrashing or stutter.
