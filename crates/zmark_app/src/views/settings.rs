use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use zmark_core::config::{Preferences, ThemeMode};
use zmark_core::i18n::{I18nStrings, SupportedLocale};

use crate::state::SettingsTab;
use crate::theme::{ThemeColors, ALL_PALETTES};
use crate::updater::UpdateStatus;
use crate::views::workspace::WorkspaceView;

pub struct SettingsWindowView {
    pub workspace: WeakEntity<WorkspaceView>,
    pub current_tab: SettingsTab,
    pub preferences: Preferences,
    pub theme: ThemeColors,
    pub strings: &'static I18nStrings,
    pub update_status: UpdateStatus,
    pub update_enabled: bool,
    pub update_reason: Option<String>,
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
        if self.theme.is_dark != effective_dark {
            self.theme = ThemeColors::resolve(&self.preferences.palette, effective_dark);
        }
        let current_tab = self.current_tab;
        let current_mode = self.preferences.theme;
        let current_palette = self.preferences.palette.clone();
        let current_locale = self.preferences.locale;
        let auto_save = self.preferences.auto_save;
        let auto_save_on_blur = self.preferences.auto_save_on_blur;
        let tab_size = self.preferences.tab_size;
        let line_height = self.preferences.line_height;
        let theme = self.theme.clone();
        let strings = self.strings;
        let update_status = self.update_status.clone();
        let update_enabled = self.update_enabled;
        let update_reason = self.update_reason.clone();
        let workspace_for_update = self.workspace.clone();
        let on_check_update = move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
            if let Some(ws) = workspace_for_update.upgrade() {
                ws.update(cx, |ws, cx| ws.check_for_updates(cx, true));
            }
        };

        let view_tab = cx.entity().clone();
        let on_set_tab = Arc::new(move |tab: SettingsTab, _window: &mut Window, cx: &mut App| {
            view_tab.update(cx, |this, cx| {
                this.current_tab = tab;
                cx.notify();
            });
        });

        let view_mode = cx.entity().clone();
        let on_set_mode = Arc::new(move |mode: ThemeMode, _window: &mut Window, cx: &mut App| {
            view_mode.update(cx, |this, cx| {
                this.preferences.theme = mode;
                let dark = match mode {
                    ThemeMode::Dark => true,
                    ThemeMode::Light => false,
                    ThemeMode::Auto => is_sys_dark,
                };
                this.theme = ThemeColors::resolve(&this.preferences.palette, dark);
                let _ = this.preferences.save();
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
                let dark = match this.preferences.theme {
                    ThemeMode::Dark => true,
                    ThemeMode::Light => false,
                    ThemeMode::Auto => is_sys_dark,
                };
                this.theme = ThemeColors::resolve(palette, dark);
                let _ = this.preferences.save();
                if let Some(ws) = this.workspace.upgrade() {
                    ws.update(cx, |ws, cx| {
                        ws.state.set_palette(palette);
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
                let _ = this.preferences.save();
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
                let _ = this.preferences.save();
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
                let _ = this.preferences.save();
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
                let _ = this.preferences.save();
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
                let _ = this.preferences.save();
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
                let _ = this.preferences.save();
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
                let _ = this.preferences.save();
                if let Some(ws) = this.workspace.upgrade() {
                    ws.update(cx, |ws, cx| {
                        ws.state.set_line_height(clamped);
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
            // Left Sidebar Navigation (Edge-to-Edge full height with traffic-lights safe insets)
            .child(
                div()
                    .w(px(160.))
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
                    .p_5()
                    .when(cfg!(target_os = "windows"), |el| el.pt(px(36.)))
                    .bg(theme.surface)
                    .overflow_y_scroll()
                    .child(match current_tab {
                        SettingsTab::Appearance => render_appearance_tab(
                            current_mode,
                            &current_palette,
                            effective_dark,
                            self.preferences.reduce_motion,
                            &theme,
                            strings,
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
                            &theme,
                            strings,
                            on_toggle_auto_save,
                            on_toggle_auto_save_on_blur,
                            on_toggle_line_numbers,
                            on_set_tab_size,
                            on_set_line_height,
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
                            render_shortcuts_tab(&theme, strings).into_any_element()
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
        .px_3()
        .py_2()
        .rounded_lg()
        .cursor_pointer()
        .flex()
        .items_center()
        .gap_2p5()
        .text_xs()
        .on_mouse_down(MouseButton::Left, handler);

    let icon_color = if active {
        theme.selection_text()
    } else {
        theme.text_muted
    };
    let icon = svg().path(icon_path).size(px(18.)).text_color(icon_color);

    if active {
        base.bg(theme.selection_bg())
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.selection_text())
            .child(icon)
            .child(title)
    } else {
        base.text_color(theme.text_muted)
            .hover(|s| s.bg(theme.hover_bg()).text_color(theme.text_primary))
            .child(icon)
            .child(title)
    }
}

/// Section heading shared by every settings page.
fn render_section_title(text: impl Into<SharedString>, theme: &ThemeColors) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
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
        .py_2()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .min_w(px(0.))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_primary)
                        .child(label.into()),
                )
                .when_some(desc, |el, desc| {
                    el.child(div().text_xs().text_color(theme.text_muted).child(desc))
                }),
        )
        .child(div().flex_shrink_0().child(trailing))
}

/// Secondary push button used for in-page actions (About -> check for updates).
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
        .h(px(26.))
        .px(px(12.))
        .rounded(px(6.))
        .flex()
        .items_center()
        .bg(theme.text_primary.opacity(0.05))
        .border(px(1.))
        .border_color(theme.text_primary.opacity(0.12))
        .text_size(px(12.))
        .text_color(theme.text_primary)
        .cursor_pointer()
        .hover(|s| s.bg(theme.text_primary.opacity(0.08)))
        .on_mouse_down(MouseButton::Left, handler)
        .child(label.into())
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
        theme.selection_bg()
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

fn render_appearance_tab(
    current_mode: ThemeMode,
    current_palette: &str,
    effective_dark: bool,
    reduce_motion: bool,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    on_set_mode: Arc<dyn Fn(ThemeMode, &mut Window, &mut App)>,
    on_set_palette: Arc<dyn Fn(&'static str, &mut Window, &mut App)>,
    on_toggle_reduce_motion: Arc<dyn Fn(bool, &mut Window, &mut App)>,
) -> impl IntoElement {
    let m1 = on_set_mode.clone();
    let m2 = on_set_mode.clone();
    let m3 = on_set_mode;

    div()
        .flex()
        .flex_col()
        .gap_5()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_muted)
                        .child(strings.appearance),
                )
                .child(
                    div()
                        .w_full()
                        .bg(theme.segmented_track_bg())
                        .rounded_lg()
                        .p(px(2.))
                        .flex()
                        .gap_1()
                        .child(render_mode_pill(
                            strings.theme_auto,
                            current_mode == ThemeMode::Auto,
                            theme,
                            move |_, window, cx| m1(ThemeMode::Auto, window, cx),
                        ))
                        .child(render_mode_pill(
                            strings.theme_light,
                            current_mode == ThemeMode::Light,
                            theme,
                            move |_, window, cx| m2(ThemeMode::Light, window, cx),
                        ))
                        .child(render_mode_pill(
                            strings.theme_dark,
                            current_mode == ThemeMode::Dark,
                            theme,
                            move |_, window, cx| m3(ThemeMode::Dark, window, cx),
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_muted)
                        .child(strings.palette),
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
                                        .rounded_md()
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
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(resolved.text_primary)
                                                .child(zmark_core::i18n::localize_palette_name(desc.id, strings)),
                                        )
                                        .when(is_active, |c| c.border_2().border_color(theme.accent))
                                        .when(!is_active, |c| c.border_2().border_color(gpui::transparent_black()))
                                }))
                        })),
                ),
        )
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
    theme: &ThemeColors,
    strings: &I18nStrings,
    on_toggle_auto_save: Arc<dyn Fn(bool, &mut Window, &mut App)>,
    on_toggle_auto_save_on_blur: Arc<dyn Fn(bool, &mut Window, &mut App)>,
    on_toggle_line_numbers: Arc<dyn Fn(bool, &mut Window, &mut App)>,
    on_set_tab_size: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_set_line_height: Arc<dyn Fn(f32, &mut Window, &mut App)>,
) -> impl IntoElement {
    let t_as = on_toggle_auto_save;
    let t_asb = on_toggle_auto_save_on_blur;
    let t_ln = on_toggle_line_numbers;
    let t_ts2 = on_set_tab_size.clone();
    let t_ts4 = on_set_tab_size;

    div()
        .flex()
        .flex_col()
        .gap_5()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_setting_row(
                    strings.auto_save_label,
                    Some(strings.auto_save_desc.into()),
                    render_ios_toggle(auto_save, theme, move |_, window, cx| {
                        t_as(!auto_save, window, cx);
                    }),
                    theme,
                ))
                .child(render_setting_row(
                    strings.auto_save_on_blur_label,
                    None,
                    render_ios_toggle(auto_save_on_blur, theme, move |_, window, cx| {
                        t_asb(!auto_save_on_blur, window, cx);
                    }),
                    theme,
                ))
                .child(render_setting_row(
                    strings.show_line_numbers_label,
                    Some(strings.show_line_numbers_desc.into()),
                    render_ios_toggle(show_line_numbers, theme, move |_, window, cx| {
                        t_ln(!show_line_numbers, window, cx);
                    }),
                    theme,
                )),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_muted)
                        .child(strings.tab_size_label),
                )
                .child(
                    div()
                        .w_full()
                        .bg(theme.segmented_track_bg())
                        .rounded_lg()
                        .p(px(2.))
                        .flex()
                        .gap_1()
                        .child({
                            let active = tab_size == 2;
                            let pill = div()
                                .flex_1()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    t_ts2(2, window, cx);
                                })
                                .child("2 Spaces");

                            if active {
                                pill.bg(theme.segmented_thumb_bg())
                                    .border(px(1.))
                                    .border_color(theme.segmented_thumb_border())
                                    .text_color(theme.segmented_thumb_text())
                                    .font_weight(FontWeight::MEDIUM)
                            } else {
                                pill.bg(gpui::transparent_black())
                                    .text_color(theme.segmented_inactive_text())
                                    .hover(|s| s.text_color(theme.text_primary))
                            }
                        })
                        .child({
                            let active = tab_size == 4;
                            let pill = div()
                                .flex_1()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    t_ts4(4, window, cx);
                                })
                                .child("4 Spaces");

                            if active {
                                pill.bg(theme.segmented_thumb_bg())
                                    .border(px(1.))
                                    .border_color(theme.segmented_thumb_border())
                                    .text_color(theme.segmented_thumb_text())
                                    .font_weight(FontWeight::MEDIUM)
                            } else {
                                pill.bg(gpui::transparent_black())
                                    .text_color(theme.segmented_inactive_text())
                                    .hover(|s| s.text_color(theme.text_primary))
                            }
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_muted)
                        .child(strings.line_height_label),
                )
                .child(
                    div()
                        .w_full()
                        .bg(theme.segmented_track_bg())
                        .rounded_lg()
                        .p(px(2.))
                        .flex()
                        .gap_1()
                        .children([1.3f32, 1.4, 1.5, 1.6, 1.7].into_iter().map(|lh_opt| {
                            let is_active = (line_height - lh_opt).abs() < 0.05;
                            let on_set = on_set_line_height.clone();
                            let pill = div()
                                .flex_1()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    on_set(lh_opt, window, cx);
                                })
                                .child(format!("{:.1}x", lh_opt));

                            if is_active {
                                pill.bg(theme.segmented_thumb_bg())
                                    .border(px(1.))
                                    .border_color(theme.segmented_thumb_border())
                                    .text_color(theme.segmented_thumb_text())
                                    .font_weight(FontWeight::MEDIUM)
                            } else {
                                pill.bg(gpui::transparent_black())
                                    .text_color(theme.segmented_inactive_text())
                                    .hover(|s| s.text_color(theme.text_primary))
                            }
                        })),
                ),
        )
}

