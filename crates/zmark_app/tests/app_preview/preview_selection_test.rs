use zmark_app::theme::ThemeColors;
use zmark_app::views::preview::{
    apply_selection_to_highlights, char_range_to_byte_range, detect_edge_autoscroll,
    serialize_preview_selection, PreviewCharPosition, PreviewSelection,
    PreviewSelectionRange,
};
use zmark_core::ast::{parse_markdown, BlockElement, InlineGroup, InlineItem, ListItem, ParsedDocument};
use gpui::FontWeight;
use std::hash::{Hash, Hasher};

// =========================================================================
// 1. Plain text serialization tests (Headings, Paragraphs, Code, Lists, Tables, Quotes, Links, Images)
// =========================================================================

#[test]
fn test_plain_text_heading_and_paragraph() {
    let doc = parse_markdown("# Top Title\n\n## Sub Title\n\nThis is a standard paragraph with **bold** and *italic* text.");
    assert_eq!(doc.blocks.len(), 3);

    assert_eq!(doc.blocks[0].to_plain_text(), "Top Title");
    assert_eq!(doc.blocks[1].to_plain_text(), "Sub Title");
    assert_eq!(
        doc.blocks[2].to_plain_text(),
        "This is a standard paragraph with bold and italic text."
    );
}

#[test]
fn test_plain_text_fenced_code_preserves_indent_and_newlines() {
    let source = "```rust\nfn calculate(x: i32) -> i32 {\n    let y = x * 2;\n    y + 1\n}\n```";
    let doc = parse_markdown(source);
    assert_eq!(doc.blocks.len(), 1);

    let expected_code = "fn calculate(x: i32) -> i32 {\n    let y = x * 2;\n    y + 1\n}\n";
    assert_eq!(doc.blocks[0].to_plain_text(), expected_code);
}

#[test]
fn test_plain_text_lists_ordered_unordered_and_tasklists() {
    let unord_doc = parse_markdown("- Apple\n- Banana\n- Cherry");
    assert_eq!(unord_doc.blocks[0].to_plain_text(), "- Apple\n- Banana\n- Cherry");

    let ord_doc = parse_markdown("1. First step\n2. Second step\n3. Third step");
    assert_eq!(
        ord_doc.blocks[0].to_plain_text(),
        "1. First step\n2. Second step\n3. Third step"
    );

    let task_doc = parse_markdown("- [x] Completed task\n- [ ] Pending task");
    assert_eq!(
        task_doc.blocks[0].to_plain_text(),
        "- [x] Completed task\n- [ ] Pending task"
    );
}

#[test]
fn test_plain_text_nested_list() {
    let nested_doc = parse_markdown("- Parent item 1\n  - Child item 1A\n  - Child item 1B\n- Parent item 2");
    let serialized = nested_doc.blocks[0].to_plain_text();
    let expected = "- Parent item 1\n  - Child item 1A\n  - Child item 1B\n- Parent item 2";
    assert_eq!(serialized, expected);
}

#[test]
fn test_plain_text_table_tab_separated() {
    let table_md = "| Name | Role | Status |\n|---|---|---|\n| Alice | Lead | Active |\n| Bob | Reviewer | Offline |";
    let doc = parse_markdown(table_md);
    assert_eq!(doc.blocks.len(), 1);

    let serialized = doc.blocks[0].to_plain_text();
    let expected = "Name\tRole\tStatus\nAlice\tLead\tActive\nBob\tReviewer\tOffline";
    assert_eq!(serialized, expected);
}

#[test]
fn test_plain_text_blockquote() {
    let bq = BlockElement::BlockQuote(vec![
        BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text("First line in quote".into())])),
        BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text("Second line in quote".into())])),
    ]);

    let serialized = bq.to_plain_text();
    assert_eq!(serialized, "First line in quote\nSecond line in quote");
}

#[test]
fn test_links_copy_text_only_without_urls() {
    let doc = parse_markdown("Check out [ZMark Homepage](https://zmark.app) for updates.");
    let text = doc.blocks[0].to_plain_text();
    assert_eq!(text, "Check out ZMark Homepage for updates.");
    assert!(!text.contains("https://zmark.app"));
}

#[test]
fn test_images_copy_alt_text() {
    let doc = parse_markdown("Architecture diagram: ![System Flowchart](assets/diagram.png) here.");
    let text = doc.blocks[0].to_plain_text();
    assert_eq!(text, "Architecture diagram: System Flowchart here.");
    assert!(!text.contains("assets/diagram.png"));
}

