//! Single source of truth for the few values that differ between desktop platforms.
//!
//! Groundwork for the Windows/Linux ports: UI code asks this module instead of
//! hardcoding macOS font names or window-chrome geometry. Nothing here changes
//! macOS behaviour.

/// UI (proportional) font family list understood by the platform text system.
pub const UI_FONT: &str = if cfg!(target_os = "macos") {
    "ui-sans-serif, system-ui"
} else if cfg!(target_os = "windows") {
    "Segoe UI"
} else {
    "Noto Sans, Cantarell, DejaVu Sans"
};

/// Monospace font family list understood by the platform text system.
pub const MONO_FONT: &str = if cfg!(target_os = "macos") {
    "ui-monospace"
} else if cfg!(target_os = "windows") {
    "Cascadia Mono, Consolas"
} else {
    "JetBrains Mono, DejaVu Sans Mono, monospace"
};

/// Leading inset that keeps header content clear of the native window controls.
pub const LEADING_WINDOW_CONTROL_INSET: f32 = 76.0;

/// Reserved space for native window controls: macOS draws traffic lights itself,
/// Windows/Linux need room for the OS-drawn minimise/maximise/close buttons.
pub const TRAILING_WINDOW_CONTROL_INSET: f32 = if cfg!(target_os = "macos") {
    12.0
} else {
    138.0
};

/// Whether the app gaining activation requires ordering the main window front.
///
/// Only the inactive → active transition does work; repeated active callbacks
/// (for example while the window is already frontmost) must stay no-ops.
pub fn should_front_window(was_active: bool, is_active: bool) -> bool {
    is_active && !was_active
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_transition_fronts_only_when_entering_active() {
        assert!(should_front_window(false, true));
        assert!(!should_front_window(true, true));
        assert!(!should_front_window(true, false));
        assert!(!should_front_window(false, false));
    }

    #[test]
    fn font_stacks_and_chrome_follow_platform() {
        assert!(!UI_FONT.is_empty() && !MONO_FONT.is_empty());
        #[cfg(target_os = "macos")]
        {
            assert_eq!(LEADING_WINDOW_CONTROL_INSET, 76.0);
            assert_eq!(TRAILING_WINDOW_CONTROL_INSET, 12.0);
            assert!(UI_FONT.contains("system-ui"));
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(TRAILING_WINDOW_CONTROL_INSET, 138.0);
            assert!(!UI_FONT.contains("system-ui"));
        }
    }
}
