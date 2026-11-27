//! Single source of truth for the few values that differ between desktop platforms.
//!
//! Groundwork for the Windows/Linux ports: UI code asks this module instead of
//! hardcoding macOS font names or window-chrome geometry. Nothing here changes
//! macOS behaviour.

use gpui::{point, px, TitlebarOptions, WindowDecorations};

/// Default UI (proportional) font family list understood by the platform text system.
pub const DEFAULT_UI_FONT: &str = if cfg!(target_os = "macos") {
    "ui-sans-serif, system-ui"
} else if cfg!(target_os = "windows") {
    "Segoe UI"
} else {
    "Noto Sans, Cantarell, DejaVu Sans"
};

/// Default monospace font family list understood by the platform text system.
pub const DEFAULT_MONO_FONT: &str = if cfg!(target_os = "macos") {
    "ui-monospace"
} else if cfg!(target_os = "windows") {
    "Cascadia Mono, Consolas"
} else {
    "JetBrains Mono, DejaVu Sans Mono, monospace"
};

struct FontState {
    ui: &'static str,
    mono: &'static str,
    generation: u32,
}

static FONTS: std::sync::RwLock<FontState> = std::sync::RwLock::new(FontState {
    ui: DEFAULT_UI_FONT,
    mono: DEFAULT_MONO_FONT,
    generation: 0,
});

/// Family names handed out as `&'static str`. Each distinct user choice is
/// leaked once, so switching back and forth does not grow memory.
static INTERNED: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

fn intern(value: String) -> &'static str {
    let mut interned = INTERNED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(existing) = interned.iter().find(|s| **s == value) {
        return existing;
    }
    let leaked: &'static str = Box::leak(value.into_boxed_str());
    interned.push(leaked);
    leaked
}

/// The user's family on its own. GPUI treats a family string as one name and
/// does not split a CSS-style list, so `"Courier New, ui-monospace"` would
/// never resolve. A family uninstalled later falls back to GPUI's built-in
/// fallback stack.
fn family_list(chosen: &str, default: &'static str) -> &'static str {
    let chosen = chosen.trim();
    if chosen.is_empty() || chosen == "system-ui" {
        default
    } else {
        intern(chosen.to_string())
    }
}

/// Body and interface font chosen in Settings (platform default when unset).
pub fn ui_font() -> &'static str {
    FONTS.read().unwrap_or_else(|e| e.into_inner()).ui
}

/// Code font chosen in Settings, used by code blocks and the source editor.
pub fn mono_font() -> &'static str {
    FONTS.read().unwrap_or_else(|e| e.into_inner()).mono
}

/// Applies the preferred families. Returns true when either one changed.
pub fn set_fonts(body: &str, code: &str) -> bool {
    let ui = family_list(body, DEFAULT_UI_FONT);
    let mono = family_list(code, DEFAULT_MONO_FONT);
    let mut fonts = FONTS.write().unwrap_or_else(|e| e.into_inner());
    if fonts.ui == ui && fonts.mono == mono {
        return false;
    }
    fonts.ui = ui;
    fonts.mono = mono;
    fonts.generation = fonts.generation.wrapping_add(1);
    true
}

/// Preview body line height. Stored as f32 bits; 1.75 matches the default
/// `line_height` preference of 1.6 plus [`PREVIEW_LINE_HEIGHT_OFFSET`].
static PREVIEW_LINE_HEIGHT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3FE0_0000);

/// Preview text sits looser than the source editor by this much.
pub const PREVIEW_LINE_HEIGHT_OFFSET: f32 = 0.15;

/// Line height factor for preview paragraphs and code blocks.
pub fn preview_line_height() -> f32 {
    f32::from_bits(PREVIEW_LINE_HEIGHT.load(std::sync::atomic::Ordering::Relaxed))
}

/// Line height factor for list item text, one offset tighter than paragraphs.
pub fn preview_list_line_height() -> f32 {
    preview_line_height() - PREVIEW_LINE_HEIGHT_OFFSET
}