fn render_language_tab(
    current_locale: SupportedLocale,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    on_set_locale: Arc<dyn Fn(SupportedLocale, &mut Window, &mut App)>,
) -> impl IntoElement {
    let locales = SupportedLocale::all();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text_muted)
                .child(strings.language),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .children(locales.chunks(2).map(|pair| {
                    div()
                        .w_full()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .children(pair.iter().map(|&(loc, _tag, name)| {
                            let is_active = current_locale.resolve() == loc;
                            let on_set = on_set_locale.clone();

                            let card = div()
                                .flex_1()
                                .px_3()
                                .py_2p5()
                                .rounded_md()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    on_set(loc, window, cx);
                                })
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .text_xs()
                                        .child(name),
                                );

                            if is_active {
                                card.bg(theme.selection_bg())
                                    .text_color(theme.selection_text())
                                    .font_weight(FontWeight::MEDIUM)
                            } else {
                                card.bg(gpui::transparent_black())
                                    .text_color(theme.text_primary)
                                    .hover(|s| s.bg(theme.hover_bg()))
                            }
                        }))
                })),
        )
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

fn render_shortcuts_tab(theme: &ThemeColors, strings: &I18nStrings) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_5()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_title(strings.shortcuts_label, theme))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .children(shortcut_rows(strings).into_iter().map(|(keys, desc)| {
                            render_shortcut_row(keys, desc, theme)
                        })),
                ),
        )
}

