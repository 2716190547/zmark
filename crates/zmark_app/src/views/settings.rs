use gpui::prelude::*;
use gpui::*;
use gpui_component::Sizable as _;
use gpui_component::color_picker::{ColorPickerEvent, ColorPickerState, ColorSelect};
use std::collections::HashMap;
use std::sync::Arc;
use zmark_core::config::{
    CustomColorSlot, CustomThemePreferences, Preferences, ShortcutActionId, ShortcutGroup,
    ThemeMode, CUSTOM_PALETTE_ID,
};
use zmark_core::i18n::{
    localize_shortcut_action, localize_shortcut_group, I18nStrings, SupportedLocale,
};

use crate::state::SettingsTab;
use crate::theme::{ThemeColors, ALL_PALETTES};
use crate::updater::UpdateStatus;
use crate::views::workspace::WorkspaceView;

/// Settings design tokens. Every settings control takes its size, radius and
/// spacing from here; see `docs/design/settings-ui.md`.
mod tokens {
    /// Height of buttons, segmented controls and key caps.
    pub const CONTROL_H: f32 = 28.0;
    /// Horizontal padding inside buttons and key caps.
    pub const CONTROL_PX: f32 = 12.0;
    /// Minimum key-cap width, so caps line up down the right edge.
    pub const KEYCAP_MIN_W: f32 = 88.0;
    /// Radius of controls (button, segment track, key cap, list item).
    pub const RADIUS: f32 = 6.0;
    /// Radius of a segment thumb, so it nests inside the track's 2px inset.
    pub const RADIUS_INNER: f32 = 4.0;
    /// Radius of cards (setting groups, palette swatches, custom editor).
    pub const RADIUS_CARD: f32 = 8.0;
    /// Inset between a segment track and its thumb.
    pub const TRACK_INSET: f32 = 2.0;
    /// Inset inside a card, used both horizontally (card edge to text and to
    /// the trailing control) and vertically (row edge to a 28px control), so a
    /// control sits the same distance from the card's top and right edges.
    pub const CARD_PX: f32 = 12.0;
    /// Vertical padding of one setting row; equal to `CARD_PX`.
    pub const ROW_PY: f32 = CARD_PX;
    /// Page title size.
    pub const TEXT_TITLE: f32 = 20.0;
    /// Gap between a section title and its card.
    pub const TITLE_GAP: f32 = 6.0;
    /// Gap between sections on a page.
    pub const SECTION_GAP: f32 = 20.0;
    /// Text sizes: row label, body/description, small badge.
    pub const TEXT_LABEL: f32 = 13.0;
    pub const TEXT_BODY: f32 = 12.0;
    pub const TEXT_SMALL: f32 = 11.0;
}
use tokens as tk;

/// Settings color roles derived from the active theme. Controls use these
/// names instead of ad-hoc opacities.
trait SettingsColors {
    /// Card fill: one step off the content background.
    fn card_bg(&self) -> Hsla;
    /// Hairline used for card borders and row dividers.
    fn hairline(&self) -> Hsla;
    /// Fill of a neutral button or key cap at rest.
    fn control_bg(&self) -> Hsla;
    /// Fill of a neutral button or key cap under the pointer.
    fn control_hover_bg(&self) -> Hsla;
}

impl SettingsColors for ThemeColors {
    fn card_bg(&self) -> Hsla {
        self.text_primary.opacity(0.035)
    }
    fn hairline(&self) -> Hsla {
        self.text_primary.opacity(0.08)
    }
    fn control_bg(&self) -> Hsla {
        self.text_primary.opacity(0.06)
    }
    fn control_hover_bg(&self) -> Hsla {
        self.text_primary.opacity(0.10)
    }
}

pub struct SettingsWindowView {
    pub workspace: WeakEntity<WorkspaceView>,
    pub current_tab: SettingsTab,
    pub preferences: Preferences,
    pub theme: ThemeColors,
    pub strings: &'static I18nStrings,
    pub update_status: UpdateStatus,
    pub update_enabled: bool,
    pub update_reason: Option<String>,
    pub editing_custom_dark: Option<bool>,
    pub resolved_effective_dark: bool,
    pub custom_syntax_expanded: bool,
    pub custom_preset_open: bool,
    custom_pickers: HashMap<CustomColorSlot, Entity<ColorPickerState>>,
    custom_picker_subs: Vec<Subscription>,
    pub recording_shortcut: Option<ShortcutActionId>,
    pub shortcut_conflict: Option<(ShortcutActionId, String, ShortcutActionId)>,
    pub shortcut_notice: Option<(ShortcutActionId, &'static str)>,
    shortcut_interceptor: Option<Subscription>,
}

impl SettingsWindowView {
    pub fn new(
        workspace: WeakEntity<WorkspaceView>,
        current_tab: SettingsTab,
        preferences: Preferences,
        theme: ThemeColors,
        strings: &'static I18nStrings,
        cx: &mut Context<Self>,
    ) -> Self {
        if let Some(ws) = workspace.upgrade() {
            cx.observe(&ws, |this, ws, cx| {
                let ws_ref = ws.read(cx);
                this.preferences = ws_ref.state.preferences.clone();
                this.theme = ws_ref.state.theme.clone();
                this.strings = ws_ref.state.strings;
                this.update_status = ws_ref.update_status.clone();
                this.update_enabled = ws_ref.update_config.enabled;
                this.update_reason = ws_ref.update_config.disabled_reason.clone();
                cx.notify();
            })
            .detach();
        }
        let (update_status, update_enabled, update_reason) = workspace
            .upgrade()
            .map(|ws| {
                let ws = ws.read(cx);
                (
                    ws.update_status.clone(),
                    ws.update_config.enabled,
                    ws.update_config.disabled_reason.clone(),
                )
            })
            .unwrap_or((UpdateStatus::Idle, false, None));
        Self {
            workspace,
            current_tab,
            preferences,
            theme,
            strings,
            update_status,
            update_enabled,
            update_reason,
            editing_custom_dark: None,
            resolved_effective_dark: false,
            custom_syntax_expanded: false,
            custom_preset_open: false,
            custom_pickers: HashMap::new(),
            custom_picker_subs: Vec::new(),
            recording_shortcut: None,
            shortcut_conflict: None,
            shortcut_notice: None,
            shortcut_interceptor: None,
        }
    }

    /// Applies or clears a customized shortcut, updates GPUI's keymap live, and saves config.
    pub fn apply_shortcut(
        &mut self,
        action: ShortcutActionId,
        spec: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.preferences.set_shortcut(action, spec.clone());
        crate::actions::sync_key_bindings(&self.preferences, cx);
        if let Some(ws) = self.workspace.upgrade() {
            ws.update(cx, |ws, cx| {
                ws.state.set_shortcut(action, spec);
                cx.notify();
            });
        }
        cx.notify();
    }

    pub fn replace_shortcut_conflict(
        &mut self,
        action: ShortcutActionId,
        spec: String,
        conflict: ShortcutActionId,
        cx: &mut Context<Self>,
    ) {
        self.preferences.set_shortcut(conflict, None);
        self.preferences.set_shortcut(action, Some(spec.clone()));
        self.shortcut_conflict = None;
        self.shortcut_notice = None;
        crate::actions::sync_key_bindings(&self.preferences, cx);
        if let Some(ws) = self.workspace.upgrade() {
            ws.update(cx, |ws, cx| {
                ws.state.set_shortcut(conflict, None);
                ws.state.set_shortcut(action, Some(spec));
                cx.notify();
            });
        }
        cx.notify();
    }

    pub fn reset_shortcut(&mut self, action: ShortcutActionId, cx: &mut Context<Self>) {
        self.stop_recording_shortcut();
        self.shortcut_conflict = None;
        self.shortcut_notice = None;
        self.preferences.reset_shortcut(action);
        crate::actions::sync_key_bindings(&self.preferences, cx);
        if let Some(ws) = self.workspace.upgrade() {
            ws.update(cx, |ws, cx| {
                ws.state.reset_shortcut(action);
                cx.notify();
            });
        }
        cx.notify();
    }

    pub fn reset_all_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.stop_recording_shortcut();
        self.shortcut_conflict = None;
        self.shortcut_notice = None;
        self.preferences.reset_all_shortcuts();
        crate::actions::sync_key_bindings(&self.preferences, cx);
        if let Some(ws) = self.workspace.upgrade() {
            ws.update(cx, |ws, cx| {
                ws.state.reset_all_shortcuts();
                cx.notify();
            });
        }
        cx.notify();
    }

    pub fn stop_recording_shortcut(&mut self) {
        self.recording_shortcut = None;
        self.shortcut_interceptor = None;
    }