#[test]
fn test_rule_returns_empty_string() {
    assert_eq!(BlockElement::Rule.to_plain_text(), "");
}

#[test]
fn test_multi_block_selection_forward_and_reverse() {
    let md = "# Title\n\nIntro paragraph.\n\n```python\nprint('hello')\n```\n\nOutro paragraph.";
    let doc = parse_markdown(md);
    assert_eq!(doc.blocks.len(), 4);

    let forward_text = doc.extract_plain_text(0, 2);
    let expected = "Title\n\nIntro paragraph.\n\nprint('hello')\n";
    assert_eq!(forward_text, expected);

    let reverse_text = doc.extract_plain_text(2, 0);
    assert_eq!(reverse_text, expected);

    assert_eq!(doc.extract_plain_text(1, 1), "Intro paragraph.");

    let clamped_text = doc.extract_plain_text(0, 100);
    assert_eq!(
        clamped_text,
        "Title\n\nIntro paragraph.\n\nprint('hello')\n\nOutro paragraph."
    );

    assert_eq!(doc.extract_plain_text(50, 100), "");
}

#[test]
fn test_multi_block_selection_skips_rules_and_compacts_consecutive_lists() {
    let mut blocks = Vec::new();
    blocks.push(BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text("Intro".into())])));
    blocks.push(BlockElement::Rule);
    blocks.push(BlockElement::List {
        ordered: false,
        items: vec![ListItem {
            checked: None,
            body: vec![BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text("Item 1".into())]))],
        }],
    });
    blocks.push(BlockElement::List {
        ordered: true,
        items: vec![ListItem {
            checked: None,
            body: vec![BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text("Item 2".into())]))],
        }],
    });

    let doc = ParsedDocument {
        blocks,
        anchors: Vec::new(),
        code_blocks: std::sync::Arc::new(Vec::new()),
    };

    let text = doc.extract_plain_text(0, 3);
    assert_eq!(text, "Intro\n\n- Item 1\n1. Item 2");
}

// =========================================================================
// 2. Ticket R1: Character-level selection & Bug reproduction
// =========================================================================

#[test]
fn test_reproduce_previous_block_only_bug() {
    // BUG REPRODUCTION:
    // Previously, preview selection had no character-level representation.
    // Selection was only block-level `(start: usize, end: usize)`.
    // When attempting to select partial text (e.g. chars 4..15 of a paragraph),
    // the old API could only extract the ENTIRE block (all 44 characters),
    // failing character-level selection completely.
    let md = "The quick brown fox jumps over the lazy dog.";
    let doc = parse_markdown(md);

    // Old block-level extraction: extracted everything
    let old_block_level = doc.extract_plain_text(0, 0);
    assert_eq!(old_block_level, "The quick brown fox jumps over the lazy dog.");
    assert_ne!(old_block_level, "quick brown");

    // NEW Character-level extraction: extracts precisely "quick brown" (chars 4..15)
    let char_level = doc.extract_plain_text_char_range(0, 4, 0, 15);
    assert_eq!(char_level, "quick brown");

    // Verified via PreviewSelection helper
    let sel = PreviewSelection::new(
        PreviewCharPosition::new(0, 4),
        PreviewCharPosition::new(0, 15),
    );
    assert_eq!(serialize_preview_selection(&doc, sel), "quick brown");
}

#[test]
fn test_character_level_mid_paragraph_start_and_end() {
    let md = "ZMark is a modern Markdown editor designed for desktop.";
    let doc = parse_markdown(md);

    // Mid-paragraph start and end: "modern Markdown editor"
    // "ZMark is a " is 11 chars
    // "modern Markdown editor" is 22 chars -> ends at 11 + 22 = 33
    let text = doc.extract_plain_text_char_range(0, 11, 0, 33);
    assert_eq!(text, "modern Markdown editor");
}

#[test]
fn test_character_level_single_block_partial_selection() {
    let md = "# System Architecture Guide";
    let doc = parse_markdown(md);

    // "System" -> chars 0..6
    assert_eq!(doc.extract_plain_text_char_range(0, 0, 0, 6), "System");

    // "Architecture" -> chars 7..19
    assert_eq!(doc.extract_plain_text_char_range(0, 7, 0, 19), "Architecture");

    // "Guide" -> chars 20..25
    assert_eq!(doc.extract_plain_text_char_range(0, 20, 0, 25), "Guide");
}