/// Applies the `line_height` preference to the preview. Returns true when it
/// changed; layout caches keyed by [`font_layout_key`] are invalidated then.
pub fn set_preview_line_height(preference: f32) -> bool {
    let value = preference + PREVIEW_LINE_HEIGHT_OFFSET;
    let old = PREVIEW_LINE_HEIGHT.swap(value.to_bits(), std::sync::atomic::Ordering::Relaxed);
    if old == value.to_bits() {
        return false;
    }
    let mut fonts = FONTS.write().unwrap_or_else(|e| e.into_inner());
    fonts.generation = fonts.generation.wrapping_add(1);
    true
}

/// Cache key for text layout that depends on both size and the current fonts.
pub fn font_layout_key(font_size: f32) -> u32 {
    let generation = FONTS.read().unwrap_or_else(|e| e.into_inner()).generation;
    font_size.to_bits() ^ generation.rotate_right(8)
}

/// Leading inset that keeps header content clear of the native window controls.
pub const LEADING_WINDOW_CONTROL_INSET: f32 = 76.0;

/// Trailing inset before app content. macOS leaves a small gap for its traffic
/// lights; Windows draws custom caption buttons as part of the header, so no
/// extra inset is needed; Linux keeps native server-side decorations and only
/// needs normal content padding.
pub const fn trailing_window_control_inset(is_macos: bool, is_windows: bool) -> f32 {
    if is_macos {
        12.0
    } else if is_windows {
        0.0
    } else {
        8.0
    }
}

pub const TRAILING_WINDOW_CONTROL_INSET: f32 =
    trailing_window_control_inset(cfg!(target_os = "macos"), cfg!(target_os = "windows"));

/// GPUI's `secondary` modifier is Command on macOS and Control on Windows/Linux.
/// Use it for application shortcuts instead of the literal `cmd` key.
pub const SHORTCUT_MODIFIER: &str = if cfg!(target_os = "macos") {
    "cmd"
} else {
    "secondary"
};

pub const MODIFIER_SYMBOL: &str = if cfg!(target_os = "macos") {
    "⌘"
} else {
    "Ctrl+"
};

pub fn shortcut(key: &str) -> String {
    format!("{SHORTCUT_MODIFIER}-{key}")
}

pub fn shortcut_hint(key: &str) -> String {
    format!("{MODIFIER_SYMBOL}{key}")
}

/// Width of one Windows caption button. Three buttons fill the standard 138px
/// caption area (46px each at 100% scale).
pub const WINDOW_CONTROL_BUTTON_WIDTH: f32 = 46.0;

/// Title-bar policy that keeps exactly one bar on every desktop.
///
/// - macOS: transparent title bar with native traffic lights at the leading edge.
/// - Windows: transparent title bar (native caption hidden), with minimise/
///   maximise/close drawn into the existing canvas header, so no second bar.
/// - Linux: native server-side decorations; the canvas header is ordinary
///   content below the system title bar.
pub fn titlebar_options(title: &str, traffic_light_y: f32) -> Option<TitlebarOptions> {
    if cfg!(target_os = "macos") {
        Some(TitlebarOptions {
            title: Some(title.to_string().into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.), px(traffic_light_y))),
        })
    } else if cfg!(target_os = "windows") {
        Some(TitlebarOptions {
            title: Some(title.to_string().into()),
            appears_transparent: true,
            traffic_light_position: None,
        })
    } else {
        None
    }
}

/// Linux keeps compositor/server-side decorations; macOS and Windows use the
/// in-app title bar instead.
pub fn window_decorations() -> Option<WindowDecorations> {
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        None
    } else {
        Some(WindowDecorations::Server)
    }
}