    /// Captures the next non-modifier keystroke in Settings to record a shortcut.
    pub fn handle_recorded_keystroke(
        &mut self,
        keystroke: &Keystroke,
        os: OperatingSystem,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(action) = self.recording_shortcut else {
            return false;
        };
        if keystroke.is_modifier_key() {
            return false;
        }
        let no_primary = !keystroke.modifiers.platform
            && !keystroke.modifiers.control
            && !keystroke.modifiers.alt;
        if no_primary && keystroke.key.eq_ignore_ascii_case("escape") {
            self.stop_recording_shortcut();
            self.shortcut_notice = None;
            cx.notify();
            return true;
        }
        if no_primary
            && (keystroke.key.eq_ignore_ascii_case("backspace")
                || keystroke.key.eq_ignore_ascii_case("delete"))
        {
            self.stop_recording_shortcut();
            self.shortcut_conflict = None;
            self.shortcut_notice = None;
            self.apply_shortcut(action, None, cx);
            return true;
        }
        let Some(spec) =
            crate::actions::recordable_shortcut_spec(keystroke, action.default_spec(), os)
        else {
            self.shortcut_notice = Some((action, self.strings.shortcut_invalid_hint));
            cx.notify();
            return true;
        };
        self.shortcut_notice = None;
        if let Some(parsed) = crate::actions::parse_shortcut_spec(&spec, os) {
            if let Some(conflict) =
                crate::actions::find_shortcut_conflict(&self.preferences, action, &parsed, os)
            {
                self.stop_recording_shortcut();
                self.shortcut_conflict = Some((action, spec, conflict));
                cx.notify();
                return true;
            }
        }
        self.stop_recording_shortcut();
        self.shortcut_conflict = None;
        self.apply_shortcut(action, Some(spec), cx);
        true
    }

    pub fn begin_recording_shortcut(
        &mut self,
        action: ShortcutActionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.recording_shortcut = Some(action);
        self.shortcut_conflict = None;
        self.shortcut_notice = None;
        let view = cx.entity().downgrade();
        let target_window = window.window_handle();
        let sub = cx.intercept_keystrokes(move |event, event_window, app| {
            if event_window.window_handle() != target_window || event.keystroke.is_modifier_key() {
                return;
            }
            let os = app.platform_info().os;
            let ks = event.keystroke.clone();
            let handled = view
                .update(app, |this, cx| this.handle_recorded_keystroke(&ks, os, cx))
                .unwrap_or(false);
            if handled {
                app.stop_propagation();
            }
        });
        self.shortcut_interceptor = Some(sub);
        cx.notify();
    }

    fn ensure_custom_pickers(
        &mut self,
        editing_dark: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let preview_theme = ThemeColors::from_custom_palette(
            self.preferences.custom_theme.variant(editing_dark),
            editing_dark,
        );
        if self.custom_pickers.is_empty() {
            for &slot in &CustomColorSlot::ALL {
                let initial = preview_theme.slot(slot);
                let picker =
                    cx.new(|cx| ColorPickerState::new(window, cx).default_value(initial));
                let sub = cx.subscribe(&picker, move |this, _, event: &ColorPickerEvent, cx| {
                    let ColorPickerEvent::Change(Some(color)) = event else {
                        return;
                    };
                    let hex = ThemeColors::hsla_to_hex(*color);
                    let target_dark = this
                        .editing_custom_dark
                        .unwrap_or(this.resolved_effective_dark);
                    if this
                        .preferences
                        .custom_theme
                        .variant(target_dark)
                        .get(slot)
                        != Some(hex.as_str())
                    {
                        this.preferences
                            .custom_theme
                            .variant_mut(target_dark)
                            .set(slot, Some(hex.clone()));
                        if let Some(ws) = this.workspace.upgrade() {
                            ws.update(cx, |ws, cx| {
                                ws.state.set_custom_theme_color(target_dark, slot, &hex);
                                cx.notify();
                            });
                        }
                        cx.notify();
                    }
                });
                self.custom_pickers.insert(slot, picker);
                self.custom_picker_subs.push(sub);
            }
            return;
        }

        for &slot in &CustomColorSlot::ALL {
            let Some(picker) = self.custom_pickers.get(&slot).cloned() else {
                continue;
            };
            let expected = preview_theme.slot(slot);
            let expected_hex = ThemeColors::hsla_to_hex(expected);
            let needs_sync = {
                let state = picker.read(cx);
                !state.is_open()
                    && state
                        .value()
                        .map(ThemeColors::hsla_to_hex)
                        .as_deref()
                        != Some(expected_hex.as_str())
            };
            if needs_sync {
                picker.update(cx, |state, cx| state.set_value(expected, window, cx));
            }
        }
    }
}

