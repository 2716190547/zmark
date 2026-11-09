# Welcome to ZMark

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
