use gpui::{actions, Action, App, KeyBinding, KeyBindingContextPredicate, Keystroke, OperatingSystem};
use gpui_component::kbd::Kbd;
use zmark_core::config::{Preferences, ShortcutActionId};

actions!(
    zmark,
    [
        ToggleSourceMode,
        ToggleSplitMode,
        TogglePreviewMode,
        ToggleDrawer,
        ToggleSidebarTab,
        ToggleSettings,
        OpenFileDialog,
        NewDraft,
        SaveFile,
        CloseActiveDoc,
        ExportHtml,
        CancelRename,
        FindInFile,
        CloseFind,
        FindNext,
        FindPrevious,
        UndoSidebarRemoval,
        SourceUndo,
        SourceRedo,
        SourceCopy,
        SourceCut,
        SourcePaste,
        SourceSelectAll,
        SourceDelete,
        SourceBold,
        SourceNumberedList,
        SourceIndent,
        SourceOutdent
    ]
);

/// Initializes built-in default ZMark keybindings (preserving component bindings).
pub fn init_key_bindings(cx: &mut App) {
    sync_key_bindings(&Preferences::default(), cx);
}

/// Ensures ZMark keybindings are installed once (safe in both app startup and tests).
pub fn ensure_key_bindings(preferences: &Preferences, cx: &mut App) {
    let already_loaded = cx
        .key_bindings()
        .borrow()
        .bindings()
        .any(|b| is_managed_zmark_action(b.action()));
    if !already_loaded {
        sync_key_bindings(preferences, cx);
    }
}

/// Rebuilds ZMark's customizable keybindings cleanly without dropping `gpui_component`
/// or fixed modal/editor keybindings.
pub fn sync_key_bindings(preferences: &Preferences, cx: &mut App) {
    let preserved: Vec<KeyBinding> = cx
        .key_bindings()
        .borrow()
        .bindings()
        .filter(|b| !is_managed_zmark_action(b.action()))
        .cloned()
        .collect();

    cx.clear_key_bindings();
    if !preserved.is_empty() {
        cx.bind_keys(preserved);
    }

    // Fixed modal / find-input bindings (preserved explicitly even on fresh Apps).
    cx.bind_keys([
        KeyBinding::new("escape", CloseFind, Some("FileFind")),
        KeyBinding::new("up", FindPrevious, Some("FileFind > Input")),
        KeyBinding::new("down", FindNext, Some("FileFind > Input")),
        KeyBinding::new("enter", FindNext, Some("FileFind > Input")),
        KeyBinding::new("shift-enter", FindPrevious, Some("FileFind > Input")),
    ]);

    let workspace_ctx = parse_ctx("Workspace && !InlineRename");
    let workspace_no_input_ctx = parse_ctx("Workspace && !Input && !InlineRename");

    let os = cx.platform_info().os;
    let mut custom_specs: Vec<(ShortcutActionId, String)> = Vec::new();
    let mut primary_taken: Vec<Keystroke> = Vec::new();

    for action in ShortcutActionId::ALL {
        let spec = preferences.shortcut_spec(action);
        if let Some(ks) = parse_shortcut_spec(spec, os) {
            primary_taken.push(ks);
            custom_specs.push((action, spec.to_string()));
        }
    }

    let mut bindings = Vec::with_capacity(custom_specs.len() + 2);
    for (action, spec) in custom_specs {
        let ctx = match action {
            ShortcutActionId::UndoSidebarRemoval
            | ShortcutActionId::SourceBold
            | ShortcutActionId::SourceNumberedList => workspace_no_input_ctx.clone(),
            ShortcutActionId::FindInFile
            | ShortcutActionId::FindNext
            | ShortcutActionId::FindPrevious => workspace_ctx.clone(),
            _ => None,
        };
        if let Ok(binding) = KeyBinding::load(&spec, boxed_action(action), ctx, false, None, cx) {
            bindings.push(binding);
        }
    }

    // Keep F3 / Shift+F3 fallback aliases active unless claimed by a custom shortcut.
    for action in [ShortcutActionId::FindNext, ShortcutActionId::FindPrevious] {
        if let Some(fallback) = action.secondary_fallback_spec() {
            if let Some(fallback_ks) = parse_shortcut_spec(fallback, os) {
                let is_taken = primary_taken
                    .iter()
                    .any(|taken| same_keystroke(taken, &fallback_ks));
                if !is_taken {
                    if let Ok(binding) = KeyBinding::load(
                        fallback,
                        boxed_action(action),
                        workspace_ctx.clone(),
                        false,
                        None,
                        cx,
                    ) {
                        bindings.push(binding);
                    }
                }
            }
        }
    }

    cx.bind_keys(bindings);
}

fn parse_ctx(expr: &'static str) -> Option<std::rc::Rc<KeyBindingContextPredicate>> {
    KeyBindingContextPredicate::parse(expr).ok().map(Into::into)
}