impl Render for SettingsWindowView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_sys_dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let effective_dark = match self.preferences.theme {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::Auto => is_sys_dark,
        };
        self.resolved_effective_dark = effective_dark;
        let editing_dark = self.editing_custom_dark.unwrap_or(effective_dark);
        let display_dark = if self.preferences.palette == CUSTOM_PALETTE_ID {
            editing_dark
        } else {
            effective_dark
        };
        self.theme = ThemeColors::resolve_preferences(&self.preferences, display_dark);
        self.theme.refresh_native_selection(window.is_window_active());
        crate::theme::sync_component_theme(&self.theme, cx);
        if self.current_tab == SettingsTab::Appearance
            && self.preferences.palette == CUSTOM_PALETTE_ID
        {
            self.ensure_custom_pickers(editing_dark, window, cx);
        }
        let current_tab = self.current_tab;
        let current_mode = self.preferences.theme;
        let current_palette = self.preferences.palette.clone();
        let custom_theme = self.preferences.custom_theme.clone();
        let custom_syntax_expanded = self.custom_syntax_expanded;
        let custom_preset_open = self.custom_preset_open;
        let custom_pickers = self.custom_pickers.clone();
        let current_locale = self.preferences.locale;
        let auto_save = self.preferences.auto_save;
        let auto_save_on_blur = self.preferences.auto_save_on_blur;
        let tab_size = self.preferences.tab_size;
        let line_height = self.preferences.line_height;
        let column_width = self.preferences.max_column_width;
        let theme = self.theme.clone();
        let strings = self.strings;
        let update_status = self.update_status.clone();
        let update_enabled = self.update_enabled;
        let update_reason = self.update_reason.clone();
        let workspace_for_update = self.workspace.clone();
        // Once an update is downloaded, the same button restarts into it, like the
        // toolbar's update icon.
        let on_check_update = move |_: &MouseDownEvent, window: &mut Window, cx: &mut App| {
            if let Some(ws) = workspace_for_update.upgrade() {
                ws.update(cx, |ws, cx| {
                    if matches!(ws.update_status, UpdateStatus::Downloaded(_)) {
                        ws.restart_for_update(Some(window), cx);
                    } else {
                        ws.check_for_updates(cx, true);
                    }
                });
            }
        };

        let workspace_for_welcome = self.workspace.clone();
        let on_restore_welcome = move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
            if let Some(ws) = workspace_for_welcome.upgrade() {
                ws.update(cx, |ws, cx| ws.restore_welcome_guide(cx));
            }
        };

        let view_tab = cx.entity().clone();
        let on_set_tab = Arc::new(move |tab: SettingsTab, _window: &mut Window, cx: &mut App| {
            view_tab.update(cx, |this, cx| {
                if tab != SettingsTab::Shortcuts {
                    this.stop_recording_shortcut();
                    this.shortcut_conflict = None;
                    this.shortcut_notice = None;
                }
                this.current_tab = tab;
                cx.notify();
            });
        });

        let view_mode = cx.entity().clone();
        let on_set_mode = Arc::new(move |mode: ThemeMode, _window: &mut Window, cx: &mut App| {
            view_mode.update(cx, |this, cx| {
                this.preferences.theme = mode;
                this.editing_custom_dark = None;
                let dark = match mode {
                    ThemeMode::Dark => true,
                    ThemeMode::Light => false,
                    ThemeMode::Auto => is_sys_dark,
                };
                this.theme = ThemeColors::resolve_preferences(&this.preferences, dark);
                if let Some(ws) = this.workspace.upgrade() {
                    ws.update(cx, |ws, cx| {
                        ws.state.set_theme_mode(mode);
                        cx.notify();
                    });
                }
                cx.notify();
            });
        });

        let view_pal = cx.entity().clone();
        let on_set_palette = Arc::new(move |palette: &'static str, _window: &mut Window, cx: &mut App| {
            view_pal.update(cx, |this, cx| {
                this.preferences.palette = palette.to_string();
                if palette != CUSTOM_PALETTE_ID {
                    this.editing_custom_dark = None;
                    this.custom_preset_open = false;
                }
                let dark = match this.preferences.theme {
                    ThemeMode::Dark => true,
                    ThemeMode::Light => false,
                    ThemeMode::Auto => is_sys_dark,
                };
                this.theme = ThemeColors::resolve_preferences(&this.preferences, dark);
                if let Some(ws) = this.workspace.upgrade() {
                    ws.update(cx, |ws, cx| {
                        ws.state.set_palette(palette);
                        cx.notify();
                    });
                }
                cx.notify();
            });
        });

        let view_edit_dark = cx.entity().clone();
        let on_set_editing_dark = Arc::new(move |dark: bool, _window: &mut Window, cx: &mut App| {
            view_edit_dark.update(cx, |this, cx| {
                this.editing_custom_dark = Some(dark);
                cx.notify();
            });
        });

        let view_toggle_syntax = cx.entity().clone();
        let on_toggle_custom_syntax = Arc::new(move |_window: &mut Window, cx: &mut App| {
            view_toggle_syntax.update(cx, |this, cx| {
                this.custom_syntax_expanded = !this.custom_syntax_expanded;
                cx.notify();
            });
        });

        let view_toggle_preset = cx.entity().clone();
        let on_toggle_custom_preset = Arc::new(move |_window: &mut Window, cx: &mut App| {
            view_toggle_preset.update(cx, |this, cx| {
                this.custom_preset_open = !this.custom_preset_open;
                cx.notify();
            });
        });

        let view_import = cx.entity().clone();
        let on_import_preset = Arc::new(move |dark: bool, preset_id: &'static str, _window: &mut Window, cx: &mut App| {
            view_import.update(cx, |this, cx| {
                let imported = ThemeColors::resolve(preset_id, dark).to_custom_palette();
                *this.preferences.custom_theme.variant_mut(dark) = imported;
                this.preferences.palette = CUSTOM_PALETTE_ID.to_string();
                this.custom_preset_open = false;
                if let Some(ws) = this.workspace.upgrade() {
                    ws.update(cx, |ws, cx| {
                        ws.state.import_palette_into_custom(dark, preset_id);
                        cx.notify();
                    });
                }
                cx.notify();
            });
        });

        let view_reset = cx.entity().clone();
        let on_reset_custom = Arc::new(move |dark: bool, _window: &mut Window, cx: &mut App| {
            view_reset.update(cx, |this, cx| {
                *this.preferences.custom_theme.variant_mut(dark) = Default::default();
                if let Some(ws) = this.workspace.upgrade() {
                    ws.update(cx, |ws, cx| {
                        ws.state.reset_custom_theme(dark);
                        cx.notify();
                    });
                }
                cx.notify();
            });
        });

        let view_loc = cx.entity().clone();
        let on_set_locale = Arc::new(move |locale: SupportedLocale, _window: &mut Window, cx: &mut App| {
            view_loc.update(cx, |this, cx| {
                this.preferences.locale = locale;
                this.strings = zmark_core::i18n::get_strings(locale);
                if let Some(ws) = this.workspace.upgrade() {
                    ws.update(cx, |ws, cx| {
                        ws.state.set_locale(locale);
                        cx.notify();
                    });
                }
                cx.notify();
            });
        });

        let view_as = cx.entity().clone();
        let on_toggle_auto_save = Arc::new(move |enabled: bool, _window: &mut Window, cx: &mut App| {
            view_as.update(cx, |this, cx| {
                this.preferences.auto_save = enabled;
                if let Some(ws) = this.workspace.upgrade() {
                    ws.update(cx, |ws, cx| {
                        ws.state.set_auto_save(enabled);
                        cx.notify();
                    });
                }
                cx.notify();
            });
        });

        let view_asb = cx.entity().clone();
        let on_toggle_auto_save_on_blur = Arc::new(move |enabled: bool, _window: &mut Window, cx: &mut App| {
            view_asb.update(cx, |this, cx| {
                this.preferences.auto_save_on_blur = enabled;
                if let Some(ws) = this.workspace.upgrade() {
                    ws.update(cx, |ws, cx| {
                        ws.state.set_auto_save_on_blur(enabled);
                        cx.notify();
                    });
                }
                cx.notify();
            });
        });

        let view_ln = cx.entity().clone();
        let on_toggle_line_numbers = Arc::new(move |enabled: bool, _window: &mut Window, cx: &mut App| {
            view_ln.update(cx, |this, cx| {
                this.preferences.show_line_numbers = enabled;
                if let Some(ws) = this.workspace.upgrade() {
                    ws.update(cx, |ws, cx| {
                        ws.state.set_show_line_numbers(enabled);
                        cx.notify();
                    });
                }
                cx.notify();
            });
        });

        let view_rm = cx.entity().clone();
        let on_toggle_reduce_motion = Arc::new(move |enabled: bool, _window: &mut Window, cx: &mut App| {
            view_rm.update(cx, |this, cx| {
                this.preferences.reduce_motion = enabled;
                if let Some(ws) = this.workspace.upgrade() {
                    ws.update(cx, |ws, cx| {
                        ws.state.set_reduce_motion(enabled);
                        cx.notify();
                    });
                }
                cx.notify();
            });
        });

        let view_ts = cx.entity().clone();
        let on_set_tab_size = Arc::new(move |size: usize, _window: &mut Window, cx: &mut App| {
            view_ts.update(cx, |this, cx| {
                this.preferences.tab_size = size;
                if let Some(ws) = this.workspace.upgrade() {
                    ws.update(cx, |ws, cx| {
                        ws.state.set_tab_size(size);
                        cx.notify();
                    });
                }
                cx.notify();
            });
        });

        let view_lh = cx.entity().clone();
        let on_set_line_height = Arc::new(move |lh: f32, _window: &mut Window, cx: &mut App| {
            view_lh.update(cx, |this, cx| {
                let clamped = Preferences::sanitize_line_height(lh);
                this.preferences.line_height = clamped;
                if let Some(ws) = this.workspace.upgrade() {
                    ws.update(cx, |ws, cx| {
                        ws.state.set_line_height(clamped);
                        cx.notify();
                    });
                }
                cx.notify();
            });
        });

        let view_cw = cx.entity().clone();
        let on_set_column_width = Arc::new(move |w: f32, _window: &mut Window, cx: &mut App| {
            view_cw.update(cx, |this, cx| {
                let clamped = Preferences::sanitize_column_width(w);
                this.preferences.max_column_width = clamped;
                if let Some(ws) = this.workspace.upgrade() {
                    ws.update(cx, |ws, cx| {
                        ws.state.set_column_width(clamped);
                        ws.state.save_preferences();
                        cx.notify();
                    });
                }
                cx.notify();
            });
        });

        div()
            .size_full()
            .relative()
            .bg(theme.bg)
            .text_color(theme.text_primary)
            .flex()
            .flex_row()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                if this.recording_shortcut.is_some() {
                    this.stop_recording_shortcut();
                    this.shortcut_notice = None;
                    cx.notify();
                }
            }))
            // Left Sidebar Navigation (Edge-to-Edge full height with traffic-lights safe insets)
            .child(
                div()
                    .w(px(160.))
                    .flex_shrink_0()
                    .h_full()
                    .border_r_1()
                    .border_color(theme.border)
                    .bg(theme.bg)
                    .pt(px(36.))
                    .px_2()
                    .pb_2()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(render_nav_item(
                        strings.appearance,
                        "icons/palette.svg",
                        current_tab == SettingsTab::Appearance,
                        &theme,
                        {
                            let t = on_set_tab.clone();
                            move |_, window, cx| t(SettingsTab::Appearance, window, cx)
                        },
                    ))
                    .child(render_nav_item(
                        strings.editor_and_save,
                        "icons/save.svg",
                        current_tab == SettingsTab::EditorSave,
                        &theme,
                        {
                            let t = on_set_tab.clone();
                            move |_, window, cx| t(SettingsTab::EditorSave, window, cx)
                        },
                    ))
                    .child(render_nav_item(
                        strings.language,
                        "icons/languages.svg",
                        current_tab == SettingsTab::Language,
                        &theme,
                        {
                            let t = on_set_tab.clone();
                            move |_, window, cx| t(SettingsTab::Language, window, cx)
                        },
                    ))
                    .child(render_nav_item(
                        strings.shortcuts_label,
                        "icons/keyboard.svg",
                        current_tab == SettingsTab::Shortcuts,
                        &theme,
                        {
                            let t = on_set_tab.clone();
                            move |_, window, cx| t(SettingsTab::Shortcuts, window, cx)
                        },
                    ))
                    .child(render_nav_item(
                        strings.about,
                        "icons/info.svg",
                        current_tab == SettingsTab::About,
                        &theme,
                        {
                            let t = on_set_tab.clone();
                            move |_, window, cx| t(SettingsTab::About, window, cx)
                        },
                    )),
            )
            // Right Content Panel
            .child(
                div()
                    .id("settings_content_scroll")
                    .flex_1()
                    .h_full()
                    .p_6()
                    .when(cfg!(target_os = "windows"), |el| el.pt(px(36.)))
                    .bg(theme.surface)
                    .overflow_y_scroll()
                    // About keeps the title's space (so content starts at the same
                    // height on every page) but hides the text.
                    .child(
                        div()
                            .pb(px(tk::SECTION_GAP))
                            .when(current_tab == SettingsTab::About, |el| el.invisible())
                            .text_size(px(tk::TEXT_TITLE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child(match current_tab {
                                SettingsTab::Appearance => strings.appearance,
                                SettingsTab::EditorSave => strings.editor_and_save,
                                SettingsTab::Language => strings.language,
                                SettingsTab::Shortcuts => strings.shortcuts_label,
                                SettingsTab::About => strings.about,
                            }),
                    )
                    .child(match current_tab {
                        SettingsTab::Appearance => render_appearance_tab(
                            current_mode,
                            &current_palette,
                            effective_dark,
                            self.preferences.reduce_motion,
                            &theme,
                            strings,
                            CustomThemeEditorProps {
                                custom_theme: &custom_theme,
                                editing_dark,
                                syntax_expanded: custom_syntax_expanded,
                                preset_open: custom_preset_open,
                                pickers: &custom_pickers,
                                on_set_editing_dark,
                                on_toggle_syntax: on_toggle_custom_syntax,
                                on_toggle_preset: on_toggle_custom_preset,
                                on_import_preset,
                                on_reset_custom,
                            },
                            on_set_mode,
                            on_set_palette,
                            on_toggle_reduce_motion,
                        )
                        .into_any_element(),
                        SettingsTab::EditorSave => render_editor_save_tab(
                            auto_save,
                            auto_save_on_blur,
                            self.preferences.show_line_numbers,
                            tab_size,
                            line_height,
                            column_width,
                            &theme,
                            strings,
                            on_toggle_auto_save,
                            on_toggle_auto_save_on_blur,
                            on_toggle_line_numbers,
                            on_set_tab_size,
                            on_set_line_height,
                            on_set_column_width,
                        )
                        .into_any_element(),
                        SettingsTab::Language => render_language_tab(
                            current_locale,
                            &theme,
                            strings,
                            on_set_locale,
                        )
                        .into_any_element(),
                        SettingsTab::Shortcuts => {
                            let view_rec = cx.entity().clone();
                            let on_start_record: Arc<
                                dyn Fn(ShortcutActionId, &mut Window, &mut App),
                            > = Arc::new(move |action, window, cx| {
                                view_rec.update(cx, |this, cx| {
                                    this.begin_recording_shortcut(action, window, cx);
                                });
                            });
                            let view_res = cx.entity().clone();
                            let on_reset_shortcut: Arc<
                                dyn Fn(ShortcutActionId, &mut Window, &mut App),
                            > = Arc::new(move |action, _window, cx| {
                                view_res.update(cx, |this, cx| {
                                    this.reset_shortcut(action, cx);
                                });
                            });
                            let view_rep = cx.entity().clone();
                            let on_replace_conflict: Arc<
                                dyn Fn(ShortcutActionId, String, ShortcutActionId, &mut Window, &mut App),
                            > = Arc::new(move |action, spec, conflict, _window, cx| {
                                view_rep.update(cx, |this, cx| {
                                    this.replace_shortcut_conflict(action, spec, conflict, cx);
                                });
                            });
                            let view_can = cx.entity().clone();
                            let on_cancel_conflict: Arc<dyn Fn(&mut Window, &mut App)> =
                                Arc::new(move |_window, cx| {
                                    view_can.update(cx, |this, cx| {
                                        this.shortcut_conflict = None;
                                        this.shortcut_notice = None;
                                        cx.notify();
                                    });
                                });
                            let view_all = cx.entity().clone();
                            let on_reset_all: Arc<dyn Fn(&mut Window, &mut App)> =
                                Arc::new(move |_window, cx| {
                                    view_all.update(cx, |this, cx| {
                                        this.reset_all_shortcuts(cx);
                                    });
                                });
                            render_shortcuts_tab(
                                &self.preferences,
                                cx.platform_info().os,
                                self.recording_shortcut,
                                self.shortcut_conflict.as_ref(),
                                self.shortcut_notice,
                                &theme,
                                strings,
                                ShortcutsTabHandlers {
                                    on_start_record,
                                    on_reset_shortcut,
                                    on_replace_conflict,
                                    on_cancel_conflict,
                                    on_reset_all,
                                },
                            )
                            .into_any_element()
                        }
                        SettingsTab::About => render_about_tab(
                            &theme,
                            strings,
                            &update_status,
                            update_enabled,
                            update_reason.as_deref(),
                            on_check_update,
                            move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                                cx.open_url("https://github.com/2716190547/zmark");
                            },
                            crate::platform::licenses_dir().map(|dir| {
                                move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                                    cx.open_with_system(&dir);
                                }
                            }),
                            on_restore_welcome,
                        )
                        .into_any_element(),
                    }),
            )
            .when(cfg!(target_os = "windows"), |el| {
                el.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .h(px(32.))
                        .flex()
                        // Opaque bar: settings content scrolls under the caption buttons
                        // and would otherwise show through and overlap them.
                        .bg(theme.bg)
                        .border_b_1()
                        .border_color(theme.border)
                        .child(div().flex_1().h_full().occlude().window_control_area(WindowControlArea::Drag))
                        .child(crate::views::workspace::render_window_controls(&theme)),
                )
            })
    }
}

