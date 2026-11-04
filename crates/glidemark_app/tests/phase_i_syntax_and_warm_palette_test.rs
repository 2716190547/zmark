use glidemark_app::state::{AppState, DocKey, ViewMode};
use glidemark_app::theme::ThemeColors;
use glidemark_app::views::editor::MarkdownTokenKind;
use glidemark_app::views::preview::{apply_selection_to_highlights, obs};
use glidemark_app::views::workspace::{SyntaxBatchRequest, WorkspaceView};
use glidemark_core::document::Document;
use glidemark_core::syntax::{
    parse_code_spans, parse_code_spans_bounded, reset_test_clock, resolve_canonical_language,
    CodeKey, ParseResult, SyntaxCache, SyntaxRole, MAX_CODE_BLOCK_BYTES,
    TEST_INITIAL_ADVANCE_MS, TEST_LINE_DELTAS_MS, TEST_MOCK_CLOCK_MS,
};
use gpui::{HighlightStyle, TestAppContext};
use std::sync::atomic::Ordering;
use std::sync::Arc;

static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Test 1: Canonical language alias table and fallbacks per Phase I specification
#[test]
fn test_phase_i_canonical_language_aliases_and_plain_fallbacks() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    // 1. Canonical aliases table
    let expected_mappings = [
        ("xml", Some("XML")),
        ("XML", Some("XML")),
        ("html", Some("HTML")),
        ("htm", Some("HTML")),
        ("java", Some("Java")),
        ("JAVA", Some("Java")),
        ("python", Some("Python")),
        ("py", Some("Python")),
        ("javascript", Some("JavaScript")),
        ("js", Some("JavaScript")),
        ("json", Some("JSON")),
        ("yaml", Some("YAML")),
        ("yml", Some("YAML")),
        ("sql", Some("SQL")),
        ("shell", Some("Bourne Again Shell (bash)")),
        ("sh", Some("Bourne Again Shell (bash)")),
        ("bash", Some("Bourne Again Shell (bash)")),
        ("rust", Some("Rust")),
        ("rs", Some("Rust")),
    ];

    for (raw, expected) in expected_mappings {
        assert_eq!(
            resolve_canonical_language(Some(raw)),
            expected,
            "Raw language '{}' must resolve to canonical {:?}",
            raw,
            expected
        );
    }

    // 2. Info string with extra parameters, e.g. "```rust,no_run" or "```python title='main.py'"
    assert_eq!(resolve_canonical_language(Some("rust,no_run")), Some("Rust"));
    assert_eq!(resolve_canonical_language(Some("python title='main.py'")), Some("Python"));
    assert_eq!(resolve_canonical_language(Some("  xml  ")), Some("XML"));

    // 3. Plain / empty fallbacks
    assert_eq!(resolve_canonical_language(None), None);
    assert_eq!(resolve_canonical_language(Some("")), None);
    assert_eq!(resolve_canonical_language(Some("   ")), None);
    assert_eq!(resolve_canonical_language(Some("text")), None);
    assert_eq!(resolve_canonical_language(Some("txt")), None);
    assert_eq!(resolve_canonical_language(Some("plain")), None);
    assert_eq!(resolve_canonical_language(Some("plaintext")), None);

    // 4. Unknown languages (e.g. typescript, toml, zsh) return None / Plain per Phase I boundary
    assert_eq!(resolve_canonical_language(Some("typescript")), None);
    assert_eq!(resolve_canonical_language(Some("ts")), None);
    assert_eq!(resolve_canonical_language(Some("toml")), None);
    assert_eq!(resolve_canonical_language(Some("zsh")), None);
    assert_eq!(resolve_canonical_language(Some("unknown_lang")), None);

    // Unknown language code block parses to single Plain span covering entire text
    let code = "const x: number = 42;\nconsole.log(x);";
    let spans = parse_code_spans(code, Some("typescript"));
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].role, SyntaxRole::Plain);
    assert_eq!(spans[0].range, 0..code.len());
}

/// Test 2: Maven XML Tag (Keyword) vs Attribute (Attribute) vs String vs Comment distinction
#[test]
fn test_phase_i_maven_xml_semantic_distinction() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let xml = r#"<!-- Maven POM configuration -->
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
    <modelVersion>4.0.0</modelVersion>
    <groupId>org.apache.maven</groupId>
    <artifactId>maven-core</artifactId>
    <version>3.9.0</version>
</project>"#;

    let spans = parse_code_spans(xml, Some("xml"));

    // 1. Comment
    let comment_span = spans
        .iter()
        .find(|s| s.role == SyntaxRole::Comment && xml[s.range.clone()].contains("Maven POM configuration"))
        .expect("Must have Comment span for XML comment");
    assert_eq!(comment_span.role, SyntaxRole::Comment);

    // 2. Tag names (Keyword)
    let project_tags: Vec<_> = spans
        .iter()
        .filter(|s| s.role == SyntaxRole::Keyword && &xml[s.range.clone()] == "project")
        .collect();
    assert!(!project_tags.is_empty(), "Tag 'project' must be Keyword");

    let group_id_tag = spans
        .iter()
        .find(|s| s.role == SyntaxRole::Keyword && &xml[s.range.clone()] == "groupId")
        .expect("Tag 'groupId' must be Keyword");
    assert_eq!(group_id_tag.role, SyntaxRole::Keyword);

    // 3. Attribute names (Attribute)
    let xmlns_attr = spans
        .iter()
        .find(|s| s.role == SyntaxRole::Attribute && &xml[s.range.clone()] == "xmlns")
        .expect("Attribute 'xmlns' must be Attribute");
    assert_eq!(xmlns_attr.role, SyntaxRole::Attribute);

    // 4. Attribute values (String)
    let attr_val = spans
        .iter()
        .find(|s| s.role == SyntaxRole::String && xml[s.range.clone()].contains("http://maven.apache.org/POM/4.0.0"))
        .expect("Attribute string must be String");
    assert_eq!(attr_val.role, SyntaxRole::String);

    // Invariant: contiguous byte reconstruction
    let reconstructed: String = spans.iter().map(|s| &xml[s.range.clone()]).collect();
    assert_eq!(reconstructed, xml, "Reconstructed text must match exactly byte-for-byte");
}

