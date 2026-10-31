use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use glidemark_core::config::{Preferences, ThemeMode};
use glidemark_core::i18n::{I18nStrings, SupportedLocale};

use crate::state::SettingsTab;
use crate::theme::{ThemeColors, ALL_PALETTES};
use crate::views::workspace::WorkspaceView;

pub struct SettingsWindowView {
    pub workspace: WeakEntity<WorkspaceView>,
    pub current_tab: SettingsTab,
    pub preferences: Preferences,
    pub theme: ThemeColors,
    pub strings: &'static I18nStrings,
    pub search_query: String,
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
                cx.notify();
            })
            .detach();
        }
        Self {
            workspace,
            current_tab,
            preferences,
            theme,
            strings,
            search_query: String::new(),
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
        let search_query = self.search_query.clone();

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
                this.strings = glidemark_core::i18n::get_strings(locale);
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
            .bg(theme.bg)
            .text_color(theme.text_primary)
            .flex()
            .flex_col()
            .overflow_hidden()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                match event.keystroke.key.as_str() {
                    "backspace" => {
                        if this.search_query.pop().is_some() {
                            cx.notify();
                        }
                    }
                    "escape" => {
                        if !this.search_query.is_empty() {
                            this.search_query.clear();
                            cx.notify();
                        }
                    }
                    "space" => {
                        this.search_query.push(' ');
                        cx.notify();
                    }
                    key => {
                        if !event.keystroke.modifiers.control && !event.keystroke.modifiers.platform {
                            if let Some(ch) = &event.keystroke.key_char {
                                this.search_query.push_str(ch);
                                cx.notify();
                            } else if key.chars().count() == 1 {
                                this.search_query.push_str(key);
                                cx.notify();
                            }
                        }
                    }
                }
            }))
            // Top Window Title Bar with traffic-lights safe insets
            .child(
                div()
                    .h(px(44.))
                    .pl(px(80.))
                    .pr_4()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.bg)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child(strings.settings),
                    ),
            )
            // Two-column Split Body
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    // Left Sidebar Navigation
                    .child(
                        div()
                            .w(px(160.))
                            .h_full()
                            .border_r_1()
                            .border_color(theme.border)
                            .bg(theme.bg)
                            .p_2()
                            .flex()
                            .flex_col()
                            .gap_1()
                            // Top Search Bar
                            .child({
                                let view_search = cx.entity().clone();
                                div()
                                    .w_full()
                                    .rounded_lg()
                                    .px_2p5()
                                    .py_1p5()
                                    .bg(theme.surface)
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        svg()
                                            .path("icons/search.svg")
                                            .size(px(14.))
                                            .text_color(theme.text_muted),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_xs()
                                            .overflow_hidden()
                                            .when(self.search_query.is_empty(), |s| {
                                                s.text_color(theme.text_muted)
                                                    .child(strings.search_settings)
                                            })
                                            .when(!self.search_query.is_empty(), |s| {
                                                s.text_color(theme.text_primary)
                                                    .child(self.search_query.clone())
                                            }),
                                    )
                                    .when(!self.search_query.is_empty(), |s| {
                                        s.child(
                                            div()
                                                .cursor_pointer()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .hover(|h| h.text_color(theme.text_primary))
                                                .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                                                    view_search.update(cx, |this, cx| {
                                                        this.search_query.clear();
                                                        cx.notify();
                                                    });
                                                })
                                                .child("×"),
                                        )
                                    })
                            })
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
                            .bg(theme.surface)
                            .overflow_y_scroll()
                            .child(match current_tab {
                                SettingsTab::Appearance => render_appearance_tab(
                                    &search_query,
                                    current_mode,
                                    &current_palette,
                                    &theme,
                                    strings,
                                    on_set_mode,
                                    on_set_palette,
                                )
                                .into_any_element(),
                                SettingsTab::EditorSave => render_editor_save_tab(
                                    &search_query,
                                    auto_save,
                                    auto_save_on_blur,
                                    tab_size,
                                    line_height,
                                    &theme,
                                    strings,
                                    on_toggle_auto_save,
                                    on_toggle_auto_save_on_blur,
                                    on_set_tab_size,
                                    on_set_line_height,
                                )
                                .into_any_element(),
                                SettingsTab::Language => render_language_tab(
                                    &search_query,
                                    current_locale,
                                    &theme,
                                    on_set_locale,
                                )
                                .into_any_element(),
                                SettingsTab::About => render_about_tab(
                                    &search_query,
                                    &theme,
                                    strings,
                                )
                                .into_any_element(),
                            }),
                    ),
            )
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
        theme.warm_pill_text()
    } else {
        theme.text_muted
    };
    let icon = svg().path(icon_path).size(px(18.)).text_color(icon_color);

    if active {
        base.bg(theme.warm_pill_bg())
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.warm_pill_text())
            .child(icon)
            .child(title)
    } else {
        base.text_color(theme.text_muted)
            .hover(|s| s.bg(theme.warm_pill_hover()).text_color(theme.text_primary))
            .child(icon)
            .child(title)
    }
}