fn render_nav_item<F>(
    title: &'static str,
    icon_path: &'static str,
    active: bool,
    theme: &ThemeColors,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let base = div()
        .px_2p5()
        .h(px(32.))
        .rounded(px(tk::RADIUS))
        .cursor_pointer()
        .flex()
        .items_center()
        .gap_2()
        .text_size(px(tk::TEXT_LABEL))
        .on_mouse_down(MouseButton::Left, handler);

    let icon_color = if active {
        theme.selection_text()
    } else {
        theme.text_muted
    };
    let icon = svg().path(icon_path).size(px(16.)).text_color(icon_color);

    if active {
        base.bg(theme.control_selection_bg())
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.selection_text())
            .child(div().flex_shrink_0().child(icon))
            .child(div().flex_1().min_w(px(0.)).whitespace_normal().child(title))
    } else {
        base.text_color(theme.text_muted)
            .hover(|s| s.bg(theme.hover_bg()).text_color(theme.text_primary))
            .child(div().flex_shrink_0().child(icon))
            .child(div().flex_1().min_w(px(0.)).whitespace_normal().child(title))
    }
}

/// Section heading shared by every settings page.
fn render_section_title(text: impl Into<SharedString>, theme: &ThemeColors) -> impl IntoElement {
    div()
        .text_size(px(tk::TEXT_BODY))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.text_muted)
        .child(text.into())
}

/// Plain body/help text under a section title.
fn render_help_text(text: impl Into<SharedString>, theme: &ThemeColors) -> impl IntoElement {
    div()
        .text_size(px(tk::TEXT_BODY))
        .text_color(theme.text_muted)
        .child(text.into())
}

/// One settings row: label (+ optional description) on the left, a control on the right.
fn render_setting_row(
    label: impl Into<SharedString>,
    desc: Option<SharedString>,
    trailing: impl IntoElement,
    theme: &ThemeColors,
) -> impl IntoElement {
    div()
        .w_full()
        .py(px(tk::ROW_PY))
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .min_w(px(0.))
                .child(
                    div()
                        .text_size(px(tk::TEXT_LABEL))
                        .text_color(theme.text_primary)
                        .child(label.into()),
                )
                .when_some(desc, |el, desc| {
                    el.child(
                        div()
                            .text_size(px(tk::TEXT_BODY))
                            .text_color(theme.text_muted)
                            .child(desc),
                    )
                }),
        )
        .child(div().flex_shrink_0().child(trailing))
}

/// A section: optional title above one card. The card is a fill only (no
/// outline), so controls inside it never sit in a frame within a frame.
fn render_section(
    title: Option<SharedString>,
    body: impl IntoElement,
    theme: &ThemeColors,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(tk::TITLE_GAP))
        .when_some(title, |el, title| el.child(render_section_title(title, theme)))
        .child(body)
}

/// A card of setting rows separated by hairlines.
fn render_setting_group(
    title: Option<SharedString>,
    rows: Vec<AnyElement>,
    theme: &ThemeColors,
) -> impl IntoElement {
    let count = rows.len();
    render_section(
        title,
        div()
            .flex()
            .flex_col()
            .px(px(tk::CARD_PX))
            .rounded(px(tk::RADIUS_CARD))
            .bg(theme.card_bg())
            .children(rows.into_iter().enumerate().map(|(i, row)| {
                div()
                    .when(i + 1 < count, |el| el.border_b_1().border_color(theme.hairline()))
                    .child(row)
            })),
        theme,
    )
}