/// Test 3: Multi-line continuity across lines (comments and strings)
#[test]
fn test_phase_i_multiline_syntax_continuity() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let rust_code = r#"/*
 * Multi-line Rust block comment
 * continuing across lines
 */
pub fn calculate() -> i32 {
    let multiline_str = "hello\nworld\nfoo";
    42
}"#;

    let spans = parse_code_spans(rust_code, Some("rust"));

    // Multi-line comment should span across lines
    let comment_span = spans
        .iter()
        .find(|s| s.role == SyntaxRole::Comment && rust_code[s.range.clone()].contains("Multi-line Rust block comment"))
        .expect("Must have multi-line comment span");
    assert!(rust_code[comment_span.range.clone()].contains("continuing across lines"));

    // fn after multi-line comment must still be recognized as Keyword
    let fn_span = spans
        .iter()
        .find(|s| s.role == SyntaxRole::Keyword && &rust_code[s.range.clone()] == "fn")
        .expect("'fn' must be Keyword");
    assert_eq!(fn_span.role, SyntaxRole::Keyword);

    // Number 42 must be Number
    let num_span = spans
        .iter()
        .find(|s| s.role == SyntaxRole::Number && &rust_code[s.range.clone()] == "42")
        .expect("'42' must be Number");
    assert_eq!(num_span.role, SyntaxRole::Number);
}

/// Test 4: Threshold downgrades to Plain per Phase I boundary
#[test]
fn test_phase_i_threshold_downgrades_to_plain() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    // 1. > 64 KiB
    let huge_code = "let x = 1;\n".repeat(6000);
    assert!(huge_code.len() > MAX_CODE_BLOCK_BYTES);
    let spans_huge = parse_code_spans(&huge_code, Some("rust"));
    assert_eq!(spans_huge.len(), 1, "Huge block must return exactly 1 span");
    assert_eq!(spans_huge[0].role, SyntaxRole::Plain);
    assert_eq!(spans_huge[0].range, 0..huge_code.len());

    // 2. > 2000 physical lines
    let long_code = "\n".repeat(2005);
    let spans_long = parse_code_spans(&long_code, Some("rust"));
    assert_eq!(spans_long.len(), 1, "Block with > 2000 lines must downgrade to Plain");
    assert_eq!(spans_long[0].role, SyntaxRole::Plain);
    assert_eq!(spans_long[0].range, 0..long_code.len());

    // 3. Any line > 1000 unicode scalars
    let wide_line = format!("let s = \"{}\";\n", "a".repeat(1005));
    let spans_wide = parse_code_spans(&wide_line, Some("rust"));
    assert_eq!(spans_wide.len(), 1, "Block with line > 1000 unicode scalars must downgrade to Plain");
    assert_eq!(spans_wide[0].role, SyntaxRole::Plain);
    assert_eq!(spans_wide[0].range, 0..wide_line.len());
}

/// Test 5: Invariant: UTF-8 contiguous non-overlapping coverage with Tab, CRLF, CJK, and Emoji
#[test]
fn test_phase_i_utf8_contiguous_non_overlapping_coverage() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let complex_code = "fn main() {\r\n\tlet cjk = \"你好，世界 🦀\";\r\n\tlet tab_indent = \t123;\r\n}\r\n";
    let spans = parse_code_spans(complex_code, Some("rust"));

    let mut current_offset = 0;
    for span in &spans {
        assert_eq!(
            span.range.start, current_offset,
            "Gaps or overlapping spans detected at byte {}",
            current_offset
        );
        assert!(span.range.end > span.range.start, "Empty span detected");
        assert!(
            complex_code.is_char_boundary(span.range.start),
            "Span start must be a valid UTF-8 char boundary"
        );
        assert!(
            complex_code.is_char_boundary(span.range.end),
            "Span end must be a valid UTF-8 char boundary"
        );
        current_offset = span.range.end;
    }
    assert_eq!(
        current_offset,
        complex_code.len(),
        "Spans must completely cover the full byte range of the code payload"
    );

    let reconstructed: String = spans.iter().map(|s| &complex_code[s.range.clone()]).collect();
    assert_eq!(reconstructed, complex_code);
}

/// Test 6: Bounded LRU cache and theme independence
#[test]
fn test_phase_i_bounded_lru_cache_and_theme_independence() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut cache = SyntaxCache::new();
    let code = "def greet(name: str) -> str:\n    return f'Hello, {name}'";

    // 1. First call parses and inserts into cache
    let key = SyntaxCache::key(code, Some("python"));
    let spans1: Arc<[glidemark_core::syntax::SyntaxSpan]> = parse_code_spans(code, Some("python")).into();
    let retained = std::collections::HashSet::new();
    let inserted = cache.insert(key.clone(), spans1.clone(), &retained);
    assert!(inserted, "Cache must admit entry within budget");

    // 2. Second call returns the cached Arc without reparsing
    let spans2 = cache.get(&key).expect("Key must be present in cache");
    assert!(
        Arc::ptr_eq(&spans1, &spans2),
        "Identical code and language must return the exact same cached Arc"
    );

    // 3. Theme change does not invalidate syntax parse cache:
    // Color mapping is a pure projection from ThemeColors::syntax_color_for_role
    let theme_light = ThemeColors::default_palette(false);
    let theme_dark = ThemeColors::default_palette(true);

    for span in spans1.iter() {
        let color_light = theme_light.syntax_color_for_role(span.role);
        let color_dark = theme_dark.syntax_color_for_role(span.role);
        assert_eq!(color_light.a, 1.0);
        assert_eq!(color_dark.a, 1.0);
    }
}

