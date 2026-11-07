use zmark_core::anchor::{SourceAnchor, SyncScrollMap};

#[test]
fn test_sync_scroll_empty() {
    let map = SyncScrollMap::new(vec![]);
    assert_eq!(map.source_line_to_preview_progress(10), (0, 0.0));
    assert_eq!(map.preview_to_source_line(0, 0.5), 1);
}

#[test]
fn test_sync_scroll_single_block() {
    // A single code block from line 5 to line 25
    let anchors = vec![SourceAnchor::new(0, 5, 25)];
    let map = SyncScrollMap::new(anchors);

    // Before start
    assert_eq!(map.source_line_to_preview_progress(1), (0, 0.0));
    // At start
    assert_eq!(map.source_line_to_preview_progress(5), (0, 0.0));
    // In middle (line 15)
    let (block_idx, progress) = map.source_line_to_preview_progress(15);
    assert_eq!(block_idx, 0);
    assert!((progress - 0.5).abs() < 1e-4);
    // At end
    assert_eq!(map.source_line_to_preview_progress(25), (0, 1.0));
    // Past end
    assert_eq!(map.source_line_to_preview_progress(30), (0, 1.0));

    // Reverse mapping
    assert_eq!(map.preview_to_source_line(0, 0.0), 5);
    assert_eq!(map.preview_to_source_line(0, 0.5), 15);
    assert_eq!(map.preview_to_source_line(0, 1.0), 25);
}

#[test]
fn test_sync_scroll_multiple_uneven_blocks() {
    // Block 0: Heading at line 1..1
    // (Line 2 is empty)
    // Block 1: Paragraph at line 3..4
    // (Line 5 is empty)
    // Block 2: Long code block at line 6..26
    let anchors = vec![
        SourceAnchor::new(0, 1, 1),
        SourceAnchor::new(1, 3, 4),
        SourceAnchor::new(2, 6, 26),
    ];
    let map = SyncScrollMap::new(anchors);

    // Block 0
    assert_eq!(map.source_line_to_preview_progress(1), (0, 0.0));

    // Inter-block gap (line 2) maps smoothly
    let (b_gap, p_gap) = map.source_line_to_preview_progress(2);
    assert_eq!(b_gap, 0);
    assert!(p_gap > 0.0);

    // Block 1: line 3 is 0.0, line 4 is 1.0
    assert_eq!(map.source_line_to_preview_progress(3), (1, 0.0));
    assert_eq!(map.source_line_to_preview_progress(4), (1, 1.0));

    // Block 2: line 16 is 50%
    let (b2, p2) = map.source_line_to_preview_progress(16);
    assert_eq!(b2, 2);
    assert!((p2 - 0.5).abs() < 1e-4);

    // Reverse mapping
    assert_eq!(map.preview_to_source_line(0, 0.0), 1);
    assert_eq!(map.preview_to_source_line(1, 0.0), 3);
    assert_eq!(map.preview_to_source_line(1, 1.0), 4);
    assert_eq!(map.preview_to_source_line(2, 0.5), 16);
}
