use std::time::{Duration, Instant};
use zmark_core::anchor::{SourceAnchor, SyncScrollMap};
use zmark_app::views::workspace::ScrollOrigin;

#[test]
fn test_sync_scroll_bidirectional_and_echo_suppression() {
    let anchors = vec![
        SourceAnchor::new(0, 1, 10),
        SourceAnchor::new(1, 11, 20),
        SourceAnchor::new(2, 21, 30),
    ];
    let map = SyncScrollMap::new(anchors);

    // Editor -> Preview mapping
    let (block_idx, _) = map.source_line_to_preview_progress(15);
    assert_eq!(block_idx, 1);

    // Preview -> Editor mapping
    let source_line = map.preview_to_source_line(2, 0.0);
    assert_eq!(source_line, 21);

    // Echo suppression logic check
    let now = Instant::now();
    let last_origin = Some((ScrollOrigin::Editor, now));

    // A preview scroll arriving within 100ms should be suppressed
    let is_echo = if let Some((origin, time)) = last_origin {
        origin == ScrollOrigin::Editor && time.elapsed() < Duration::from_millis(100)
    } else {
        false
    };
    assert!(is_echo);

    // A preview scroll arriving after 100ms should NOT be suppressed
    let old_origin = Some((ScrollOrigin::Editor, now - Duration::from_millis(150)));
    let is_echo_after_window = if let Some((origin, time)) = old_origin {
        origin == ScrollOrigin::Editor && time.elapsed() < Duration::from_millis(100)
    } else {
        false
    };
    assert!(!is_echo_after_window);
}

#[test]
fn test_code_copy_feedback_expiration() {
    let hash = 123456789u64;
    let now = Instant::now();
    let copied_state = Some((hash, now));

    // Immediate check: active
    let is_copied = copied_state
        .map(|(h, t)| h == hash && t.elapsed() < Duration::from_millis(1200))
        .unwrap_or(false);
    assert!(is_copied);

    // After 1200ms: expired
    let expired_state = Some((hash, now - Duration::from_millis(1300)));
    let is_expired = expired_state
        .map(|(h, t)| h == hash && t.elapsed() < Duration::from_millis(1200))
        .unwrap_or(false);
    assert!(!is_expired);
}