/// Test 7: Preview and Editor consistency on the same code block
#[test]
fn test_phase_i_preview_and_editor_syntax_consistency() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let markdown_doc = [
        "# Sample Document",
        "```xml",
        "<project xmlns=\"http://maven.apache.org\">",
        "    <groupId>org.apache.maven</groupId>",
        "</project>",
        "```",
        "End of document",
    ];

    let xml_payload = "<project xmlns=\"http://maven.apache.org\">\n    <groupId>org.apache.maven</groupId>\n</project>";
    let preview_spans = parse_code_spans(xml_payload, Some("xml"));

    // 1. Editor tokenization via AST parser and code line mappings
    let full_doc = markdown_doc.join("\n");
    let doc = glidemark_core::ast::parse_markdown(&full_doc);
    assert_eq!(doc.code_blocks.len(), 1);
    let code_block = &doc.code_blocks[0];
    let spans = parse_code_spans(&code_block.key.payload, code_block.key.language);

    // Line 1: ```xml delimiter
    assert_eq!(code_block.fence_open_line, 1);

    // Line 2: <project xmlns="http://maven.apache.org">
    let line2_mapping = &code_block.lines[0];
    let line2_spans: Vec<glidemark_core::syntax::SyntaxSpan> = spans
        .iter()
        .filter_map(|s| {
            let overlap_start = s.range.start.max(line2_mapping.payload_range.start);
            let overlap_end = s.range.end.min(line2_mapping.payload_range.end);
            if overlap_start < overlap_end {
                Some(glidemark_core::syntax::SyntaxSpan {
                    range: (overlap_start - line2_mapping.payload_range.start)..(overlap_end - line2_mapping.payload_range.start),
                    role: s.role,
                })
            } else {
                None
            }
        })
        .collect();
    let line2_tokens = glidemark_app::views::editor::spans_to_code_tokens(markdown_doc[2], &line2_spans);
    assert!(
        line2_tokens.iter().any(|t| t.kind == MarkdownTokenKind::Keyword && t.text == "project"),
        "Editor must recognize 'project' tag as Keyword"
    );
    assert!(
        line2_tokens.iter().any(|t| t.kind == MarkdownTokenKind::Attribute && t.text == "xmlns"),
        "Editor must recognize 'xmlns' as Attribute"
    );
    assert!(
        line2_tokens.iter().any(|t| t.kind == MarkdownTokenKind::StringLiteral && t.text.contains("maven.apache.org")),
        "Editor must recognize attribute value as StringLiteral"
    );

    // Line 3: <groupId>org.apache.maven</groupId>
    let line3_mapping = &code_block.lines[1];
    let line3_spans: Vec<glidemark_core::syntax::SyntaxSpan> = spans
        .iter()
        .filter_map(|s| {
            let overlap_start = s.range.start.max(line3_mapping.payload_range.start);
            let overlap_end = s.range.end.min(line3_mapping.payload_range.end);
            if overlap_start < overlap_end {
                Some(glidemark_core::syntax::SyntaxSpan {
                    range: (overlap_start - line3_mapping.payload_range.start)..(overlap_end - line3_mapping.payload_range.start),
                    role: s.role,
                })
            } else {
                None
            }
        })
        .collect();
    let line3_tokens = glidemark_app::views::editor::spans_to_code_tokens(markdown_doc[3], &line3_spans);
    assert!(
        line3_tokens.iter().any(|t| t.kind == MarkdownTokenKind::Keyword && t.text == "groupId"),
        "Editor must recognize 'groupId' tag as Keyword"
    );

    // 2. Preview semantic spans
    assert!(
        preview_spans.iter().any(|s| s.role == SyntaxRole::Keyword && &xml_payload[s.range.clone()] == "project"),
        "Preview must recognize 'project' tag as Keyword"
    );
    assert!(
        preview_spans.iter().any(|s| s.role == SyntaxRole::Attribute && &xml_payload[s.range.clone()] == "xmlns"),
        "Preview must recognize 'xmlns' as Attribute"
    );

    // 3. Selection overlay without wiping foreground colors
    let theme = ThemeColors::default_palette(false);
    let syntax_highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = preview_spans
        .iter()
        .map(|s| {
            (
                s.range.clone(),
                HighlightStyle {
                    color: Some(theme.syntax_color_for_role(s.role)),
                    ..Default::default()
                },
            )
        })
        .collect();

    // Select middle range 5..25
    let selected = apply_selection_to_highlights(xml_payload, &syntax_highlights, 5..25, theme.selection_bg());
    assert!(!selected.is_empty());

    // Highlight spans inside the selection must have both background_color AND color preserved
    for (range, style) in &selected {
        if range.start >= 5 && range.end <= 25 {
            assert!(style.background_color.is_some(), "Selection background must be set");
            assert!(style.color.is_some(), "Syntax foreground color must NOT be wiped by selection");
        }
    }
}