/// Auxiliary windows use the same transparent titlebar as the main window.
pub fn auxiliary_titlebar_options(title: &str, traffic_light_y: f32) -> Option<TitlebarOptions> {
    if cfg!(target_os = "macos") {
        Some(TitlebarOptions {
            title: Some(title.to_string().into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.), px(traffic_light_y))),
        })
    } else if cfg!(target_os = "windows") {
        Some(TitlebarOptions {
            title: Some(title.to_string().into()),
            appears_transparent: true,
            traffic_light_position: None,
        })
    } else {
        None
    }
}


/// Sets the packaged icon explicitly, so a Dock launch also uses the application
/// icon even if LaunchServices has an older cached entry.
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
pub fn register_application_icon() {
    use cocoa::appkit::NSApplication;
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSBundle, NSString};
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let app = NSApplication::sharedApplication(nil);
        let bundle: id = NSBundle::mainBundle();
        let name = NSString::alloc(nil).init_str("AppIcon");
        let image: id = msg_send![bundle, imageForResource: name];
        let image = if image.is_null() {
            msg_send![class!(NSImage), imageNamed: name]
        } else {
            image
        };
        if !image.is_null() {
            let _: () = msg_send![app, setApplicationIconImage: image];
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn register_application_icon() {}
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
        assert!(!DEFAULT_UI_FONT.is_empty() && !DEFAULT_MONO_FONT.is_empty());
        #[cfg(target_os = "macos")]
        {
            assert_eq!(LEADING_WINDOW_CONTROL_INSET, 76.0);
            assert_eq!(TRAILING_WINDOW_CONTROL_INSET, 12.0);
            assert!(DEFAULT_UI_FONT.contains("system-ui"));
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert!(!DEFAULT_UI_FONT.contains("system-ui"));
        }
    }

    #[test]
    fn application_shortcuts_use_command_on_macos_and_control_elsewhere() {
        if cfg!(target_os = "macos") {
            assert_eq!(shortcut("s"), "cmd-s");
            assert_eq!(shortcut_hint("S"), "⌘S");
        } else {
            assert_eq!(shortcut("s"), "secondary-s");
            assert_eq!(shortcut_hint("S"), "Ctrl+S");
        }
    }

    #[test]
    fn trailing_inset_keeps_one_bar_per_platform() {
        assert_eq!(trailing_window_control_inset(true, false), 12.0);
        assert_eq!(trailing_window_control_inset(false, true), 0.0);
        assert_eq!(trailing_window_control_inset(false, false), 8.0);
        assert_eq!(WINDOW_CONTROL_BUTTON_WIDTH * 3.0, 138.0);
    }

    #[test]
    fn titlebar_policy_never_adds_a_second_bar() {
        if cfg!(any(target_os = "macos", target_os = "windows")) {
            let titlebar =
                titlebar_options("ZMark", 12.0).expect("custom chrome on macOS/Windows");
            assert!(titlebar.appears_transparent);
            if cfg!(target_os = "macos") {
                assert_eq!(
                    titlebar.traffic_light_position,
                    Some(point(px(14.), px(12.)))
                );
            } else {
                assert_eq!(titlebar.traffic_light_position, None);
            }
        } else {
            assert!(titlebar_options("ZMark", 12.0).is_none());
            assert_eq!(window_decorations(), Some(WindowDecorations::Server));
        }
    }
}

/// Uses the system trash service; never falls back to permanent deletion.
#[cfg(target_os = "macos")]
pub fn move_file_to_trash(path: &std::path::Path) -> std::io::Result<()> {
    use cocoa::base::{id, nil, BOOL, NO};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
    unsafe {
        let pool: id = msg_send![class!(NSAutoreleasePool), new];
        let url: id = msg_send![class!(NSURL), fileURLWithFileSystemRepresentation: path.as_ptr() isDirectory: NO relativeToURL: nil];
        let manager: id = msg_send![class!(NSFileManager), defaultManager];
        let mut error: id = nil;
        let success: BOOL = msg_send![manager, trashItemAtURL: url resultingItemURL: std::ptr::null_mut::<id>() error: &mut error];
        let result = if success != NO {
            Ok(())
        } else if error != nil {
            let description: id = msg_send![error, localizedDescription];
            Err(std::io::Error::other(std::ffi::CStr::from_ptr(description.UTF8String()).to_string_lossy().into_owned()))
        } else {
            Err(std::io::Error::other("Unable to move file to Trash"))
        };
        let _: () = msg_send![pool, drain];
        result
    }
}

