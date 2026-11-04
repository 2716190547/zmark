use glidemark_core::ast::parse_markdown;
use glidemark_core::syntax::{parse_code_spans_bounded, resolve_canonical_language, SyntaxRole};

static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn get_source_line(source: &str, line_idx: usize) -> &str {
    let mut current_idx = 0;
    let mut current_start = 0;
    let bytes = source.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'\n' {
            if current_idx == line_idx {
                return &source[current_start..=i];
            }
            current_idx += 1;
            current_start = i + 1;
        }
    }
    if current_idx == line_idx && current_start <= source.len() {
        return &source[current_start..];
    }
    panic!("Line index {} out of range for source len {}", line_idx, source.len());
}

// -----------------------------------------------------------------------------
// Test A1: 12-Case Mapping Table
// -----------------------------------------------------------------------------

#[test]
fn test_stage_a_12_case_mapping_table() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let cases = [
        ("1. Top-level LF", "```rust\nfn a() {}\n```\n"),
        ("2. Top-level CRLF", "```rust\r\nfn a() {}\r\n```\r\n"),
        ("3. List space nested", "- item\n\n  ```rust\n  fn a() {}\n  ```\n"),
        ("4. List space nested CRLF", "- item\r\n\r\n  ```rust\r\n  fn a() {}\r\n  ```\r\n"),
        ("5. List Tab nested", "- item\n\n\t```rust\n\tfn a() {}\n\t```\n"),
        ("6. BlockQuote nested", "> ```rust\n> fn a() {}\n> ```\n"),
        ("7. BlockQuote nested list", "> - item\n>   ```rust\n>   fn a() {}\n>   ```\n"),
        ("8. No trailing LF", "```rust\nfn a() {}\n```"),
        ("9. Indented code inside block", "```rust\n    let x = 1;\n```\n"),
        ("10. Empty code block", "```rust\n```\n"),
        ("11. Blank line in code", "```rust\nfn a() {}\n\nfn b() {}\n```\n"),
        ("12. CRLF blank line in code", "```rust\r\nfn a() {}\r\n\r\nfn b() {}\r\n```\r\n"),
    ];

    for (name, src) in cases {
        let doc = parse_markdown(src);
        assert_eq!(
            doc.code_blocks.len(),
            1,
            "Case '{}' must produce exactly 1 ParsedCodeBlock in doc.code_blocks",
            name
        );
        let block = &doc.code_blocks[0];
        assert!(block.is_valid, "Case '{}' block.is_valid must be true", name);

        let payload_lines: Vec<&str> = if block.key.payload.is_empty() {
            Vec::new()
        } else {
            let p = block.key.payload.strip_suffix('\n').unwrap_or(&block.key.payload);
            p.split('\n').collect()
        };

        assert_eq!(
            block.lines.len(),
            payload_lines.len(),
            "Case '{}' mapping lines count ({}) must match payload lines count ({})",
            name,
            block.lines.len(),
            payload_lines.len()
        );

        for (idx, m) in block.lines.iter().enumerate() {
            let src_line = get_source_line(src, m.source_line_idx);
            assert!(
                m.prefix_len + m.content_len <= src_line.len(),
                "Case '{}' line {} overrun: prefix_len={}, content_len={}, src_line.len()={}",
                name,
                idx,
                m.prefix_len,
                m.content_len,
                src_line.len()
            );

            let source_content = &src_line[m.prefix_len..m.prefix_len + m.content_len];
            assert_eq!(
                source_content, payload_lines[idx],
                "Case '{}' line {} source_code_content must match payload_code_content",
                name, idx
            );

            let line_eol = &src_line[m.prefix_len + m.content_len..];
            if !m.is_synthetic_lf {
                assert_eq!(
                    line_eol.len(),
                    m.eol_len,
                    "Case '{}' line {} EOL len mismatch: expected {}, got {}",
                    name,
                    idx,
                    m.eol_len,
                    line_eol.len()
                );
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Test A3: Canonical Bash/Shell Resolution
// -----------------------------------------------------------------------------

#[test]
fn test_stage_a_canonical_bash_resolution() {
    let _guard = TEST_MUTEX.lock().unwrap();
    // 1. Raw alias "bash" resolves to canonical "Bourne Again Shell (bash)"
    assert_eq!(
        resolve_canonical_language(Some("bash")),
        Some("Bourne Again Shell (bash)")
    );
    assert_eq!(
        resolve_canonical_language(Some("sh")),
        Some("Bourne Again Shell (bash)")
    );
    assert_eq!(
        resolve_canonical_language(Some("shell")),
        Some("Bourne Again Shell (bash)")
    );

    // 2. Canonical name "Bourne Again Shell (bash)" resolves to itself (never returns None!)
    assert_eq!(
        resolve_canonical_language(Some("Bourne Again Shell (bash)")),
        Some("Bourne Again Shell (bash)")
    );
    assert_eq!(
        resolve_canonical_language(Some("bourne again shell (bash)")),
        Some("Bourne Again Shell (bash)")
    );

    // 3. parse_code_spans_bounded with canonical name produces shell syntax spans (NOT plain fallback)
    let res_canonical = parse_code_spans_bounded("echo 'hello' && exit 0", Some("Bourne Again Shell (bash)"), None);
    let res_alias = parse_code_spans_bounded("echo 'hello' && exit 0", Some("bash"), None);
    assert_eq!(res_canonical, res_alias);
    let spans_canonical = res_canonical.spans().expect("canonical bash parsing should produce Ready spans");
    assert!(
        spans_canonical.iter().any(|s| s.role != SyntaxRole::Plain),
        "Bourne Again Shell (bash) must produce syntax-highlighted roles, not plain fallback: {:?}",
        spans_canonical
    );

    // 4. Verify "echo 1" canonical resolution matches alias without panicking or mismatch
    let res_1 = parse_code_spans_bounded("echo 1", Some("Bourne Again Shell (bash)"), None);
    let res_1_alias = parse_code_spans_bounded("echo 1", Some("bash"), None);
    assert_eq!(res_1, res_1_alias);
}