/// Test 8: Real Workspace Preview Render & Foreground Text Color Observation
#[gpui::test]
async fn test_phase_i_real_preview_render_and_selection_overlay(cx: &mut TestAppContext) {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    obs::reset();
    let xml_doc = "# Title\n\n```xml\n<project xmlns=\"http://maven.apache.org\">\n    <groupId>org.apache.maven</groupId>\n</project>\n```\n";
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(xml_doc.to_string());
        let mut state = AppState::new(doc);
        state.parse_now();
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });

    // First draw to register visible region in preview list
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    // Run until deferred background syntax scheduler finishes parsing
    cx.run_until_parked();

    // Subsequent draw renders with syntax highlights
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    let highlights = obs::last_code_highlights().expect("Must record rendered preview code block highlights");
    assert!(!highlights.is_empty(), "Preview code block must render syntax highlights");

    let theme = ThemeColors::default_palette(false);
    let keyword_color = theme.syntax_keyword;
    let attribute_color = theme.syntax_attribute;
    let string_color = theme.syntax_string;

    // Check tag 'project' has keyword color
    let project_highlight = highlights.iter().find(|(r, h)| {
        r.start == 1 && r.end == 8 && h.color == Some(keyword_color)
    });
    assert!(project_highlight.is_some(), "Tag 'project' must be rendered with syntax_keyword color");

    // Check 'xmlns' has attribute color
    let xmlns_highlight = highlights.iter().find(|(r, h)| {
        r.start == 9 && r.end == 14 && h.color == Some(attribute_color)
    });
    assert!(xmlns_highlight.is_some(), "Attribute 'xmlns' must be rendered with syntax_attribute color");

    // Check attribute string has string color
    let string_highlight = highlights.iter().find(|(_r, h)| {
        h.color == Some(string_color)
    });
    assert!(string_highlight.is_some(), "Attribute value string must be rendered with syntax_string color");

    // Test selection overlay: simulate selection over xmlns attribute (char offset 9..14 in code block)
    cx.update(|_window, cx| {
        view.update(cx, |this, _cx| {
            this.preview_selection = Some(glidemark_app::views::preview::PreviewSelectionRange {
                anchor: glidemark_app::views::preview::PreviewCharPosition { block_ix: 1, char_offset: 9 },
                head: glidemark_app::views::preview::PreviewCharPosition { block_ix: 1, char_offset: 14 },
            });
        });
    });

    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    let selected_highlights = obs::last_code_highlights().expect("Must record highlights after selection");
    let selected_xmlns = selected_highlights.iter().find(|(r, _)| r.start == 9 && r.end == 14);
    let (_sel_range, sel_style) = selected_xmlns.expect("xmlns highlight must exist under selection");
    assert_eq!(sel_style.background_color, Some(theme.selection_bg()), "Selection background must overlay on highlighted span");
    assert_eq!(sel_style.color, Some(attribute_color), "Selection background must NOT wipe syntax foreground color");
}

/// Test 9: Real Workspace Editor Render & Token Kind Observation
#[gpui::test]
async fn test_phase_i_real_editor_render_tokens(cx: &mut TestAppContext) {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    obs::reset();
    let rust_doc = "# Title\n```rust\nfn main() {\n    let val = 42;\n    let msg = \"hello\";\n}\n```\n";
    let (_view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(rust_doc.to_string());
        let mut state = AppState::new(doc);
        state.set_view_mode(ViewMode::Source);
        WorkspaceView::new(state, cx)
    });

    // First draw to register visible region in preview list
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    // Run until deferred background syntax scheduler finishes parsing
    cx.run_until_parked();

    // Subsequent draw renders with syntax highlights
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    // Check physical line 2: "fn main() {"
    let line2_tokens = obs::editor_tokens(2).expect("Editor line 2 tokens must be logged");
    assert!(
        line2_tokens.iter().any(|t| t.kind == MarkdownTokenKind::Keyword && t.text == "fn"),
        "Editor must render 'fn' with Keyword token kind"
    );

    // Check physical line 3: "    let val = 42;"
    let line3_tokens = obs::editor_tokens(3).expect("Editor line 3 tokens must be logged");
    assert!(
        line3_tokens.iter().any(|t| t.kind == MarkdownTokenKind::Keyword && t.text == "let"),
        "Editor must render 'let' with Keyword token kind"
    );
    assert!(
        line3_tokens.iter().any(|t| t.kind == MarkdownTokenKind::Number && t.text == "42"),
        "Editor must render '42' with Number token kind"
    );

    // Check physical line 4: "    let msg = \"hello\";"
    let line4_tokens = obs::editor_tokens(4).expect("Editor line 4 tokens must be logged");
    assert!(
        line4_tokens.iter().any(|t| t.kind == MarkdownTokenKind::StringLiteral && t.text.contains("hello")),
        "Editor must render '\"hello\"' with StringLiteral token kind"
    );
}

/// Test 10: Cold start non-blocking initial render and asynchronous completion
#[gpui::test]
async fn test_phase_i_cold_start_nonblocking_and_async_refresh(cx: &mut TestAppContext) {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let doc_str = "```rust\nfn cold() -> i32 { 100 }\n```\n";
    let (_view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(doc_str.to_string());
        let mut state = AppState::new(doc);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });

    // Initial draw WITHOUT cx.run_until_parked(): must complete immediately without blocking
    let draw_start = std::time::Instant::now();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let draw_duration = draw_start.elapsed();
    assert!(draw_duration.as_millis() < 500, "Cold render must not block UI thread waiting for syntax parser");

    // Background executor finishes parsing
    cx.run_until_parked();

    // Subsequent draw has full syntax highlights
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let highlights = obs::last_code_highlights().expect("Must have highlights after background parse completes");
    assert!(!highlights.is_empty(), "Post-async draw must contain highlights");
}