#[test]
fn test_character_level_reverse_dragging() {
    let md = "The quick brown fox jumps over the lazy dog.";
    let doc = parse_markdown(md);

    // Reverse drag: anchor is at char 15, head dragged back to char 4
    let sel_rev = PreviewSelection::new(
        PreviewCharPosition::new(0, 15),
        PreviewCharPosition::new(0, 4),
    );
    assert_eq!(
        sel_rev.normalized(),
        (PreviewCharPosition::new(0, 4), PreviewCharPosition::new(0, 15))
    );
    assert_eq!(serialize_preview_selection(&doc, sel_rev), "quick brown");

    // Reverse drag across multiple blocks
    let multi_md = "First block text.\n\nSecond block text.\n\nThird block text.";
    let multi_doc = parse_markdown(multi_md);

    // Dragged from Block 2 char 5 backwards to Block 0 char 6
    let cross_rev = PreviewSelection::new(
        PreviewCharPosition::new(2, 5),
        PreviewCharPosition::new(0, 6),
    );
    let serialized = serialize_preview_selection(&multi_doc, cross_rev);
    assert_eq!(serialized, "block text.\n\nSecond block text.\n\nThird");
}

#[test]
fn test_cross_block_spanning_headings_paragraphs_lists_tables_and_code() {
    let md = r#"# Main Heading

Introductory paragraph explaining the system.

- List item alpha
- List item beta

| Header A | Header B |
|---|---|
| Cell 1 | Cell 2 |

```rust
fn execute() {
    let result = 42;
}
```"#;

    let doc = parse_markdown(md);
    assert_eq!(doc.blocks.len(), 5);

    // Select spanning:
    // Start: middle of heading (char 5 -> "Heading")
    // End: middle of code block (inside "fn execute()")
    let sel = PreviewSelection::new(
        PreviewCharPosition::new(0, 5),
        PreviewCharPosition::new(4, 12),
    );

    let text = serialize_preview_selection(&doc, sel);

    // Heading tail
    assert!(text.starts_with("Heading\n\n"));
    // Full paragraph included
    assert!(text.contains("Introductory paragraph explaining the system.\n\n"));
    // List included
    assert!(text.contains("- List item alpha\n- List item beta\n\n"));
    // Table with tab separation included
    assert!(text.contains("Header A\tHeader B\nCell 1\tCell 2\n\n"));
    // Code block prefix included
    assert!(text.ends_with("fn execute()"));
}

#[test]
fn test_table_partial_selection_with_tab_separation() {
    let table_md = "| Name | Role | Status |\n|---|---|---|\n| Alice | Lead | Active |\n| Bob | Reviewer | Offline |";
    let doc = parse_markdown(table_md);

    // Table text is: "Name\tRole\tStatus\nAlice\tLead\tActive\nBob\tReviewer\tOffline"
    // Select from char 0 to char 9 ("Name\tRole")
    let sel = PreviewSelection::new(
        PreviewCharPosition::new(0, 0),
        PreviewCharPosition::new(0, 9),
    );
    let text = serialize_preview_selection(&doc, sel);
    assert_eq!(text, "Name\tRole");
    assert!(text.contains('\t'));

    // Select across rows
    // "Name\tRole\tStatus\nAlice" -> chars 0..22
    let sel_row = PreviewSelection::new(
        PreviewCharPosition::new(0, 0),
        PreviewCharPosition::new(0, 22),
    );
    let text_row = serialize_preview_selection(&doc, sel_row);
    assert_eq!(text_row, "Name\tRole\tStatus\nAlice");
}

#[test]
fn test_indented_code_block_with_whitespace_preserved() {
    let source = "```rust\nfn calculate() {\n    let val = 100;\n    val * 2\n}\n```";
    let doc = parse_markdown(source);

    // In plain text, "fn calculate() {\n" is 17 chars
    // Line 2: "    let val = 100;\n" (19 chars)
    // Select chars 17..36 -> precisely "    let val = 100;\n"
    let sel = PreviewSelection::new(
        PreviewCharPosition::new(0, 17),
        PreviewCharPosition::new(0, 36),
    );
    let text = serialize_preview_selection(&doc, sel);
    assert_eq!(text, "    let val = 100;\n");
    assert!(text.starts_with("    "));
}

// =========================================================================
// 3. Highlight styling generation tests with selection_bg()
// =========================================================================

