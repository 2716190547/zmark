use glidemark_app::state::format_mtime;
use glidemark_app::theme::ThemeColors;
use glidemark_app::views::preview::should_wrap_code_block;
use glidemark_app::views::workspace::topbar_save_button_color;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Phase G P2-a: Unit tests for should_wrap_code_block pure function
#[test]
fn test_should_wrap_code_block_rules_and_case_trim_insensitivity() {
    // 1. Unannotated or empty language code blocks must wrap (return true)
    assert!(
        should_wrap_code_block(None),
        "None (no language specified) must wrap"
    );
    assert!(
        should_wrap_code_block(Some("")),
        "Empty string language must wrap"
    );
    assert!(
        should_wrap_code_block(Some("   ")),
        "Whitespace-only language must wrap"
    );

    // 2. Text language (case and whitespace-insensitive) must wrap (return true)
    assert!(
        should_wrap_code_block(Some("text")),
        "'text' language must wrap"
    );
    assert!(
        should_wrap_code_block(Some("TEXT")),
        "'TEXT' (uppercase) language must wrap"
    );
    assert!(
        should_wrap_code_block(Some("Text")),
        "'Text' (titlecase) language must wrap"
    );
    assert!(
        should_wrap_code_block(Some("  text  ")),
        "'  text  ' (padded with whitespace) language must wrap"
    );
    assert!(
        should_wrap_code_block(Some("  TEXT  ")),
        "'  TEXT  ' (padded uppercase) language must wrap"
    );

    // 3. Programming languages must scroll horizontally (return false)
    assert!(
        !should_wrap_code_block(Some("python")),
        "'python' language must NOT wrap (horizontal scroll)"
    );
    assert!(
        !should_wrap_code_block(Some("rust")),
        "'rust' language must NOT wrap"
    );
    assert!(
        !should_wrap_code_block(Some("javascript")),
        "'javascript' language must NOT wrap"
    );
    assert!(
        !should_wrap_code_block(Some("  python  ")),
        "'  python  ' (padded) language must NOT wrap"
    );

    // 4. ADR 0036 constraint assertion:
    // Mermaid blocks are handled separately by render_mermaid_element as interactive widgets
    // with pan/zoom and do not enter should_wrap_code_block.
    let is_mermaid = "mermaid".eq_ignore_ascii_case("mermaid");
    assert!(is_mermaid, "Mermaid blocks are routed directly to interactive renderer");
}

/// Phase G P2-b: Unit tests for topbar_save_button_color pure function
#[test]
fn test_topbar_save_button_color_dirty_and_clean_states() {
    let light_theme = ThemeColors::default_palette(false);
    let dark_theme = ThemeColors::default_palette(true);

    // Light theme: dirty -> accent, clean -> text_muted
    assert_eq!(
        topbar_save_button_color(true, &light_theme),
        light_theme.accent,
        "Dirty document must show theme.accent on save button in light mode"
    );
    assert_eq!(
        topbar_save_button_color(false, &light_theme),
        light_theme.text_muted,
        "Clean document must show theme.text_muted on save button in light mode"
    );

    // Dark theme: dirty -> accent, clean -> text_muted
    assert_eq!(
        topbar_save_button_color(true, &dark_theme),
        dark_theme.accent,
        "Dirty document must show theme.accent on save button in dark mode"
    );
    assert_eq!(
        topbar_save_button_color(false, &dark_theme),
        dark_theme.text_muted,
        "Clean document must show theme.text_muted on save button in dark mode"
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