/// Test 11: Cursor movement and scroll reuse tokens without reconstructing or reparsing
#[gpui::test]
async fn test_phase_i_scroll_and_cursor_reuse_without_reparsing(cx: &mut TestAppContext) {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let doc_str = "```rust\nfn test_reuse() {\n    let a = 1;\n}\n```\n";
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(doc_str.to_string());
        let mut state = AppState::new(doc);
        state.set_view_mode(ViewMode::Source);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    let (initial_blocks, initial_rev) = cx.update(|_window, cx| {
        let ws = view.read(cx);
        (ws.source_code_blocks.clone(), ws.source_code_blocks_rev)
    });
    assert!(!initial_blocks.is_empty());

    // Move cursor right: revision does NOT change
    cx.update(|_window, cx| {
        view.update(cx, |this, _cx| {
            this.state.editor.move_right();
        });
    });

    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    let (moved_blocks, moved_rev) = cx.update(|_window, cx| {
        let ws = view.read(cx);
        (ws.source_code_blocks.clone(), ws.source_code_blocks_rev)
    });
    assert_eq!(moved_rev, initial_rev, "Cursor movement must not bump revision");
    assert!(Arc::ptr_eq(&initial_blocks, &moved_blocks), "Source code blocks must be 100% reused across cursor moves");
}

/// Test 12a: Old doc completion is rejected by guard and does not pollute cache
#[gpui::test]
async fn test_phase_i_stale_doc_rejection_does_not_pollute_cache(cx: &mut TestAppContext) {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let doc_str = "```rust\nfn doc_test() {}\n```\n";
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(doc_str.to_string());
        let mut state = AppState::new(doc);
        state.set_view_mode(ViewMode::Source);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    let (current_doc, current_rev, current_req_id) = cx.update(|_window, cx| {
        let ws = view.read(cx);
        (ws.state.active.clone(), ws.state.editor.revision, ws.syntax_request_counter)
    });

    // Positive control: valid request with matching doc, rev, and req_id enters cache
    let valid_code = "fn valid_doc_positive_control() {}";
    let valid_key = CodeKey::from_raw(Some("rust"), Arc::from(valid_code));
    let valid_request = SyntaxBatchRequest {
        request_id: current_req_id,
        doc_key: current_doc.clone(),
        revision: current_rev,
        blocks: vec![valid_key.clone()],
    };
    cx.update(|_window, cx| {
        view.update(cx, |this, cx| {
            this.run_syntax_batch(valid_request, cx);
        });
    });
    cx.run_until_parked();

    let valid_in_cache = cx.update(|_window, cx| {
        let ws = view.read(cx);
        let mut cache = ws.syntax_cache.lock().unwrap();
        cache.get(&valid_key).is_some()
    });
    assert!(valid_in_cache, "Positive control request must enter cache");

    // Re-read current state
    let (_current_doc, current_rev, current_req_id) = cx.update(|_window, cx| {
        let ws = view.read(cx);
        (ws.state.active.clone(), ws.state.editor.revision, ws.syntax_request_counter)
    });

    // Stale doc request: different doc_key, BUT revision and request_id match exactly (no + 1!)
    let sentinel_doc = "fn should_be_rejected_doc() {}";
    let sentinel_key = CodeKey::from_raw(Some("rust"), Arc::from(sentinel_doc));
    let stale_doc_request = SyntaxBatchRequest {
        request_id: current_req_id,
        doc_key: Some(DocKey::Scratch(999999)),
        revision: current_rev,
        blocks: vec![sentinel_key.clone()],
    };

    cx.update(|_window, cx| {
        view.update(cx, |this, cx| {
            this.run_syntax_batch(stale_doc_request, cx);
        });
    });
    cx.run_until_parked();

    let cache_contains_doc = cx.update(|_window, cx| {
        let ws = view.read(cx);
        let mut cache = ws.syntax_cache.lock().unwrap();
        cache.get(&sentinel_key).is_some()
    });
    assert!(
        !cache_contains_doc,
        "Old doc completion must be rejected by guard and not pollute cache"
    );
}

/// Test 12b: Old revision completion is rejected by guard and does not pollute cache
#[gpui::test]
async fn test_phase_i_stale_revision_rejection_does_not_pollute_cache(cx: &mut TestAppContext) {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let doc_str = "```rust\nfn rev_test() {}\n```\n";
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(doc_str.to_string());
        let mut state = AppState::new(doc);
        state.set_view_mode(ViewMode::Source);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    // Advance editor revision by typing
    cx.update(|_window, cx| {
        view.update(cx, |this, cx| {
            this.state.editor.insert_text("// extra\n");
            this.schedule_syntax_parse(cx);
        });
    });
    cx.run_until_parked();

    let (current_doc_key, current_rev, _current_code_rev, current_req_id) = cx.update(|_window, cx| {
        let ws = view.read(cx);
        (
            ws.state.active.clone(),
            ws.state.editor.revision,
            ws.source_code_blocks_rev,
            ws.syntax_request_counter,
        )
    });

    // Positive control: valid request with matching doc, rev, and req_id enters cache
    let valid_code = "fn valid_rev_positive_control() {}";
    let valid_key = CodeKey::from_raw(Some("rust"), Arc::from(valid_code));
    let valid_request = SyntaxBatchRequest {
        request_id: current_req_id,
        doc_key: current_doc_key.clone(),
        revision: current_rev,
        blocks: vec![valid_key.clone()],
    };
    cx.update(|_window, cx| {
        view.update(cx, |this, cx| {
            this.run_syntax_batch(valid_request, cx);
        });
    });
    cx.run_until_parked();

    let valid_in_cache = cx.update(|_window, cx| {
        let ws = view.read(cx);
        let mut cache = ws.syntax_cache.lock().unwrap();
        cache.get(&valid_key).is_some()
    });
    assert!(valid_in_cache, "Positive control request must enter cache");

    // Re-read current state
    let (current_doc_key, current_rev, current_code_rev, current_req_id) = cx.update(|_window, cx| {
        let ws = view.read(cx);
        (
            ws.state.active.clone(),
            ws.state.editor.revision,
            ws.source_code_blocks_rev,
            ws.syntax_request_counter,
        )
    });

    // Stale revision request: revision is stale (e.g. current_rev.saturating_sub(1)), BUT doc_key and request_id match (no + 1!)
    let sentinel_rev = "fn should_be_rejected_rev() {}";
    let sentinel_key = CodeKey::from_raw(Some("rust"), Arc::from(sentinel_rev));
    let stale_rev_request = SyntaxBatchRequest {
        request_id: current_req_id,
        doc_key: current_doc_key.clone(),
        revision: current_rev.saturating_sub(1),
        blocks: vec![sentinel_key.clone()],
    };

    cx.update(|_window, cx| {
        view.update(cx, |this, cx| {
            this.run_syntax_batch(stale_rev_request, cx);
        });
    });
    cx.run_until_parked();

    let cache_contains_rev = cx.update(|_window, cx| {
        let ws = view.read(cx);
        let mut cache = ws.syntax_cache.lock().unwrap();
        cache.get(&sentinel_key).is_some()
    });
    assert!(
        !cache_contains_rev,
        "Old revision completion must be rejected by guard and not pollute cache"
    );

    let final_code_rev = cx.update(|_window, cx| view.read(cx).source_code_blocks_rev);
    assert_eq!(
        final_code_rev, current_code_rev,
        "source_code_blocks_rev must not be rolled back by stale response"
    );
}