fn render_empty_state(search_query: &str, theme: &ThemeColors) -> impl IntoElement {
    div()
        .p_8()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .child(
            svg()
                .path("icons/search.svg")
                .size(px(24.))
                .text_color(theme.text_muted),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .child(format!("No results for \"{}\"", search_query)),
        )
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
        theme.warm_pill_bg()
    };
    div()
        .w(px(36.))
        .h(px(20.))
        .p(px(2.))
        .rounded_full()
        .bg(track_bg)
        .text_color(theme.warm_pill_text())
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
    search_query: &str,
    current_mode: ThemeMode,
    current_palette: &str,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    on_set_mode: Arc<dyn Fn(ThemeMode, &mut Window, &mut App)>,
    on_set_palette: Arc<dyn Fn(&'static str, &mut Window, &mut App)>,
) -> impl IntoElement {
    let q = search_query.trim().to_lowercase();
    let m1 = on_set_mode.clone();
    let m2 = on_set_mode.clone();
    let m3 = on_set_mode.clone();

    let show_mode = q.is_empty()
        || strings.appearance.to_lowercase().contains(&q)
        || strings.theme_auto.to_lowercase().contains(&q)
        || strings.theme_light.to_lowercase().contains(&q)
        || strings.theme_dark.to_lowercase().contains(&q);

    let filtered_palettes: Vec<_> = ALL_PALETTES
        .iter()
        .filter(|desc| {
            q.is_empty()
                || desc.id.to_lowercase().contains(&q)
                || desc.name.to_lowercase().contains(&q)
                || glidemark_core::i18n::localize_palette_name(desc.id, strings)
                    .to_lowercase()
                    .contains(&q)
                || strings.palette.to_lowercase().contains(&q)
        })
        .collect();

    let show_palette = !filtered_palettes.is_empty();

    if !show_mode && !show_palette {
        return render_empty_state(search_query, theme).into_any_element();
    }

    div()
        .flex()
        .flex_col()
        .gap_5()
        .when(show_mode, |s| {
            s.child(
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
                            .rounded_2xl()
                            .bg(theme.bg)
                            .p_4()
                            .flex()
                            .gap_2()
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
        })
        .when(show_palette, |s| {
            s.child(
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
                            .rounded_2xl()
                            .bg(theme.bg)
                            .p_4()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(filtered_palettes.into_iter().map(|desc| {
                                let is_active = current_palette == desc.id;
                                let palette_id = desc.id;
                                let on_set = on_set_palette.clone();

                                let card = div()
                                    .w(px(165.))
                                    .px_3()
                                    .py_2()
                                    .rounded_lg()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                        on_set(palette_id, window, cx);
                                    })
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .w(px(12.))
                                            .h(px(12.))
                                            .rounded_full()
                                            .bg(desc.preview_color),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .child(glidemark_core::i18n::localize_palette_name(desc.id, strings)),
                                    );

                                if is_active {
                                    card.bg(theme.warm_pill_bg())
                                        .text_color(theme.warm_pill_text())
                                        .font_weight(FontWeight::MEDIUM)
                                } else {
                                    card.bg(theme.surface)
                                        .text_color(theme.text_primary)
                                        .hover(|s| s.bg(theme.warm_pill_hover()))
                                }
                            })),
                    ),
            )
        })
        .into_any_element()
}

