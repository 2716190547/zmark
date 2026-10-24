# Separation of glidemark_core and glidemark_app

We isolate all Markdown tokenization, source anchor mapping, configuration, and export logic into a pure-Rust `glidemark_core` crate with zero GPUI dependencies, leaving `glidemark_app` purely responsible for windowing, layout, and event handling.

## Considered Options

- **Single Binary Crate**: Simplest to write initially, but tethers AST parsing and business logic directly to GPUI's rapid and breaking API release cycles.
- **Multi-Crate Workspace (`glidemark_core` + `glidemark_app`)**: Isolates pure domain logic; allows unit testing AST algorithms, anchor calculations, and HTML export headlessly without initializing a GPU window or Metal context.

## Consequences

- Faster CI/CD test execution because `glidemark_core` tests run headlessly on any platform without GPU drivers.
- Complete safety against GPUI refactors breaking Markdown domain invariants.