/// Test 12c: Cancelled request does not enter cache and does not become permanent Plain
#[gpui::test]
async fn test_phase_i_cancellation_does_not_enter_cache_and_not_permanent_plain(cx: &mut TestAppContext) {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let doc_str = "```rust\nfn cancelled_test() { let x = 1; }\n```\n";
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(doc_str.to_string());
        let mut state = AppState::new(doc);
        state.set_view_mode(ViewMode::Source);
        WorkspaceView::new(state, cx)
    });

    let cancel_code = "fn to_be_cancelled() { let a = 99; }";
    let key = CodeKey::from_raw(Some("rust"), Arc::from(cancel_code));

    // Manually run a batch and immediately cancel it
    cx.update(|_window, cx| {
        view.update(cx, |this, cx| {
            this.syntax_request_counter += 1;
            let req = SyntaxBatchRequest {
                request_id: this.syntax_request_counter,
                doc_key: this.state.active.clone(),
                revision: this.state.editor.revision,
                blocks: vec![key.clone()],
            };
            this.run_syntax_batch(req, cx);
            this.syntax_cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        });
    });
    cx.run_until_parked();

    // Verify cancelled request did NOT enter cache
    let (in_cache, state_is_plain) = cx.update(|_window, cx| {
        let ws = view.read(cx);
        let mut cache = ws.syntax_cache.lock().unwrap();
        let in_cache = cache.get(&key).is_some();
        let block_state = ws.syntax_block_states.get(&key).copied();
        (in_cache, block_state)
    });
    assert!(!in_cache, "Cancelled request must NOT enter cache");
    assert_ne!(state_is_plain, Some(glidemark_app::views::workspace::SyntaxBlockState::HardLimit));

    // Now re-request the same block normally: it must be parsed and admitted with syntax highlighting
    cx.update(|_window, cx| {
        view.update(cx, |this, cx| {
            this.syntax_request_counter += 1;
            let req = SyntaxBatchRequest {
                request_id: this.syntax_request_counter,
                doc_key: this.state.active.clone(),
                revision: this.state.editor.revision,
                blocks: vec![key.clone()],
            };
            this.run_syntax_batch(req, cx);
        });
    });
    cx.run_until_parked();

    let cached_spans = cx.update(|_window, cx| {
        let ws = view.read(cx);
        let mut cache = ws.syntax_cache.lock().unwrap();
        cache.get(&key)
    });
    let spans = cached_spans.expect("Re-requested block must be admitted into cache");
    assert!(
        spans.iter().any(|s| s.role == SyntaxRole::Keyword),
        "Cancelled block re-requested must not become permanent Plain: {:?}",
        spans
    );
}

