# Self-Contained HTML Export Over Embedded PDF Engines

We export documents as standalone HTML files bundling all styling and highlighted code instead of embedding a headless browser or heavy native PDF engine.

## Considered Options

- **Embedded Headless Chromium / WebKit**: Generates pixel-perfect PDFs directly from within the app, but balloons binary size by 50MB+ and incurs massive runtime overhead.
- **Typst / HarfBuzz Native PDF Generation**: High-fidelity PDF generation in pure Rust, but requires writing a custom typesetting translation layer from Markdown AST to Typst syntax.
- **Self-Contained HTML**: Generates a single `.html` file with embedded modern CSS typography and Tree-sitter syntax highlighting, allowing users to print or save as vector PDF natively from their system browser.

## Consequences

- Keeps app binary lightweight (<25MB) and zero-dependency.
- Offloads complex print drivers and pagination layout to the operating system's browser engine.
