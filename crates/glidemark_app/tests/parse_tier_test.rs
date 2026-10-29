use glidemark_app::state::{parse_debounce, PARSE_TIER_FAST_BYTES, PARSE_TIER_SLOW_BYTES};
use std::time::Duration;

#[test]
fn test_parse_debounce_tiers() {
    assert_eq!(parse_debounce(0), Some(Duration::from_millis(100)));
    assert_eq!(
        parse_debounce(PARSE_TIER_FAST_BYTES - 1),
        Some(Duration::from_millis(100))
    );
    assert_eq!(
        parse_debounce(PARSE_TIER_FAST_BYTES),
        Some(Duration::from_millis(300))
    );
    assert_eq!(
        parse_debounce(PARSE_TIER_SLOW_BYTES),
        Some(Duration::from_millis(300))
    );
    assert_eq!(parse_debounce(PARSE_TIER_SLOW_BYTES + 1), None);
}