/// Test 12d: Latest pending request is executed and admitted
#[gpui::test]
async fn test_phase_i_latest_pending_request_is_executed_and_admitted(cx: &mut TestAppContext) {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let doc_str = "```rust\nfn initial() {}\n```\n";
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(doc_str.to_string());
        let mut state = AppState::new(doc);
        state.set_view_mode(ViewMode::Source);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    // Set request counter to 10 so saturating_sub(1) is guaranteed strictly smaller
    let (current_doc, current_rev, current_req_id) = cx.update(|_window, cx| {
        view.update(cx, |this, _cx| {
            this.syntax_request_counter = 10;
            (this.state.active.clone(), this.state.editor.revision, this.syntax_request_counter)
        })
    });

    // 1. Positive control: valid request with (current_doc, current_rev, current_req_id)
    let valid_code = "fn valid_req_id_positive_control() {}";
    let valid_key = CodeKey::from_raw(Some("rust"), Arc::from(valid_code));
    let valid_req = SyntaxBatchRequest {
        request_id: current_req_id,
        doc_key: current_doc.clone(),
        revision: current_rev,
        blocks: vec![valid_key.clone()],
    };
    cx.update(|_window, cx| {
        view.update(cx, |this, cx| {
            this.run_syntax_batch(valid_req, cx);
        });
    });
    cx.run_until_parked();

    let valid_in_cache = cx.update(|_window, cx| {
        let ws = view.read(cx);
        let mut cache = ws.syntax_cache.lock().unwrap();
        cache.get(&valid_key).is_some()
    });
    assert!(valid_in_cache, "Positive control request with matching req_id must enter cache");

    // 2. Stale request_id check: request_id is stale (current_req_id - 1), but doc_key and revision match
    let (current_doc, current_rev, current_req_id) = cx.update(|_window, cx| {
        let ws = view.read(cx);
        (ws.state.active.clone(), ws.state.editor.revision, ws.syntax_request_counter)
    });
    let stale_code = "fn should_be_rejected_stale_req_id() {}";
    let stale_key = CodeKey::from_raw(Some("rust"), Arc::from(stale_code));
    let stale_req = SyntaxBatchRequest {
        request_id: current_req_id.saturating_sub(1),
        doc_key: current_doc.clone(),
        revision: current_rev,
        blocks: vec![stale_key.clone()],
    };
    cx.update(|_window, cx| {
        view.update(cx, |this, cx| {
            this.run_syntax_batch(stale_req, cx);
        });
    });
    cx.run_until_parked();

    let stale_in_cache = cx.update(|_window, cx| {
        let ws = view.read(cx);
        let mut cache = ws.syntax_cache.lock().unwrap();
        cache.get(&stale_key).is_some()
    });
    assert!(
        !stale_in_cache,
        "Stale request_id must be rejected by guard and not enter cache"
    );

    // 3. Latest pending request is executed and admitted
    let code_first = "fn slow_first() { let x = 1; }";
    let key_first = CodeKey::from_raw(Some("rust"), Arc::from(code_first));
    let code_second = "fn latest_second() { let y = 2; }";
    let key_second = CodeKey::from_raw(Some("rust"), Arc::from(code_second));

    // Launch first batch, then queue second batch while first is active
    cx.update(|_window, cx| {
        view.update(cx, |this, cx| {
            this.syntax_request_counter += 1;
            let req1 = SyntaxBatchRequest {
                request_id: this.syntax_request_counter,
                doc_key: this.state.active.clone(),
                revision: this.state.editor.revision,
                blocks: vec![key_first.clone()],
            };
            this.run_syntax_batch(req1, cx);

            // While active, queue second request
            this.syntax_request_counter += 1;
            let req2 = SyntaxBatchRequest {
                request_id: this.syntax_request_counter,
                doc_key: this.state.active.clone(),
                revision: this.state.editor.revision,
                blocks: vec![key_second.clone()],
            };
            this.syntax_cancel_flag.store(true, Ordering::Relaxed);
            this.syntax_pending_request = Some(req2);
        });
    });

    cx.run_until_parked();

    // Verify latest pending request was executed and admitted
    let cached_second = cx.update(|_window, cx| {
        let ws = view.read(cx);
        let mut cache = ws.syntax_cache.lock().unwrap();
        cache.get(&key_second)
    });
    let spans2 = cached_second.expect("Latest pending request must be executed and admitted into cache");
    assert!(
        spans2.iter().any(|s| s.role == SyntaxRole::Keyword),
        "Latest pending request spans must be highlighted: {:?}",
        spans2
    );
}

/// Test 13: 8 / 17 / 33 / 35 small block tests with both sides colored, 1 parse per key, and loop prevention
#[gpui::test]
async fn test_phase_i_33_blocks_cache_thrashing_infinite_render_loop(cx: &mut TestAppContext) {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    for &block_count in &[8, 17, 33, 35] {
        obs::reset();
        let mut doc_str = String::new();
        for i in 0..block_count {
            doc_str.push_str(&format!(
                "```rust\nfn b_{}() -> usize {{\n    let x = {};\n    x\n}}\n```\n",
                i, i
            ));
        }

        let (view, cx) = cx.add_window_view(|_window, cx| {
            let mut doc = Document::new_scratch();
            doc.update_buffer(doc_str);
            let mut state = AppState::new(doc);
            state.set_view_mode(ViewMode::Split);
            WorkspaceView::new(state, cx)
        });

        // 1. Initial draw & bounded parking (< 2s)
        let park1_start = std::time::Instant::now();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.run_until_parked();
        assert!(
            park1_start.elapsed().as_secs() < 2,
            "Parking for {} blocks must complete in < 2s without infinite loop",
            block_count
        );

        // Subsequent draw with syntax highlights
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });

        // 2. 1 parse per key: verify cache entries does not exceed block_count (no 2x duplication between editor and preview)
        cx.update(|_window, cx| {
            let ws = view.read(cx);
            let cache = ws.syntax_cache.lock().unwrap();
            assert!(
                cache.entries.len() <= block_count,
                "Cache entries count ({}) must not exceed block_count ({}) - single parse per key",
                cache.entries.len(),
                block_count
            );
        });

        // 3. Both sides colored check on first visible block
        let (has_editor_color, has_preview_color) = cx.update(|_window, cx| {
            let ws = view.read(cx);
            // Check editor tokens
            let tokens = obs::editor_tokens(1).unwrap_or_default();
            let editor_kw = tokens.iter().any(|t| t.kind == MarkdownTokenKind::Keyword && t.text == "fn");

            // Check preview highlights
            let highlights = obs::last_code_highlights().unwrap_or_default();
            let preview_kw = highlights.iter().any(|(_, h)| h.color == Some(ws.state.theme.syntax_keyword));

            (editor_kw, preview_kw)
        });
        assert!(
            has_editor_color,
            "Editor side must have syntax colors for block_count={}",
            block_count
        );
        assert!(
            has_preview_color,
            "Preview side must have syntax colors for block_count={}",
            block_count
        );
    }
}