/// Neutral button: filled, no outline, fixed control height.
fn render_secondary_button<F>(
    id: &'static str,
    label: impl Into<SharedString>,
    theme: &ThemeColors,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    div()
        .id(id)
        .h(px(tk::CONTROL_H))
        .px(px(tk::CONTROL_PX))
        .rounded(px(tk::RADIUS))
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.control_bg())
        .text_size(px(tk::TEXT_BODY))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.text_primary)
        .cursor_pointer()
        .hover(|s| s.bg(theme.control_hover_bg()))
        .on_mouse_down(MouseButton::Left, handler)
        .child(label.into())
}

/// Text-only button for low-emphasis actions (reset, cancel).
fn render_text_button<F>(
    id: SharedString,
    label: impl Into<SharedString>,
    theme: &ThemeColors,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    div()
        .id(id)
        .h(px(tk::CONTROL_H))
        .px_2()
        .rounded(px(tk::RADIUS))
        .flex()
        .items_center()
        .text_size(px(tk::TEXT_BODY))
        .text_color(theme.text_muted)
        .cursor_pointer()
        .hover(|s| s.bg(theme.hover_bg()).text_color(theme.text_primary))
        .on_mouse_down(MouseButton::Left, handler)
        .child(label.into())
}

/// Segmented control track. Children are [`render_segment`]s.
fn render_segmented(theme: &ThemeColors) -> Div {
    div()
        .w_full()
        .h(px(tk::CONTROL_H))
        .p(px(tk::TRACK_INSET))
        .rounded(px(tk::RADIUS))
        .bg(theme.control_bg())
        .flex()
        .gap(px(tk::TRACK_INSET))
}

/// One option of a segmented control. The selected thumb is a raised fill
/// with a soft shadow instead of an outline.
fn render_segment<F>(
    id: SharedString,
    label: impl Into<SharedString>,
    active: bool,
    theme: &ThemeColors,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let sel = id.to_string();
    let base = div()
        .id(id)
        .debug_selector(move || sel.clone())
        .flex_1()
        .h_full()
        .rounded(px(tk::RADIUS_INNER))
        .text_size(px(tk::TEXT_BODY))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, handler)
        .child(label.into());
    if active {
        base.bg(theme.segmented_thumb_bg())
            .shadow_xs()
            .text_color(theme.segmented_thumb_text())
            .font_weight(FontWeight::MEDIUM)
    } else {
        base.text_color(theme.segmented_inactive_text())
            .hover(|s| s.text_color(theme.text_primary))
    }
}

fn render_ios_toggle<F>(
    active: bool,
    theme: &ThemeColors,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let track_bg = if active {
        theme.accent
    } else {
        theme.control_selection_bg()
    };
    div()
        .w(px(36.))
        .h(px(20.))
        .p(px(2.))
        .rounded_full()
        .bg(track_bg)
        .text_color(theme.selection_text())
        .cursor_pointer()
        .flex()
        .items_center()
        .when(active, |s| s.justify_end())
        .when(!active, |s| s.justify_start())
        .on_mouse_down(MouseButton::Left, handler)
        .child(
            div()
                .size(px(16.))
                .rounded_full()
                .bg(rgb(0xffffff))
                .shadow_sm(),
        )
}

struct CustomThemeEditorProps<'a> {
    custom_theme: &'a CustomThemePreferences,
    editing_dark: bool,
    syntax_expanded: bool,
    preset_open: bool,
    pickers: &'a HashMap<CustomColorSlot, Entity<ColorPickerState>>,
    on_set_editing_dark: Arc<dyn Fn(bool, &mut Window, &mut App)>,
    on_toggle_syntax: Arc<dyn Fn(&mut Window, &mut App)>,
    on_toggle_preset: Arc<dyn Fn(&mut Window, &mut App)>,
    on_import_preset: Arc<dyn Fn(bool, &'static str, &mut Window, &mut App)>,
    on_reset_custom: Arc<dyn Fn(bool, &mut Window, &mut App)>,
}

fn render_custom_color_rows(
    slots: &[CustomColorSlot],
    pickers: &HashMap<CustomColorSlot, Entity<ColorPickerState>>,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .children(slots.iter().filter_map(|&slot| {
            let picker = pickers.get(&slot)?;
            let label = zmark_core::i18n::localize_custom_color_slot(slot, strings);
            Some(
                div()
                    .w_full()
                    .py_1()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_size(px(tk::TEXT_LABEL))
                            .text_color(theme.text_primary)
                            .child(label),
                    )
                    .child(
                        div()
                            .w(px(136.))
                            .flex_shrink_0()
                            .child(ColorSelect::new(picker).small()),
                    ),
            )
        }))
}

fn render_custom_theme_editor(
    props: &CustomThemeEditorProps<'_>,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
) -> impl IntoElement {
    let editing_dark = props.editing_dark;
    let on_light = props.on_set_editing_dark.clone();
    let on_dark = props.on_set_editing_dark.clone();
    let on_toggle_preset = props.on_toggle_preset.clone();
    let on_reset = props.on_reset_custom.clone();
    let on_toggle_syntax = props.on_toggle_syntax.clone();

    div()
        .w_full()
        .p(px(tk::CARD_PX))
        .rounded(px(tk::RADIUS_CARD))
        .bg(theme.card_bg())
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .w_full()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    render_segmented(theme)
                        .w(px(196.))
                        .child(render_mode_pill(
                            strings.custom_editing_light,
                            !editing_dark,
                            theme,
                            move |_, window, cx| on_light(false, window, cx),
                        ))
                        .child(render_mode_pill(
                            strings.custom_editing_dark,
                            editing_dark,
                            theme,
                            move |_, window, cx| on_dark(true, window, cx),
                        )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .child(render_secondary_button(
                            "btn_custom_import_preset",
                            strings.custom_import_preset,
                            theme,
                            move |_, window, cx| on_toggle_preset(window, cx),
                        ))
                        .child(render_secondary_button(
                            "btn_custom_reset",
                            strings.custom_reset_defaults,
                            theme,
                            move |_, window, cx| on_reset(editing_dark, window, cx),
                        )),
                ),
        )
        .when(props.preset_open, |el| {
            el.child(
                div()
                    .w_full()
                    .flex()
                    .flex_wrap()
                    .gap_1p5()
                    .children(ALL_PALETTES.iter().map(|desc| {
                        let preset_id = desc.id;
                        let resolved = ThemeColors::resolve(preset_id, editing_dark);
                        let on_import = props.on_import_preset.clone();
                        div()
                            .id(SharedString::from(format!("custom_import_{preset_id}")))
                            .h(px(tk::CONTROL_H))
                            .px_2p5()
                            .rounded(px(tk::RADIUS))
                            .bg(resolved.bg)
                            .border_1()
                            .border_color(resolved.border)
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_size(px(tk::TEXT_BODY))
                            .text_color(resolved.text_primary)
                            .hover(|s| s.border_color(resolved.accent))
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                on_import(editing_dark, preset_id, window, cx);
                            })
                            .child(div().size(px(8.)).rounded_full().bg(resolved.accent))
                            .child(zmark_core::i18n::localize_palette_name(preset_id, strings))
                    })),
            )
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .child(render_section_title(strings.custom_core_colors, theme))
                .child(render_custom_color_rows(
                    &CustomColorSlot::CORE,
                    props.pickers,
                    theme,
                    strings,
                )),
        )
        .child(
            div()
                .pt_2()
                .border_t_1()
                .border_color(theme.hairline())
                .flex()
                .flex_col()
                .gap_1p5()
                .child(
                    div()
                        .id("btn_toggle_custom_syntax")
                        .py_1()
                        .flex()
                        .items_center()
                        .justify_between()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            on_toggle_syntax(window, cx);
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    svg()
                                        .path(if props.syntax_expanded {
                                            "icons/chevron-down.svg"
                                        } else {
                                            "icons/chevron-right.svg"
                                        })
                                        .size(px(14.))
                                        .text_color(theme.text_muted),
                                )
                                .child(render_section_title(strings.custom_syntax_colors, theme)),
                        )
                        .child(
                            div()
                                .text_size(px(tk::TEXT_BODY))
                                .text_color(theme.text_muted)
                                .child("12"),
                        ),
                )
                .when(props.syntax_expanded, |el| {
                    el.child(render_custom_color_rows(
                        &CustomColorSlot::SYNTAX,
                        props.pickers,
                        theme,
                        strings,
                    ))
                }),
        )
}