#[cfg(not(target_os = "macos"))]
pub fn move_file_to_trash(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference='Stop'; Add-Type -AssemblyName Microsoft.VisualBasic; if (Test-Path -LiteralPath $env:ZMARK_TRASH_PATH -PathType Container) { [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($env:ZMARK_TRASH_PATH, 'OnlyErrorDialogs', 'SendToRecycleBin') } else { [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($env:ZMARK_TRASH_PATH, 'OnlyErrorDialogs', 'SendToRecycleBin') }"])
        .env("ZMARK_TRASH_PATH", path).output()?;
    #[cfg(not(target_os = "windows"))]
    let output = std::process::Command::new("gio").args(["trash", "--"]).arg(path).output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(String::from_utf8_lossy(&output.stderr).trim().to_string()))
    }
}

/// Shows `path` selected in the system file manager.
///
/// On Windows GPUI's `reveal_path` calls shell COM APIs on a background thread that
/// never initialised COM, so it fails silently; launch Explorer directly instead.
pub fn reveal_in_folder(path: &std::path::Path, cx: &gpui::App) {
    #[cfg(target_os = "windows")]
    {
        let _ = cx;
        let _ = spawn_explorer_select(path);
    }
    #[cfg(not(target_os = "windows"))]
    cx.reveal_path(path);
}

#[cfg(target_os = "windows")]
fn spawn_explorer_select(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    let raw = path.to_string_lossy();
    let plain = windows_display_path(&raw);
    // explorer.exe parses its own command line; the path must be quoted after `/select,`.
    std::process::Command::new("explorer.exe")
        .raw_arg(format!("/select,\"{plain}\""))
        .spawn()
        .map(|_| ())
}

/// Strips the `\\?\` verbatim prefix that `canonicalize` adds on Windows; Explorer rejects it.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn windows_display_path(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = raw.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        raw.to_string()
    }
}

#[cfg(test)]
mod reveal_tests {
    use super::windows_display_path;

    #[test]
    fn strips_verbatim_prefixes() {
        assert_eq!(windows_display_path(r"\\?\C:\a b\x.md"), r"C:\a b\x.md");
        assert_eq!(windows_display_path(r"\\?\UNC\srv\share\x.md"), r"\\srv\share\x.md");
        assert_eq!(windows_display_path(r"C:\x.md"), r"C:\x.md");
    }
}

/// Resolve dynamic AppKit colors on the UI thread, using the app's appearance.
#[cfg(target_os = "macos")]
pub fn text_selection_colors(dark: bool, active: bool) -> Option<(gpui::Hsla, gpui::Hsla)> {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let pool: id = msg_send![class!(NSAutoreleasePool), new];
        let previous: id = msg_send![class!(NSAppearance), currentAppearance];
        let name = NSString::alloc(nil).init_str(if dark { "NSAppearanceNameDarkAqua" } else { "NSAppearanceNameAqua" });
        let appearance: id = msg_send![class!(NSAppearance), appearanceNamed: name];
        let _: () = msg_send![class!(NSAppearance), setCurrentAppearance: appearance];
        let background: id = if active { msg_send![class!(NSColor), selectedTextBackgroundColor] }
            else { msg_send![class!(NSColor), unemphasizedSelectedTextBackgroundColor] };
        let foreground: id = if active { msg_send![class!(NSColor), selectedTextColor] }
            else { msg_send![class!(NSColor), unemphasizedSelectedTextColor] };
        let space: id = msg_send![class!(NSColorSpace), sRGBColorSpace];
        let convert = |color: id| -> Option<gpui::Hsla> {
            let color: id = msg_send![color, colorUsingColorSpace: space];
            if color == nil { return None; }
            let (mut r, mut g, mut b, mut a) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
            let _: () = msg_send![color, getRed: &mut r green: &mut g blue: &mut b alpha: &mut a];
            Some(gpui::Rgba { r: r as f32, g: g as f32, b: b as f32, a: a as f32 }.into())
        };
        let result = convert(background).zip(convert(foreground));
        let _: () = msg_send![class!(NSAppearance), setCurrentAppearance: previous];
        let _: () = msg_send![name, release];
        let _: () = msg_send![pool, drain];
        result
    }
}