/// Test 13b: Eviction when exceeding budget / capacity and recovery when scrolled back
#[gpui::test]
async fn test_phase_i_small_blocks_eviction_and_scroll_recovery(cx: &mut TestAppContext) {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let mut doc_str = String::new();
    for i in 0..35 {
        doc_str.push_str(&format!(
            "```rust\nfn block_{}() -> usize {{\n    let x = {};\n    x\n}}\n```\n",
            i, i
        ));
    }

    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(doc_str);
        let mut state = AppState::new(doc);
        state.set_view_mode(ViewMode::Source);
        WorkspaceView::new(state, cx)
    });

    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    let key0 = CodeKey::from_raw(
        Some("rust"),
        Arc::from("fn block_0() -> usize {\n    let x = 0;\n    x\n}\n"),
    );

    // Set cache budget to tightly hold only 5 entries to test eviction
    let sample_spans = cx.update(|_window, cx| {
        let ws = view.read(cx);
        let mut cache = ws.syntax_cache.lock().unwrap();
        let spans = cache.get(&key0).unwrap_or_else(|| Arc::from([]));
        let entry_heap = glidemark_core::syntax::SyntaxCache::entry_heap_bytes(&key0, &spans);
        let overhead = glidemark_core::syntax::heap_usable_size(5 * std::mem::size_of::<glidemark_core::syntax::CacheEntry>());
        cache.max_bytes = overhead + 5 * entry_heap;
        spans
    });

    // Insert additional blocks until block 0 is evicted
    cx.update(|_window, cx| {
        view.update(cx, |this, _cx| {
            let mut cache = this.syntax_cache.lock().unwrap();
            let retained = std::collections::HashSet::new();
            for i in 10..20 {
                let key_i = CodeKey::from_raw(
                    Some("rust"),
                    Arc::from(format!(
                        "fn block_{}() -> usize {{\n    let x = {};\n    x\n}}\n",
                        i, i
                    )),
                );
                cache.insert(key_i, sample_spans.clone(), &retained);
            }
        });
    });

    // Verify block 0 was evicted
    let contains_key0 = cx.update(|_window, cx| {
        let ws = view.read(cx);
        let mut cache = ws.syntax_cache.lock().unwrap();
        cache.get(&key0).is_some()
    });
    assert!(!contains_key0, "Block 0 must be evicted when capacity/budget is exceeded");

    // Scroll back / re-request block 0: verify it recovers!
    cx.update(|_window, cx| {
        view.update(cx, |this, cx| {
            this.syntax_request_counter += 1;
            let req = SyntaxBatchRequest {
                request_id: this.syntax_request_counter,
                doc_key: this.state.active.clone(),
                revision: this.state.editor.revision,
                blocks: vec![key0.clone()],
            };
            this.run_syntax_batch(req, cx);
        });
    });
    cx.run_until_parked();

    let recovered_key0 = cx.update(|_window, cx| {
        let ws = view.read(cx);
        let mut cache = ws.syntax_cache.lock().unwrap();
        cache.get(&key0).is_some()
    });
    assert!(recovered_key0, "Block 0 must recover when scrolled back / re-requested");
}

/// Test 14: Cumulative 50ms soft budget without slow-line resets (deterministic mock clock)
#[test]
fn test_phase_i_cumulative_soft_budget_without_resets() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    // 1. Initial / line 0 timeout returns BudgetExceeded
    reset_test_clock();
    TEST_MOCK_CLOCK_MS.store(1000, Ordering::Relaxed);
    TEST_INITIAL_ADVANCE_MS.store(60, Ordering::Relaxed);
    let res0 = parse_code_spans_bounded("let x = 1;\n", Some("rust"), None);
    assert_eq!(res0, ParseResult::BudgetExceeded);
    reset_test_clock();

    // 2. Multi-line timeout on middle line returns BudgetExceeded
    reset_test_clock();
    TEST_MOCK_CLOCK_MS.store(1000, Ordering::Relaxed);
    TEST_LINE_DELTAS_MS[0].store(10, Ordering::Relaxed);
    TEST_LINE_DELTAS_MS[1].store(45, Ordering::Relaxed);
    let code_3 = "let a = 1;\nlet b = 2;\nlet c = 3;\n";
    let res_mid = parse_code_spans_bounded(code_3, Some("rust"), None);
    assert_eq!(res_mid, ParseResult::BudgetExceeded);
    reset_test_clock();

    // 3. Multi-line timeout on final line returns BudgetExceeded
    reset_test_clock();
    TEST_MOCK_CLOCK_MS.store(1000, Ordering::Relaxed);
    TEST_LINE_DELTAS_MS[0].store(15, Ordering::Relaxed);
    TEST_LINE_DELTAS_MS[1].store(15, Ordering::Relaxed);
    TEST_LINE_DELTAS_MS[2].store(25, Ordering::Relaxed);
    let res_fin = parse_code_spans_bounded(code_3, Some("rust"), None);
    assert_eq!(res_fin, ParseResult::BudgetExceeded);
    reset_test_clock();

    // 4. Slow lines without resets (cumulative 25ms + 28ms = 53ms >= 50ms) returns BudgetExceeded
    reset_test_clock();
    TEST_MOCK_CLOCK_MS.store(1000, Ordering::Relaxed);
    TEST_LINE_DELTAS_MS[0].store(25, Ordering::Relaxed);
    TEST_LINE_DELTAS_MS[1].store(28, Ordering::Relaxed);
    TEST_LINE_DELTAS_MS[2].store(5, Ordering::Relaxed);
    let res_cum = parse_code_spans_bounded(code_3, Some("rust"), None);
    assert_eq!(res_cum, ParseResult::BudgetExceeded);
    reset_test_clock();

    // 5. Fast parsing without mock clock returns Ready
    let fast_res = parse_code_spans_bounded(code_3, Some("rust"), None);
    assert!(fast_res.is_ready());
}