#[test]
fn test_highlight_styling_generation_with_selection_bg() {
    let theme = ThemeColors::default_palette(false);
    let text = "Hello brave new world";

    // 1. Plain text partial selection (chars 6..15: "brave new")
    let highlights = apply_selection_to_highlights(
        text,
        &[],
        6..15,
        theme.selection_bg(),
    );
    assert_eq!(highlights.len(), 1);
    assert_eq!(highlights[0].0, 6..15);
    assert_eq!(highlights[0].1.background_color, Some(theme.selection_bg()));

    // 2. Merging selection over existing styled inlines (e.g. Bold over "brave" at 6..11)
    let existing = vec![(
        6..11,
        gpui::HighlightStyle {
            font_weight: Some(FontWeight::BOLD),
            ..Default::default()
        },
    )];

    // Selection 9..15 covers part of bold ("ve") and plain text (" new")
    let merged = apply_selection_to_highlights(
        text,
        &existing,
        9..15,
        theme.selection_bg(),
    );

    // Should produce 3 disjoint, sorted segments:
    // 1) 6..9: Bold only
    // 2) 9..11: Bold + Selection background
    // 3) 11..15: Selection background only
    assert_eq!(merged.len(), 3);
    assert_eq!(merged[0].0, 6..9);
    assert_eq!(merged[0].1.font_weight, Some(FontWeight::BOLD));
    assert_eq!(merged[0].1.background_color, None);

    assert_eq!(merged[1].0, 9..11);
    assert_eq!(merged[1].1.font_weight, Some(FontWeight::BOLD));
    assert_eq!(merged[1].1.background_color, Some(theme.selection_bg()));

    assert_eq!(merged[2].0, 11..15);
    assert_eq!(merged[2].1.font_weight, None);
    assert_eq!(merged[2].1.background_color, Some(theme.selection_bg()));
}


#[test]
fn test_char_range_to_byte_range_mapping_multibyte_utf8() {
    // "你好，ZMark 🚀 世界"
    // 你: 3 bytes (char 0)
    // 好: 3 bytes (char 1)
    // ，: 3 bytes (char 2)
    // Z-M-a-r-k: 5 bytes (chars 3..8)
    //  : 1 byte (char 8)
    // 🚀: 4 bytes (char 9)
    //  : 1 byte (char 10)
    // 世: 3 bytes (char 11)
    // 界: 3 bytes (char 12)
    let text = "你好，ZMark 🚀 世界";

    // Chars 0..2 -> "你好"
    let b_range_cjk = char_range_to_byte_range(text, 0..2);
    assert_eq!(b_range_cjk, 0..6);
    assert_eq!(&text[b_range_cjk], "你好");

    // Chars 3..8 -> "ZMark"
    let b_range_ascii = char_range_to_byte_range(text, 3..8);
    assert_eq!(b_range_ascii, 9..14);
    assert_eq!(&text[b_range_ascii], "ZMark");

    // Char 9..10 -> "🚀"
    let b_range_emoji = char_range_to_byte_range(text, 9..10);
    assert_eq!(b_range_emoji, 15..19);
    assert_eq!(&text[b_range_emoji], "🚀");
}

#[test]
fn test_preview_selection_char_range_for_block() {
    // Selection spanning from Block 1 char 10 to Block 3 char 5
    let sel = PreviewSelection::new(
        PreviewCharPosition::new(1, 10),
        PreviewCharPosition::new(3, 5),
    );

    // Block 0: outside
    assert_eq!(sel.char_range_for_block(0, 50), None);

    // Block 1: start block -> 10..50
    assert_eq!(sel.char_range_for_block(1, 50), Some(10..50));

    // Block 2: middle block -> entire 0..50
    assert_eq!(sel.char_range_for_block(2, 50), Some(0..50));

    // Block 3: end block -> 0..5
    assert_eq!(sel.char_range_for_block(3, 50), Some(0..5));

    // Block 4: outside
    assert_eq!(sel.char_range_for_block(4, 50), None);

    // Single block partial selection
    let sel_single = PreviewSelection::new(
        PreviewCharPosition::new(2, 8),
        PreviewCharPosition::new(2, 16),
    );
    assert_eq!(sel_single.char_range_for_block(2, 50), Some(8..16));
    assert_eq!(sel_single.char_range_for_block(1, 50), None);

    // Collapsed selection returns None
    let sel_empty = PreviewSelection::new(
        PreviewCharPosition::new(2, 8),
        PreviewCharPosition::new(2, 8),
    );
    assert_eq!(sel_empty.char_range_for_block(2, 50), None);
}

