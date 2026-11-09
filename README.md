# ZMark

<img src="assets/icon.iconset/icon_256x256.png" alt="ZMark icon" width="128">

[简体中文](README.zh-CN.md)

ZMark is a Markdown editor and reader for local files, built with Rust and GPUI. Edit Markdown, read a formatted preview, or use both side by side.

## Features

- Source, split, and preview modes with synchronized scrolling.
- A drawer for multiple workspace folders, loose files, drafts, search, and document outline.
- Markdown tables, task lists, code blocks with syntax highlighting, and Mermaid diagrams.
- Light and dark appearance, multiple palettes, and localized settings.
- Export a self-contained HTML document for offline reading.

## Getting started

Download available builds from the [releases repository](https://github.com/2716190547/zmark-releases/releases), or build from source below. The repository includes desktop build workflows for macOS, Linux, and Windows; availability and platform behavior depend on the published build.

On macOS, open a Markdown file with `Cmd+O`, switch views with `Cmd+1`, `Cmd+2`, or `Cmd+3`, and open settings with `Cmd+,`. Save with `Cmd+S` and export HTML with `Cmd+Shift+E`. The settings window lists common shortcuts.

## Build from source

Install a current stable Rust toolchain and platform dependencies, then run:

```sh
cargo run -p zmark_app
cargo test --workspace
```

The [desktop workflow](.github/workflows/build-desktop.yml) lists the Linux build dependencies and packaging steps. On macOS, `./scripts/bundle_macos.sh` builds and installs an application bundle.

## Technology

ZMark uses **Rust** for the application, **GPUI** for its interface, **pulldown-cmark** for Markdown parsing, **syntect** for code highlighting, and **rusty-mermaid** for diagram rendering. Images are decoded with the Rust `image` crate.

See [architecture decisions](docs/adr/README.md) and the [project glossary](GLOSSARY.md) for more detail.