#[cfg(not(target_os = "macos"))]
pub fn text_selection_colors(_dark: bool, _active: bool) -> Option<(gpui::Hsla, gpui::Hsla)> { None }

/// Folder holding ZMark's licenses and THIRD-PARTY-NOTICES.html in an installed
/// build: `Contents/Resources/licenses` on macOS, `licenses` beside the exe on
/// Windows, `/usr/share/doc/zmark` from the .deb. `None` for dev builds.
pub fn licenses_dir() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = if cfg!(target_os = "macos") {
        exe.parent()?.parent()?.join("Resources/licenses")
    } else if cfg!(target_os = "windows") {
        exe.parent()?.join("licenses")
    } else {
        std::path::PathBuf::from("/usr/share/doc/zmark")
    };
    dir.join("THIRD-PARTY-NOTICES.html").is_file().then_some(dir)
}

/// Whether macOS "natural scrolling" is on, i.e. scroll deltas are already
/// inverted relative to the finger or wheel motion. Absent key means on (the
/// macOS default).
pub fn scroll_direction_inverted() -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
        use cocoa::base::{id, nil};
        use cocoa::foundation::NSString;
        use objc::{class, msg_send, sel, sel_impl};
        let defaults: id = msg_send![class!(NSUserDefaults), standardUserDefaults];
        let key = NSString::alloc(nil).init_str("com.apple.swipescrolldirection");
        let value: id = msg_send![defaults, objectForKey: key];
        let _: () = msg_send![key, release];
        if value == nil {
            return true;
        }
        let on: cocoa::base::BOOL = msg_send![value, boolValue];
        on != cocoa::base::NO
    }
    // Windows wheel-up is positive. Linux keeps the inverted mapping it shipped with.
    #[cfg(not(target_os = "macos"))]
    {
        !cfg!(target_os = "windows")
    }
}

/// Turn the `file://` URLs the OS hands an open-URL handler into local paths.
/// Other schemes are ignored; percent escapes (spaces, CJK names) are decoded.
pub fn paths_from_open_urls(urls: &[String]) -> Vec<std::path::PathBuf> {
    urls.iter()
        .filter_map(|url| url.strip_prefix("file://"))
        .map(|rest| rest.strip_prefix("localhost").unwrap_or(rest))
        .filter_map(|path| percent_encoding::percent_decode_str(path).decode_utf8().ok())
        .map(|path| std::path::PathBuf::from(path.into_owned()))
        .collect()
}

#[cfg(test)]
mod open_url_tests {
    use super::paths_from_open_urls;
    use std::path::PathBuf;

    #[test]
    fn decodes_file_urls_and_skips_other_schemes() {
        let urls = vec![
            "file:///Users/a/%E7%AC%94%E8%AE%B0%20one.md".to_string(),
            "file://localhost/tmp/b.md".to_string(),
            "https://example.com/c.md".to_string(),
        ];
        assert_eq!(
            paths_from_open_urls(&urls),
            vec![PathBuf::from("/Users/a/笔记 one.md"), PathBuf::from("/tmp/b.md")]
        );
    }
}