fn render_appearance_tab(
    current_mode: ThemeMode,
    current_palette: &str,
    effective_dark: bool,
    reduce_motion: bool,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    custom: CustomThemeEditorProps<'_>,
    on_set_mode: Arc<dyn Fn(ThemeMode, &mut Window, &mut App)>,
    on_set_palette: Arc<dyn Fn(&'static str, &mut Window, &mut App)>,
    on_toggle_reduce_motion: Arc<dyn Fn(bool, &mut Window, &mut App)>,
) -> impl IntoElement {
    let m1 = on_set_mode.clone();
    let m2 = on_set_mode.clone();
    let m3 = on_set_mode;
    let is_custom_active = current_palette == CUSTOM_PALETTE_ID;
    let resolved_custom = ThemeColors::from_custom_palette(
        custom.custom_theme.variant(custom.editing_dark),
        custom.editing_dark,
    );
    let on_select_custom = on_set_palette.clone();

    div()
        .flex()
        .flex_col()
        .gap(px(tk::SECTION_GAP))
        .child(render_section(
            None,
            div()
                .flex()
                .gap_3()
                .child(render_mode_preview(
                    "mode_auto", strings.theme_auto, ModePreview::Auto, current_palette,
                    current_mode == ThemeMode::Auto, theme,
                    move |_, window, cx| m1(ThemeMode::Auto, window, cx),
                ))
                .child(render_mode_preview(
                    "mode_light", strings.theme_light, ModePreview::Light, current_palette,
                    current_mode == ThemeMode::Light, theme,
                    move |_, window, cx| m2(ThemeMode::Light, window, cx),
                ))
                .child(render_mode_preview(
                    "mode_dark", strings.theme_dark, ModePreview::Dark, current_palette,
                    current_mode == ThemeMode::Dark, theme,
                    move |_, window, cx| m3(ThemeMode::Dark, window, cx),
                )),
            theme,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    render_section_title(strings.palette, theme),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(ALL_PALETTES.chunks(2).map(|pair| {
                            div()
                                .w_full()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .children(pair.iter().map(|desc| {
                                    let is_active = current_palette == desc.id;
                                    let palette_id = desc.id;
                                    let on_set = on_set_palette.clone();
                                    let resolved = ThemeColors::resolve(desc.id, effective_dark);

                                    div()
                                        .flex_1()
                                        .p_3()
                                        .rounded(px(tk::RADIUS_CARD))
                                        .bg(resolved.bg)
                                        .cursor_pointer()
                                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                            on_set(palette_id, window, cx);
                                        })
                                        .flex()
                                        .flex_col()
                                        .justify_between()
                                        .gap_3()
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .justify_between()
                                                .child(
                                                    div()
                                                        .text_base()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(resolved.text_primary)
                                                        .child("Aa"),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(24.))
                                                        .h(px(4.))
                                                        .rounded_sm()
                                                        .bg(resolved.accent),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(tk::TEXT_BODY))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(resolved.text_primary)
                                                .child(zmark_core::i18n::localize_palette_name(desc.id, strings)),
                                        )
                                        .when(is_active, |c| c.border_2().border_color(theme.accent))
                                        .when(!is_active, |c| c.border_2().border_color(gpui::transparent_black()))
                                }))
                        }))
                        .child(
                            div()
                                .id("palette_card_custom")
                                .w_full()
                                .p_3()
                                .rounded(px(tk::RADIUS_CARD))
                                .bg(resolved_custom.bg)
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    on_select_custom(CUSTOM_PALETTE_ID, window, cx);
                                })
                                .flex()
                                .flex_col()
                                .justify_between()
                                .gap_3()
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_base()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(resolved_custom.text_primary)
                                                .child("Aa"),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_1()
                                                .child(div().size(px(8.)).rounded_full().bg(resolved_custom.surface))
                                                .child(div().size(px(8.)).rounded_full().bg(resolved_custom.syntax_heading))
                                                .child(div().size(px(8.)).rounded_full().bg(resolved_custom.syntax_keyword))
                                                .child(div().w(px(24.)).h(px(4.)).rounded_sm().bg(resolved_custom.accent)),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(tk::TEXT_BODY))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(resolved_custom.text_primary)
                                        .child(zmark_core::i18n::localize_palette_name(
                                            CUSTOM_PALETTE_ID,
                                            strings,
                                        )),
                                )
                                .when(is_custom_active, |c| c.border_2().border_color(theme.accent))
                                .when(!is_custom_active, |c| {
                                    c.border_2().border_color(gpui::transparent_black())
                                }),
                        ),
                ),
        )
        .when(is_custom_active, |el| {
            el.child(render_custom_theme_editor(&custom, theme, strings))
        })
        .child(render_setting_row(
            strings.reduce_motion_label,
            Some(strings.reduce_motion_desc.into()),
            render_ios_toggle(reduce_motion, theme, move |_, window, cx| {
                on_toggle_reduce_motion(!reduce_motion, window, cx);
            }),
            theme,
        ))
}

fn render_editor_save_tab(
    auto_save: bool,
    auto_save_on_blur: bool,
    show_line_numbers: bool,
    tab_size: usize,
    line_height: f32,
    column_width: f32,
    theme: &ThemeColors,
    strings: &I18nStrings,
    on_toggle_auto_save: Arc<dyn Fn(bool, &mut Window, &mut App)>,
    on_toggle_auto_save_on_blur: Arc<dyn Fn(bool, &mut Window, &mut App)>,
    on_toggle_line_numbers: Arc<dyn Fn(bool, &mut Window, &mut App)>,
    on_set_tab_size: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_set_line_height: Arc<dyn Fn(f32, &mut Window, &mut App)>,
    on_set_column_width: Arc<dyn Fn(f32, &mut Window, &mut App)>,
) -> impl IntoElement {
    let t_as = on_toggle_auto_save;
    let t_asb = on_toggle_auto_save_on_blur;
    let t_ln = on_toggle_line_numbers;
    let t_ts2 = on_set_tab_size.clone();
    let t_ts4 = on_set_tab_size;

    div()
        .flex()
        .flex_col()
        .gap(px(tk::SECTION_GAP))
        .child(render_setting_group(
            None,
            vec![
                render_setting_row(
                    strings.auto_save_label,
                    Some(strings.auto_save_desc.into()),
                    render_ios_toggle(auto_save, theme, move |_, window, cx| {
                        t_as(!auto_save, window, cx);
                    }),
                    theme,
                )
                .into_any_element(),
                render_setting_row(
                    strings.auto_save_on_blur_label,
                    None,
                    render_ios_toggle(auto_save_on_blur, theme, move |_, window, cx| {
                        t_asb(!auto_save_on_blur, window, cx);
                    }),
                    theme,
                )
                .into_any_element(),
                render_setting_row(
                    strings.show_line_numbers_label,
                    Some(strings.show_line_numbers_desc.into()),
                    render_ios_toggle(show_line_numbers, theme, move |_, window, cx| {
                        t_ln(!show_line_numbers, window, cx);
                    }),
                    theme,
                )
                .into_any_element(),
            ],
            theme,
        ))
        .child(render_setting_group(
            None,
            vec![
                render_setting_row(
                    strings.tab_size_label,
                    None,
                    render_segmented(theme)
                        .w(px(168.))
                        .child(render_segment(
                            "tab_size_2".into(),
                            "2",
                            tab_size == 2,
                            theme,
                            move |_, window, cx| t_ts2(2, window, cx),
                        ))
                        .child(render_segment(
                            "tab_size_4".into(),
                            "4",
                            tab_size == 4,
                            theme,
                            move |_, window, cx| t_ts4(4, window, cx),
                        )),
                    theme,
                )
                .into_any_element(),
                render_setting_row(
                    strings.line_height_label,
                    None,
                    render_segmented(theme)
                        .w(px(260.))
                        .children([1.3f32, 1.4, 1.5, 1.6, 1.7].into_iter().map(|lh_opt| {
                            let on_set = on_set_line_height.clone();
                            render_segment(
                                format!("line_height_{lh_opt:.1}").into(),
                                format!("{lh_opt:.1}"),
                                (line_height - lh_opt).abs() < 0.05,
                                theme,
                                move |_, window, cx| on_set(lh_opt, window, cx),
                            )
                        })),
                    theme,
                )
                .into_any_element(),
                render_setting_row(
                    strings.column_width,
                    Some(strings.column_width_desc.into()),
                    render_segmented(theme)
                        .w(px(300.))
                        .children([640.0f32, 760.0, 880.0, 1000.0, 1200.0].into_iter().map(|w_opt| {
                            let on_set = on_set_column_width.clone();
                            render_segment(
                                format!("column_width_{w_opt:.0}").into(),
                                format!("{w_opt:.0}"),
                                (column_width - w_opt).abs() < 0.5,
                                theme,
                                move |_, window, cx| on_set(w_opt, window, cx),
                            )
                        })),
                    theme,
                )
                .into_any_element(),
            ],
            theme,
        ))
}