fn render_editor_save_tab(
    search_query: &str,
    auto_save: bool,
    auto_save_on_blur: bool,
    tab_size: usize,
    line_height: f32,
    theme: &ThemeColors,
    strings: &I18nStrings,
    on_toggle_auto_save: Arc<dyn Fn(bool, &mut Window, &mut App)>,
    on_toggle_auto_save_on_blur: Arc<dyn Fn(bool, &mut Window, &mut App)>,
    on_set_tab_size: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_set_line_height: Arc<dyn Fn(f32, &mut Window, &mut App)>,
) -> impl IntoElement {
    let q = search_query.trim().to_lowercase();
    let t_as = on_toggle_auto_save;
    let t_asb = on_toggle_auto_save_on_blur;
    let t_ts2 = on_set_tab_size.clone();
    let t_ts4 = on_set_tab_size;

    let show_auto_save = q.is_empty()
        || strings.auto_save_label.to_lowercase().contains(&q) // warm_pill_bg warm_pill_text
        || strings.auto_save_desc.to_lowercase().contains(&q);

    let show_auto_save_on_blur = q.is_empty()
        || strings.auto_save_on_blur_label.to_lowercase().contains(&q);

    let show_tab_size = q.is_empty()
        || strings.tab_size_label.to_lowercase().contains(&q) // warm_pill_bg warm_pill_text
        || "tab".contains(&q)
        || "spaces".contains(&q);

    let show_line_height = q.is_empty()
        || strings.line_height_label.to_lowercase().contains(&q)
        || "line".contains(&q)
        || "height".contains(&q);

    let has_save_card = show_auto_save || show_auto_save_on_blur;
    let has_editor_card = show_tab_size || show_line_height;

    if !has_save_card && !has_editor_card {
        return render_empty_state(search_query, theme).into_any_element();
    }

    div()
        .flex()
        .flex_col()
        .gap_4()
        .when(has_save_card, |container| {
            let mut card = div()
                .rounded_2xl()
                .bg(theme.bg)
                .p_4()
                .flex()
                .flex_col();

            let mut has_prev_item = false;

            if show_auto_save {
                card = card.child(
                    div()
                        .py_2()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text_primary)
                                        .child(strings.auto_save_label),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child(strings.auto_save_desc),
                                ),
                        )
                        .child(
                            // iOS toggle switch styled with warm_pill_bg and warm_pill_text
                            render_ios_toggle(auto_save, theme, move |_, window, cx| {
                                t_as(!auto_save, window, cx);
                            }),
                        ),
                );
                has_prev_item = true;
            }

            if show_auto_save_on_blur {
                if has_prev_item {
                    card = card.child(
                        div()
                            .h(px(1.))
                            .bg(theme.border)
                            .w_full(),
                    );
                }
                card = card.child(
                    div()
                        .py_2()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text_primary)
                                .child(strings.auto_save_on_blur_label),
                        )
                        .child(
                            render_ios_toggle(auto_save_on_blur, theme, move |_, window, cx| {
                                t_asb(!auto_save_on_blur, window, cx);
                            }),
                        ),
                );
            }

            container.child(card)
        })
        .when(has_editor_card, |container| {
            let mut card = div()
                .rounded_2xl()
                .bg(theme.bg)
                .p_4()
                .flex()
                .flex_col();

            let mut has_prev_item = false;

            if show_tab_size {
                card = card.child(
                    div()
                        .py_2()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text_primary)
                                .child(strings.tab_size_label),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child({
                                    let active = tab_size == 2;
                                    let pill = div()
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                            t_ts2(2, window, cx);
                                        })
                                        .child("2 Spaces");

                                    if active {
                                        pill.bg(theme.warm_pill_bg())
                                            .text_color(theme.warm_pill_text())
                                            .font_weight(FontWeight::SEMIBOLD)
                                    } else {
                                        pill.bg(theme.surface)
                                            .text_color(theme.text_muted)
                                            .hover(|s| s.bg(theme.warm_pill_hover()).text_color(theme.text_primary))
                                    }
                                })
                                .child({
                                    let active = tab_size == 4;
                                    let pill = div()
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                            t_ts4(4, window, cx);
                                        })
                                        .child("4 Spaces");

                                    if active {
                                        pill.bg(theme.warm_pill_bg())
                                            .text_color(theme.warm_pill_text())
                                            .font_weight(FontWeight::SEMIBOLD)
                                    } else {
                                        pill.bg(theme.surface)
                                            .text_color(theme.text_muted)
                                            .hover(|s| s.bg(theme.warm_pill_hover()).text_color(theme.text_primary))
                                    }
                                }),
                        ),
                );
                has_prev_item = true;
            }

            if show_line_height {
                if has_prev_item {
                    card = card.child(
                        div()
                            .h(px(1.))
                            .bg(theme.border)
                            .w_full(),
                    );
                }
                card = card.child(
                    div()
                        .py_2()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text_primary)
                                .child(strings.line_height_label),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .children([1.3f32, 1.4, 1.5, 1.6, 1.7].into_iter().map(|lh_opt| {
                                    let is_active = (line_height - lh_opt).abs() < 0.05;
                                    let on_set = on_set_line_height.clone();
                                    let pill = div()
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                            on_set(lh_opt, window, cx);
                                        })
                                        .child(format!("{:.1}x", lh_opt));

                                    if is_active {
                                        pill.bg(theme.warm_pill_bg())
                                            .text_color(theme.warm_pill_text())
                                            .font_weight(FontWeight::SEMIBOLD)
                                    } else {
                                        pill.bg(theme.surface)
                                            .text_color(theme.text_muted)
                                            .hover(|s| s.bg(theme.warm_pill_hover()).text_color(theme.text_primary))
                                    }
                                })),
                        ),
                );
            }

            container.child(card)
        })
        .into_any_element()
}