#[test]
fn test_detect_edge_autoscroll() {
    let top = 0.0;
    let height = 800.0;

    // Within top 40px edge zone -> negative velocity (scroll up)
    let top_vel = detect_edge_autoscroll(10.0, top, height);
    assert!(top_vel.is_some());
    assert!(top_vel.unwrap() < 0.0);

    // Within bottom 40px edge zone (>= 760.0) -> positive velocity (scroll down)
    let bot_vel = detect_edge_autoscroll(780.0, top, height);
    assert!(bot_vel.is_some());
    assert!(bot_vel.unwrap() > 0.0);

    // Middle zone -> None
    assert_eq!(detect_edge_autoscroll(400.0, top, height), None);
    // Preserve the old stage B non-zero-origin cases without creating a window.
    assert!(detect_edge_autoscroll(400.0, 38.0, 762.0).is_none());
    assert!(detect_edge_autoscroll(45.0, 38.0, 762.0).unwrap() < 0.0);
    assert!(detect_edge_autoscroll(780.0, 38.0, 762.0).unwrap() > 0.0);
}

// =========================================================================
// 4. Backward-compatibility & Helper methods tests
// =========================================================================

#[test]
fn test_preview_selection_range_methods() {
    let range_fwd = PreviewSelection::new(
        PreviewCharPosition::new(2, 0),
        PreviewCharPosition::new(5, 10),
    );
    assert_eq!(range_fwd.anchor, PreviewCharPosition::new(2, 0));
    assert_eq!(range_fwd.head, PreviewCharPosition::new(5, 10));
    assert_eq!(
        range_fwd.normalized(),
        (PreviewCharPosition::new(2, 0), PreviewCharPosition::new(5, 10))
    );
    let (sf, ef) = range_fwd.normalized();
    assert!(!(sf.block_ix <= 1 && 1 <= ef.block_ix));
    assert!(sf.block_ix <= 2 && 2 <= ef.block_ix);
    assert!(sf.block_ix <= 4 && 4 <= ef.block_ix);
    assert!(sf.block_ix <= 5 && 5 <= ef.block_ix);
    assert!(!(sf.block_ix <= 6 && 6 <= ef.block_ix));

    let range_rev = PreviewSelection::new(
        PreviewCharPosition::new(5, 10),
        PreviewCharPosition::new(2, 0),
    );
    assert_eq!(
        range_rev.normalized(),
        (PreviewCharPosition::new(2, 0), PreviewCharPosition::new(5, 10))
    );
    let (s, e) = range_rev.normalized();
    assert!(!(s.block_ix <= 1 && 1 <= e.block_ix));
    assert!(s.block_ix <= 2 && 2 <= e.block_ix);
    assert!(s.block_ix <= 5 && 5 <= e.block_ix);
    assert!(!(s.block_ix <= 6 && 6 <= e.block_ix));

    let default_range = PreviewSelectionRange::default();
    assert_eq!(
        default_range.normalized(),
        (PreviewCharPosition::default(), PreviewCharPosition::default())
    );
    let (ds, de) = default_range.normalized();
    assert!(ds.block_ix == 0 && de.block_ix == 0);
    assert!(!(ds.block_ix <= 1 && 1 <= de.block_ix));
}

#[test]
fn test_serialize_preview_selection_helper() {
    let md = "# Heading 1\n\nParagraph 1\n\nParagraph 2";
    let doc = parse_markdown(md);

    let range = PreviewSelection::new(
        PreviewCharPosition::new(1, 0),
        PreviewCharPosition::new(2, usize::MAX),
    );
    let serialized = serialize_preview_selection(&doc, range);
    assert_eq!(serialized, "Paragraph 1\n\nParagraph 2");

    let range_rev = PreviewSelection::new(
        PreviewCharPosition::new(2, usize::MAX),
        PreviewCharPosition::new(0, 0),
    );
    let serialized_rev = serialize_preview_selection(&doc, range_rev);
    assert_eq!(serialized_rev, "Heading 1\n\nParagraph 1\n\nParagraph 2");
}

#[test]
fn test_code_block_copy_hash_independence() {
    let code_sample = "let mut score = 42;\nscore += 10;";
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code_sample.hash(&mut hasher);
    let expected_hash = hasher.finish();

    assert_ne!(expected_hash, 0);
    let code_to_copy = code_sample.to_string();
    assert_eq!(code_to_copy, code_sample);
}