fn boxed_action(action: ShortcutActionId) -> Box<dyn Action> {
    match action {
        ShortcutActionId::OpenFileDialog => Box::new(OpenFileDialog),
        ShortcutActionId::NewDraft => Box::new(NewDraft),
        ShortcutActionId::SaveFile => Box::new(SaveFile),
        ShortcutActionId::CloseActiveDoc => Box::new(CloseActiveDoc),
        ShortcutActionId::ExportHtml => Box::new(ExportHtml),
        ShortcutActionId::ToggleDrawer => Box::new(ToggleDrawer),
        ShortcutActionId::ToggleSidebarTab => Box::new(ToggleSidebarTab),
        ShortcutActionId::UndoSidebarRemoval => Box::new(UndoSidebarRemoval),
        ShortcutActionId::ToggleSourceMode => Box::new(ToggleSourceMode),
        ShortcutActionId::ToggleSplitMode => Box::new(ToggleSplitMode),
        ShortcutActionId::TogglePreviewMode => Box::new(TogglePreviewMode),
        ShortcutActionId::ToggleSettings => Box::new(ToggleSettings),
        ShortcutActionId::FindInFile => Box::new(FindInFile),
        ShortcutActionId::FindNext => Box::new(FindNext),
        ShortcutActionId::FindPrevious => Box::new(FindPrevious),
        ShortcutActionId::SourceBold => Box::new(SourceBold),
        ShortcutActionId::SourceNumberedList => Box::new(SourceNumberedList),
    }
}

fn is_managed_zmark_action(action: &dyn Action) -> bool {
    let name = action.name();
    matches!(
        name,
        "zmark::ToggleSourceMode"
            | "zmark::ToggleSplitMode"
            | "zmark::TogglePreviewMode"
            | "zmark::ToggleDrawer"
            | "zmark::ToggleSidebarTab"
            | "zmark::ToggleSettings"
            | "zmark::OpenFileDialog"
            | "zmark::NewDraft"
            | "zmark::SaveFile"
            | "zmark::CloseActiveDoc"
            | "zmark::ExportHtml"
            | "zmark::FindInFile"
            | "zmark::CloseFind"
            | "zmark::FindNext"
            | "zmark::FindPrevious"
            | "zmark::UndoSidebarRemoval"
            | "zmark::SourceBold"
            | "zmark::SourceNumberedList"
    )
}

/// Parses a configured shortcut specification (`""` -> `None`).
pub fn parse_shortcut_spec(spec: &str, os: OperatingSystem) -> Option<Keystroke> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return None;
    }
    Keystroke::parse_with_os(trimmed, os).ok()
}

/// Formats an active shortcut specification via `gpui_component::kbd::Kbd::format`.
pub fn format_shortcut_spec(spec: &str, os: OperatingSystem) -> Option<String> {
    parse_shortcut_spec(spec, os).map(|ks| Kbd::format(&ks))
}

/// Returns the formatted active shortcut hint for `action`, or `None` if unbound.
pub fn shortcut_display_for_action(
    preferences: &Preferences,
    action: ShortcutActionId,
    os: OperatingSystem,
) -> Option<String> {
    format_shortcut_spec(preferences.shortcut_spec(action), os)
}

/// Returns `label (shortcut)` when bound, or `label` when unbound.
pub fn label_with_shortcut(
    label: &str,
    preferences: &Preferences,
    action: ShortcutActionId,
    os: OperatingSystem,
) -> String {
    match shortcut_display_for_action(preferences, action, os) {
        Some(keys) => format!("{label} ({keys})"),
        None => label.to_string(),
    }
}

/// Returns whether two GPUI keystrokes represent the same physical shortcut.
pub fn same_keystroke(a: &Keystroke, b: &Keystroke) -> bool {
    a.modifiers.platform == b.modifiers.platform
        && a.modifiers.control == b.modifiers.control
        && a.modifiers.alt == b.modifiers.alt
        && a.modifiers.shift == b.modifiers.shift
        && a.key.eq_ignore_ascii_case(&b.key)
}

/// Normalizes a recorded `Keystroke` into a persistable GPUI spec string if valid as
/// a global shortcut (requires `Cmd`/`Ctrl`/`Alt` or a function key `F1`..`F35`),
/// returning the matching default spec string when equivalent so defaults stay sparse.
pub fn recordable_shortcut_spec(
    keystroke: &Keystroke,
    default_spec: &str,
    os: OperatingSystem,
) -> Option<String> {
    if keystroke.is_modifier_key() || keystroke.key.is_empty() {
        return None;
    }
    let has_primary_mod =
        keystroke.modifiers.platform || keystroke.modifiers.control || keystroke.modifiers.alt;
    if !has_primary_mod && !is_function_key(&keystroke.key) {
        return None;
    }
    let mut normalized = keystroke.clone();
    normalized.key = normalized.key.to_ascii_lowercase();
    normalized.key_char = None;
    normalized.modifiers.function = false;

    if let Some(default_ks) = parse_shortcut_spec(default_spec, os) {
        if same_keystroke(&normalized, &default_ks) {
            return Some(default_spec.to_string());
        }
    }
    Some(normalized.unparse(os))
}

/// Finds an existing customizable action using the same shortcut keystroke.
pub fn find_shortcut_conflict(
    preferences: &Preferences,
    target: ShortcutActionId,
    candidate: &Keystroke,
    os: OperatingSystem,
) -> Option<ShortcutActionId> {
    ShortcutActionId::ALL.into_iter().find(|&other| {
        other != target
            && parse_shortcut_spec(preferences.shortcut_spec(other), os)
                .is_some_and(|ks| same_keystroke(&ks, candidate))
    })
}

fn is_function_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower
        .strip_prefix('f')
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (1..=35).contains(&n))
}