fn render_language_tab(
    search_query: &str,
    current_locale: SupportedLocale,
    theme: &ThemeColors,
    on_set_locale: Arc<dyn Fn(SupportedLocale, &mut Window, &mut App)>,
) -> impl IntoElement {
    let q = search_query.trim().to_lowercase();
    let filtered_locales: Vec<_> = SupportedLocale::all()
        .iter()
        .filter(|&&(_loc, tag, name)| {
            q.is_empty()
                || name.to_lowercase().contains(&q)
                || tag.to_lowercase().contains(&q)
        })
        .copied()
        .collect();

    if filtered_locales.is_empty() {
        return render_empty_state(search_query, theme).into_any_element();
    }

    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .rounded_2xl()
                .bg(theme.bg)
                .p_4()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(filtered_locales.into_iter().map(|(loc, _tag, name)| {
                    let is_active = current_locale.resolve() == loc;
                    let on_set = on_set_locale.clone();

                    let card = div()
                        .w(px(165.))
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            on_set(loc, window, cx);
                        })
                        .child(
                            div()
                                .text_xs()
                                .child(name),
                        );

                    if is_active {
                        card.bg(theme.warm_pill_bg())
                            .text_color(theme.warm_pill_text())
                            .font_weight(FontWeight::MEDIUM)
                    } else {
                        card.bg(theme.surface)
                            .text_color(theme.text_primary)
                            .hover(|s| s.bg(theme.warm_pill_hover()))
                    }
                })),
        )
        .into_any_element()
}

fn render_about_tab(
    search_query: &str,
    theme: &ThemeColors,
    strings: &I18nStrings,
) -> impl IntoElement {
    let q = search_query.trim().to_lowercase();

    let show_header = q.is_empty()
        || "glidemark".contains(&q)
        || "version".contains(&q)
        || "markdown".contains(&q)
        || strings.about.to_lowercase().contains(&q);

    let shortcuts: [(&'static str, String); 6] = [
        ("⌘O", strings.open_file.to_string()),
        ("⌘S", strings.save_file.to_string()),
        ("⌘E", strings.export_html.to_string()),
        ("⌘B", strings.directory_drawer.to_string()),
        (
            "⌘1 / ⌘2 / ⌘3",
            format!(
                "{} / {} / {}",
                strings.source_mode, strings.split_mode, strings.preview_mode
            ),
        ),
        ("⌘,", strings.settings.to_string()),
    ];

    let filtered_shortcuts: Vec<_> = shortcuts
        .into_iter()
        .filter(|(keys, desc)| {
            q.is_empty()
                || keys.to_lowercase().contains(&q)
                || desc.to_lowercase().contains(&q)
                || strings.shortcuts_label.to_lowercase().contains(&q)
        })
        .collect();

    let show_shortcuts = !filtered_shortcuts.is_empty();

    if !show_header && !show_shortcuts {
        return render_empty_state(search_query, theme).into_any_element();
    }

    div()
        .flex()
        .flex_col()
        .gap_4()
        .when(show_header, |s| {
            s.child(
                div()
                    .p_4()
                    .rounded_2xl()
                    .bg(theme.bg)
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child("GlideMark"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child("GPU-Accelerated Markdown Editor & Reader · Rust + GPUI"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.accent)
                            .child("Version 0.1.0"),
                    ),
            )
        })
        .when(show_shortcuts, |s| {
            let mut card = div()
                .p_4()
                .rounded_2xl()
                .bg(theme.bg)
                .flex()
                .flex_col();

            let count = filtered_shortcuts.len();
            for (i, (keys, desc)) in filtered_shortcuts.into_iter().enumerate() {
                card = card.child(render_shortcut_row(keys, desc, theme));
                if i + 1 < count {
                    card = card.child(
                        div()
                            .h(px(1.))
                            .bg(theme.border)
                            .w_full(),
                    );
                }
            }

            s.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_muted)
                            .child(strings.shortcuts_label),
                    )
                    .child(card),
            )
        })
        .into_any_element()
}

fn render_shortcut_row(
    keys: impl Into<String>,
    desc: impl Into<String>,
    theme: &ThemeColors,
) -> impl IntoElement {
    div()
        .py_2()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .child(desc.into()),
        )
        .child(
            div()
                .px_1p5()
                .py_0p5()
                .rounded(px(4.))
                .bg(theme.surface)
                .text_xs()
                .font_family("ui-monospace, monospace")
                .text_color(theme.text_primary)
                .child(keys.into()),
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
        .rounded_lg()
        .text_xs()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, handler);

    if active {
        base.bg(theme.warm_pill_bg())
            .text_color(theme.warm_pill_text())
            .font_weight(FontWeight::MEDIUM)
            .child(label)
    } else {
        base.bg(theme.surface)
            .text_color(theme.text_muted)
            .hover(|s| s.bg(theme.warm_pill_hover()).text_color(theme.text_primary))
            .child(label)
    }
}