fn render_about_tab(
    theme: &ThemeColors,
    strings: &I18nStrings,
    update_status: &UpdateStatus,
    update_enabled: bool,
    update_reason: Option<&str>,
    on_check_update: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    on_open_github: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
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
        .gap_5()
        // Product header: centered logo, name, tagline.
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .pt_2()
                .child(img("brand/logo.png").w(px(64.)).h(px(64.)).flex_shrink_0())
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text_primary)
                        .child("ZMark"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(strings.about_tagline),
                ),
        )
        // Version + updates share the same row component as the other pages.
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_title(strings.software_updates, theme))
                .child(render_setting_row(
                    strings.current_version_label,
                    None,
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(crate::APP_VERSION),
                    theme,
                ))
                .child(render_setting_row(
                    strings.check_updates,
                    Some(update_text).filter(|text| !text.is_empty()).map(Into::into),
                    render_secondary_button(
                        "btn_check_updates",
                        strings.check_updates,
                        theme,
                        on_check_update,
                    ),
                    theme,
                )),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_title(strings.open_source_label, theme))
                .child(render_setting_row(
                    "github.com/2716190547/zmark",
                    Some(strings.footer_credit.into()),
                    render_secondary_button("btn_open_github", "GitHub", theme, on_open_github),
                    theme,
                )),
        )
}

fn render_shortcut_row(
    keys: impl Into<String>,
    desc: impl Into<String>,
    theme: &ThemeColors,
) -> impl IntoElement {
    let desc_str = desc.into();
    let keys_str = keys.into();
    div()
        .w_full()
        .py_1p5()
        .px_1()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .child(desc_str),
        )
        .child(
            div()
                .px_1p5()
                .py_0p5()
                .rounded(px(4.))
                .bg(theme.selection_bg())
                .text_xs()
                .font_family(crate::platform::MONO_FONT)
                .text_color(theme.text_primary)
                .child(keys_str),
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
    let base = div()
        .flex_1()
        .py_1p5()
        .rounded_md()
        .text_xs()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, handler);

    if active {
        base.bg(theme.segmented_thumb_bg())
            .border(px(1.))
            .border_color(theme.segmented_thumb_border())
            .text_color(theme.segmented_thumb_text())
            .font_weight(FontWeight::MEDIUM)
            .child(label)
    } else {
        base.bg(gpui::transparent_black())
            .text_color(theme.segmented_inactive_text())
            .hover(|s| s.text_color(theme.text_primary))
            .child(label)
    }
}