fn render_language_tab(
    current_locale: SupportedLocale,
    theme: &ThemeColors,
    _strings: &'static I18nStrings,
    on_set_locale: Arc<dyn Fn(SupportedLocale, &mut Window, &mut App)>,
) -> impl IntoElement {
    // One list card like every other page: rows split by hairlines, the
    // current language marked with a check instead of a filled block.
    let locales = SupportedLocale::all();
    let count = locales.len();
    div()
        .flex()
        .flex_col()
        .px(px(tk::CARD_PX))
        .rounded(px(tk::RADIUS_CARD))
        .bg(theme.card_bg())
        .children(locales.iter().enumerate().map(|(i, &(loc, tag, name))| {
            let is_active = current_locale.resolve() == loc;
            let on_set = on_set_locale.clone();
            div()
                .id(SharedString::from(format!("locale_{tag}")))
                .h(px(40.))
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .when(i + 1 < count, |el| el.border_b_1().border_color(theme.hairline()))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| on_set(loc, window, cx))
                .child(
                    div()
                        .text_size(px(tk::TEXT_LABEL))
                        .text_color(theme.text_primary)
                        .when(is_active, |el| el.font_weight(FontWeight::MEDIUM))
                        .when(!is_active, |el| el.hover(|s| s.text_color(theme.accent)))
                        .child(name),
                )
                .when(is_active, |el| {
                    el.child(
                        svg()
                            .path("icons/check.svg")
                            .size(px(16.))
                            .text_color(theme.accent),
                    )
                })
        }))
}

pub fn shortcut_rows(strings: &I18nStrings) -> [(String, String); 6] {
    let hint = crate::platform::shortcut_hint;
    [
        (hint("O"), strings.open_file.to_string()),
        (hint("S"), strings.save_file.to_string()),
        (format!("⇧{}", hint("E")), strings.export_html.to_string()),
        (hint("B"), strings.directory_drawer.to_string()),
        (
            [1, 2, 3]
                .iter()
                .map(|n| hint(&n.to_string()))
                .collect::<Vec<_>>()
                .join(" / "),
            format!(
                "{} / {} / {}",
                strings.source_mode, strings.split_mode, strings.preview_mode
            ),
        ),
        (hint(","), strings.settings.to_string()),
    ]
}

struct ShortcutsTabHandlers {
    on_start_record: Arc<dyn Fn(ShortcutActionId, &mut Window, &mut App)>,
    on_reset_shortcut: Arc<dyn Fn(ShortcutActionId, &mut Window, &mut App)>,
    on_replace_conflict: Arc<dyn Fn(ShortcutActionId, String, ShortcutActionId, &mut Window, &mut App)>,
    on_cancel_conflict: Arc<dyn Fn(&mut Window, &mut App)>,
    on_reset_all: Arc<dyn Fn(&mut Window, &mut App)>,
}

fn render_shortcuts_tab(
    preferences: &Preferences,
    os: OperatingSystem,
    recording: Option<ShortcutActionId>,
    conflict: Option<&(ShortcutActionId, String, ShortcutActionId)>,
    notice: Option<(ShortcutActionId, &'static str)>,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    handlers: ShortcutsTabHandlers,
) -> impl IntoElement {
    let has_overrides = !preferences.shortcuts.is_empty();
    let on_reset_all = handlers.on_reset_all.clone();

    div()
        .flex()
        .flex_col()
        .gap(px(tk::SECTION_GAP))
        // The nav already names this page, so the header is just the hint
        // and, once something is customized, "reset all".
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(render_help_text(strings.shortcut_help_banner, theme))
                .when(has_overrides, |el| {
                    el.child(render_secondary_button(
                        "btn_reset_all_shortcuts",
                        strings.shortcut_reset_all,
                        theme,
                        move |_, window, cx| on_reset_all(window, cx),
                    ))
                }),
        )
        .children(ShortcutGroup::ALL.into_iter().map(|group| {
            let group_actions: Vec<ShortcutActionId> = ShortcutActionId::ALL
                .into_iter()
                .filter(|a| a.group() == group)
                .collect();
            let count = group_actions.len();
            render_section(
                Some(localize_shortcut_group(group, strings).into()),
                div()
                        .flex()
                        .flex_col()
                        .px(px(tk::CARD_PX))
                        .rounded(px(tk::RADIUS_CARD))
                        .bg(theme.card_bg())
                        .children(group_actions.into_iter().enumerate().map(|(index, action)| {
                            let is_recording = recording == Some(action);
                            let is_customized = preferences.is_shortcut_customized(action);
                            let spec = preferences.shortcut_spec(action);
                            let key_label = if is_recording {
                                strings.shortcut_recording_prompt.to_string()
                            } else {
                                crate::actions::format_shortcut_spec(spec, os)
                                    .unwrap_or_else(|| strings.shortcut_unbound.to_string())
                            };
                            let row_conflict =
                                conflict.filter(|(target, _, _)| *target == action).cloned();
                            let row_notice = notice.filter(|(target, _)| *target == action).map(|(_, msg)| msg);

                            let on_rec = handlers.on_start_record.clone();
                            let on_res = handlers.on_reset_shortcut.clone();
                            let on_rep = handlers.on_replace_conflict.clone();
                            let on_can = handlers.on_cancel_conflict.clone();

                            // Same fill-only cap as a neutral button; recording is the one
                            // state that earns an outline, in the accent color.
                            let pill_bg = if is_recording || is_customized {
                                theme.control_selection_bg()
                            } else {
                                theme.control_bg()
                            };
                            let pill_border = if is_recording { theme.accent } else { gpui::transparent_black() };
                            let pill_text = if is_recording || is_customized {
                                theme.selection_text()
                            } else if spec.trim().is_empty() {
                                theme.text_muted
                            } else {
                                theme.text_primary
                            };
                            let edit_hint = strings.shortcut_edit_hint;

                            let btn_id = format!("shortcut_btn_{}", action.id());
                            let btn_sel = btn_id.clone();
                            let res_id = format!("shortcut_reset_{}", action.id());
                            let rep_id = format!("shortcut_replace_{}", action.id());
                            let can_id = format!("shortcut_cancel_{}", action.id());

                            div()
                                .flex()
                                .flex_col()
                                .gap_1p5()
                                .py(px(tk::ROW_PY))
                                .when(index + 1 < count, |el| {
                                    el.border_b_1().border_color(theme.hairline())
                                })
                                .child(
                                    div()
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap_2()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_1p5()
                                                .child(
                                                    div()
                                                        .text_size(px(tk::TEXT_LABEL))
                                                        .text_color(theme.text_primary)
                                                        .child(localize_shortcut_action(action, strings)),
                                                )
                                                .when(is_customized && !is_recording, |el| {
                                                    el.child(
                                                        div()
                                                            .text_size(px(tk::TEXT_SMALL))
                                                            .text_color(theme.text_muted)
                                                            .child(strings.shortcut_custom_badge),
                                                    )
                                                }),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_1p5()
                                                .child(
                                                    div()
                                                        .id(SharedString::from(btn_id))
                                                        .debug_selector(move || btn_sel.clone())
                                                        .h(px(tk::CONTROL_H))
                                                        .min_w(px(tk::KEYCAP_MIN_W))
                                                        .px(px(tk::CONTROL_PX - 2.0))
                                                        .rounded(px(tk::RADIUS))
                                                        .border_1()
                                                        .border_color(pill_border)
                                                        .bg(pill_bg)
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap_1p5()
                                                        .text_size(px(tk::TEXT_BODY))
                                                        .when(is_recording, |el| el.font_weight(FontWeight::MEDIUM))
                                                        .font_family(crate::platform::MONO_FONT)
                                                        .text_color(pill_text)
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(theme.control_hover_bg()))
                                                        .tooltip(move |_, cx| {
                                                            cx.new(|_| crate::views::workspace::TooltipView {
                                                                text: edit_hint.to_string(),
                                                            })
                                                            .into()
                                                        })
                                                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                                            cx.stop_propagation();
                                                            on_rec(action, window, cx);
                                                        })
                                                        .child(key_label)
                                                        .when(!is_recording, |el| {
                                                            el.child(
                                                                svg()
                                                                    .path("icons/pencil.svg")
                                                                    .size(px(11.))
                                                                    .flex_shrink_0()
                                                                    .text_color(theme.text_muted),
                                                            )
                                                        }),
                                                )
                                                .when(is_customized && !is_recording, |el| {
                                                    let res_sel = res_id.clone();
                                                    el.child(
                                                        div().debug_selector(move || res_sel.clone()).child(
                                                            render_text_button(
                                                                SharedString::from(res_id),
                                                                strings.shortcut_reset,
                                                                theme,
                                                                move |_, window, cx| {
                                                                    cx.stop_propagation();
                                                                    on_res(action, window, cx);
                                                                },
                                                            ),
                                                        ),
                                                    )
                                                }),
                                        ),
                                )
                                .when_some(row_notice, |el, msg| {
                                    el.child(render_help_text(msg, theme))
                                })
                                .when_some(row_conflict, |el, (target, new_spec, conflicting)| {
                                    let conflict_name = localize_shortcut_action(conflicting, strings);
                                    let key_display = crate::actions::format_shortcut_spec(&new_spec, os)
                                        .unwrap_or_else(|| new_spec.clone());
                                    let conflict_msg = format!(
                                        "{} ({key_display})",
                                        strings.shortcut_conflict_hint.replace("{action}", conflict_name)
                                    );
                                    let rep_sel = rep_id.clone();
                                    let can_sel = can_id.clone();
                                    el.child(
                                        div()
                                            .w_full()
                                            .px(px(tk::CARD_PX))
                                            .py_1p5()
                                            .rounded(px(tk::RADIUS))
                                            .bg(theme.control_selection_bg())
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_size(px(tk::TEXT_BODY))
                                                    .text_color(theme.selection_text())
                                                    .child(conflict_msg),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_1p5()
                                                    .child(
                                                        div()
                                                            .id(SharedString::from(rep_id))
                                                            .debug_selector(move || rep_sel.clone())
                                                            .h(px(tk::CONTROL_H - 4.0))
                                                            .px(px(tk::CONTROL_PX - 2.0))
                                                            .rounded(px(tk::RADIUS))
                                                            .flex()
                                                            .items_center()
                                                            .bg(theme.text_primary)
                                                            .text_size(px(tk::TEXT_BODY))
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(theme.surface)
                                                            .cursor_pointer()
                                                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                                                cx.stop_propagation();
                                                                on_rep(target, new_spec.clone(), conflicting, window, cx);
                                                            })
                                                            .child(strings.shortcut_replace),
                                                    )
                                                    .child(
                                                        div()
                                                            .id(SharedString::from(can_id))
                                                            .debug_selector(move || can_sel.clone())
                                                            .h(px(tk::CONTROL_H - 4.0))
                                                            .px_2()
                                                            .rounded(px(tk::RADIUS))
                                                            .flex()
                                                            .items_center()
                                                            .text_size(px(tk::TEXT_BODY))
                                                            .text_color(theme.text_muted)
                                                            .cursor_pointer()
                                                            .hover(|s| s.bg(theme.hover_bg()))
                                                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                                                cx.stop_propagation();
                                                                on_can(window, cx);
                                                            })
                                                            .child(strings.modal_cancel),
                                                    ),
                                            ),
                                    )
                                })
                        })),
                theme,
            )
        }))
}

