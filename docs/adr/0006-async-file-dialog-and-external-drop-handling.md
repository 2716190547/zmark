# ADR 0006: Asynchronous File Dialogs and External File Drop Handling

## Status
Accepted

## Context
When opening files via `rfd::FileDialog::pick_file()` on macOS, the synchronous invocation blocks the main GPUI thread by running an AppKit modal loop (`runModal`). Any subsequent operating system events dispatched while the modal loop is active (such as keyboard layout change notifications) cause GPUI's `AppCell` to attempt re-entrant mutable borrowing, triggering a fatal `panic_already_borrowed` panic and abort trap. Additionally, users expect modern desktop Markdown apps to allow dragging external files directly from Finder into the application window to open them.

## Decision
1. **Asynchronous File Dialog**: Replace synchronous `rfd::FileDialog::pick_file()` with asynchronous execution spawned via `cx.spawn(...)` (subsequently upgraded to GPUI native `cx.prompt_for_paths` in ADR 0024). Because asynchronous dialogs utilize macOS non-blocking completion handlers, the GPUI event loop is never synchronously blocked or re-entrantly pumped. When the user selects a file, the path is dispatched back into the GPUI context via `view.update(&mut cx, ...)`.
2. **Native External Drop**: Implement GPUI's native `.on_drop<ExternalPaths>` and `.can_drop(...)` on the root workspace view. When a file is dropped from the OS into the window, `ExternalPaths` are inspected, and the primary `.md` or `.markdown` file is immediately loaded into `AppState`.
3. **Resizable Split View**: Introduce `split_ratio: f32` to dynamically distribute horizontal space between the Source Editor and Preview panes, guarded by a draggable divider with a 20%–80% range and double-click 50/50 reset.
4. **Inline Rename**: Support double-clicking the topbar file title to switch into inline renaming mode, persisting changes to disk via `fs::rename` upon confirmation.

## Consequences
- Eliminates application crashes during file dialog interactions on macOS.
- Enables native file drag-and-drop workflows directly from Finder.
- Provides ergonomic, fluid resizing for split-screen editing and reading.
