use glidemark_app::state::format_mtime;
use glidemark_app::views::preview::should_wrap_code_block;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Phase G P1 & P2: Unit tests for should_wrap_code_block pure function per ADR 0036 §3
#[test]
fn test_should_wrap_code_block_rules_and_case_trim_insensitivity() {
    // 1. None, empty string, or whitespace-only code blocks must retain horizontal scroll (return false)
    assert!(
        !should_wrap_code_block(None),
        "None (no language specified) must NOT wrap (preserve horizontal scroll for ASCII trees)"
    );
    assert!(
        !should_wrap_code_block(Some("")),
        "Empty string language must NOT wrap"
    );
    assert!(
        !should_wrap_code_block(Some("   ")),
        "Whitespace-only language must NOT wrap"
    );

    // 2. Text language (case and whitespace-insensitive) must retain horizontal scroll (return false)
    assert!(
        !should_wrap_code_block(Some("text")),
        "'text' language must NOT wrap (preserve horizontal scroll for ASCII trees)"
    );
    assert!(
        !should_wrap_code_block(Some("TEXT")),
        "'TEXT' (uppercase) language must NOT wrap"
    );
    assert!(
        !should_wrap_code_block(Some("Text")),
        "'Text' (titlecase) language must NOT wrap"
    );
    assert!(
        !should_wrap_code_block(Some("  text  ")),
        "'  text  ' (padded with whitespace) language must NOT wrap"
    );
    assert!(
        !should_wrap_code_block(Some("  TEXT  ")),
        "'  TEXT  ' (padded uppercase) language must NOT wrap"
    );

    // 3. Programming languages must soft-wrap (return true)
    assert!(
        should_wrap_code_block(Some("python")),
        "'python' language must wrap (whitespace_normal)"
    );
    assert!(
        should_wrap_code_block(Some("rust")),
        "'rust' language must wrap"
    );
    assert!(
        should_wrap_code_block(Some("javascript")),
        "'javascript' language must wrap"
    );
    assert!(
        should_wrap_code_block(Some("  python  ")),
        "'  python  ' (padded) language must wrap"
    );
    assert!(
        should_wrap_code_block(Some("Python")),
        "'Python' (capitalized) language must wrap"
    );
}


/// Phase G P2-c: Unit tests for format_mtime pure function
#[test]
fn test_format_mtime_none_epoch_pre_epoch_and_future_fallbacks() {
    let now = SystemTime::now();

    // 1. None mtime -> returns None
    assert_eq!(
        format_mtime(None, now),
        None,
        "format_mtime(None, now) must return None"
    );

    // 2. Pre-epoch fallback (e.g. 100 seconds before UNIX_EPOCH) -> returns None
    let pre_epoch = UNIX_EPOCH
        .checked_sub(Duration::from_secs(100))
        .unwrap_or(UNIX_EPOCH);
    // On systems where checked_sub succeeds, it must return None
    if pre_epoch < UNIX_EPOCH {
        assert_eq!(
            format_mtime(Some(pre_epoch), now),
            None,
            "Pre-epoch timestamp must return None fallback"
        );
    }

    // 3. Unix epoch -> valid format (1970/01/01 ...)
    let epoch_str = format_mtime(Some(UNIX_EPOCH), now).expect("epoch must format");
    assert_eq!(epoch_str.len(), 16);
    assert!(epoch_str.starts_with("1970/01/01"));

    // 4. Normal current time -> valid format YYYY/MM/DD HH:mm
    let now_str = format_mtime(Some(now), now).expect("now must format");
    assert_eq!(now_str.len(), 16);
    assert_eq!(&now_str[4..5], "/");
    assert_eq!(&now_str[7..8], "/");
    assert_eq!(&now_str[10..11], " ");
    assert_eq!(&now_str[13..14], ":");

    // 5. Future time (e.g. 1 hour in future) -> clamped to `now`
    let future = now + Duration::from_secs(3600);
    let future_str = format_mtime(Some(future), now).expect("future time must format gracefully");
    assert_eq!(
        future_str, now_str,
        "Future timestamp must clamp to now to avoid confusing clock skew displays"
    );
}