fn render_about_tab(
    theme: &ThemeColors,
    strings: &I18nStrings,
    update_status: &UpdateStatus,
    update_enabled: bool,
    update_reason: Option<&str>,
    on_check_update: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    on_open_github: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    on_open_licenses: Option<impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>,
    on_restore_welcome: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let check_label = if matches!(update_status, UpdateStatus::Downloaded(_)) {
        strings.restart_update
    } else {
        strings.check_updates
    };
    let update_text = match update_status {
        UpdateStatus::Checking | UpdateStatus::Installing => strings.update_checking.to_string(),
        UpdateStatus::Latest => strings.up_to_date.to_string(),
        UpdateStatus::Error(error) => format!("{}: {error}", strings.update_failed),
        UpdateStatus::Downloaded(version) => format!("{} v{version}", strings.update_ready),
        UpdateStatus::Idle if update_enabled => String::new(),
        UpdateStatus::Idle => update_reason
            .unwrap_or(strings.update_disabled)
            .to_string(),
    };

    div()
        .flex()
        .flex_col()
        .gap(px(tk::SECTION_GAP))
        // Product header: centered logo, name, tagline. The About page skips the
        // page title, so this header is its heading.
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .child(img("brand/logo.png").w(px(64.)).h(px(64.)).flex_shrink_0())
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text_primary)
                        .child("ZMark"),
                )
                .child(render_help_text(strings.about_tagline, theme)),
        )
        .child(render_setting_group(
            Some(strings.software_updates.into()),
            vec![render_setting_row(
                format!("{} {}", strings.version_label, crate::APP_VERSION),
                Some(update_text).filter(|text| !text.is_empty()).map(Into::into),
                render_secondary_button(
                    "btn_check_updates",
                    check_label,
                    theme,
                    on_check_update,
                ),
                theme,
            )
            .into_any_element()],
            theme,
        ))
        .child(render_setting_group(
            Some(strings.help_section_label.into()),
            vec![render_setting_row(
                strings.welcome_guide_label,
                Some(strings.welcome_guide_desc.into()),
                render_secondary_button(
                    "btn_restore_welcome",
                    strings.welcome_guide_restore,
                    theme,
                    on_restore_welcome,
                ),
                theme,
            )
            .into_any_element()],
            theme,
        ))
        .child(render_setting_group(
            Some(strings.open_source_label.into()),
            {
                let mut rows = vec![render_setting_row(
                    strings.source_code_label,
                    Some(format!("github.com/2716190547/zmark · {}", strings.footer_credit).into()),
                    render_secondary_button("btn_open_github", "GitHub", theme, on_open_github),
                    theme,
                )
                .into_any_element()];
                if let Some(on_open_licenses) = on_open_licenses {
                    rows.push(
                        render_setting_row(
                            strings.licenses_label,
                            Some(strings.licenses_desc.into()),
                            render_secondary_button(
                                "btn_open_licenses",
                                strings.licenses_open,
                                theme,
                                on_open_licenses,
                            ),
                            theme,
                        )
                        .into_any_element(),
                    );
                }
                rows
            },
            theme,
        ))
}

#[derive(Clone, Copy)]
enum ModePreview {
    Auto,
    Light,
    Dark,
}

/// A small drawn window in the palette's light or dark colors; "Auto" shows
/// the light window on the left and the dark one on the right.
fn render_window_sketch(colors: &ThemeColors, radius: f32) -> Div {
    // GPUI clips overflow to a rectangle, not the parent's rounded corners, so
    // the sketch paints its own rounded fill and title bar.
    let line = |w: f32, c: Hsla| div().w(px(w)).h(px(3.)).rounded_full().bg(c);
    div()
        .size_full()
        .rounded(px(radius))
        .bg(colors.bg)
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(12.))
                .px(px(6.))
                .rounded_t(px(radius))
                .flex()
                .items_center()
                .gap(px(3.))
                .bg(colors.surface)
                .child(div().size(px(4.)).rounded_full().bg(rgb(0xff5f57)))
                .child(div().size(px(4.)).rounded_full().bg(rgb(0xfebc2e)))
                .child(div().size(px(4.)).rounded_full().bg(rgb(0x28c840))),
        )
        .child(
            div()
                .flex_1()
                .p(px(8.))
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(line(36., colors.accent))
                .child(line(56., colors.text_primary.opacity(0.55)))
                .child(line(44., colors.text_primary.opacity(0.35)))
                .child(line(30., colors.text_primary.opacity(0.35))),
        )
}

#[allow(clippy::too_many_arguments)]
fn render_mode_preview<F>(
    id: &'static str,
    label: &'static str,
    mode: ModePreview,
    palette: &str,
    active: bool,
    theme: &ThemeColors,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let light = ThemeColors::resolve(palette, false);
    let dark = ThemeColors::resolve(palette, true);
    let r = tk::RADIUS_CARD - 2.0;
    let picture = match mode {
        ModePreview::Light => render_window_sketch(&light, r),
        ModePreview::Dark => render_window_sketch(&dark, r),
        ModePreview::Auto => div()
            .size_full()
            .relative()
            .child(render_window_sketch(&light, r))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(relative(0.5))
                    .overflow_hidden()
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right_0()
                            .w(px(104.))
                            .child(render_window_sketch(&dark, r)),
                    ),
            ),
    };
    div()
        .id(id)
        .debug_selector(move || label.to_owned())
        .flex()
        .flex_col()
        .items_center()
        .gap_1p5()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, handler)
        .child(
            div()
                .w(px(112.))
                .h(px(72.))
                .p(px(2.))
                .rounded(px(tk::RADIUS_CARD + 2.0))
                .border_2()
                .border_color(if active { theme.accent } else { gpui::transparent_black() })
                .child(
                    // Outline drawn on top so light and dark sketches both get an edge.
                    div()
                        .size_full()
                        .relative()
                        .child(picture)
                        .child(
                            div()
                                .absolute()
                                .inset_0()
                                .rounded(px(r))
                                .border_1()
                                .border_color(theme.text_primary.opacity(0.14)),
                        ),
                ),
        )
        .child(
            div()
                .text_size(px(tk::TEXT_BODY))
                .text_color(if active { theme.text_primary } else { theme.text_muted })
                .when(active, |el| el.font_weight(FontWeight::MEDIUM))
                .child(label),
        )
}

fn render_mode_pill<F>(
    label: &'static str,
    active: bool,
    theme: &ThemeColors,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    render_segment(SharedString::from(label), label, active, theme, handler)
}
