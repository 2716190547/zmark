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
        }
    }
}

impl Render for SettingsWindowView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_sys_dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let current_tab = self.current_tab;
        let current_mode = self.preferences.theme;
        let current_palette = self.preferences.palette.clone();
        let current_locale = self.preferences.locale;
        let auto_save = self.preferences.auto_save;
        let auto_save_on_blur = self.preferences.auto_save_on_blur;
        let tab_size = self.preferences.tab_size;
        let theme = self.theme.clone();
        let strings = self.strings;

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

        div()
            .size_full()
            .bg(theme.bg)
            .text_color(theme.text_primary)
            .flex()
            .flex_col()
            .overflow_hidden()
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
                                    current_mode,
                                    &current_palette,
                                    &theme,
                                    strings,
                                    on_set_mode,
                                    on_set_palette,
                                )
                                .into_any_element(),
                                SettingsTab::EditorSave => render_editor_save_tab(
                                    auto_save,
                                    auto_save_on_blur,
                                    tab_size,
                                    &theme,
                                    strings,
                                    on_toggle_auto_save,
                                    on_toggle_auto_save_on_blur,
                                    on_set_tab_size,
                                )
                                .into_any_element(),
                                SettingsTab::Language => render_language_tab(
                                    current_locale,
                                    &theme,
                                    on_set_locale,
                                )
                                .into_any_element(),
                                SettingsTab::About => render_about_tab(
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
        .rounded_md()
        .cursor_pointer()
        .flex()
        .items_center()
        .gap_2p5()
        .text_xs()
        .on_mouse_down(MouseButton::Left, handler);

    let color = if active { theme.accent } else { theme.text_muted };
    let icon = svg().path(icon_path).size(px(14.)).text_color(color);

    if active {
        base.bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.accent)
            .child(icon)
            .child(title)
    } else {
        base.text_color(theme.text_muted)
            .hover(|s| s.bg(theme.surface).text_color(theme.text_primary))
            .child(icon)
            .child(title)
    }
}

fn render_appearance_tab(
    current_mode: ThemeMode,
    current_palette: &str,
    theme: &ThemeColors,
    strings: &I18nStrings,
    on_set_mode: Arc<dyn Fn(ThemeMode, &mut Window, &mut App)>,
    on_set_palette: Arc<dyn Fn(&'static str, &mut Window, &mut App)>,
) -> impl IntoElement {
    let m1 = on_set_mode.clone();
    let m2 = on_set_mode.clone();
    let m3 = on_set_mode.clone();

    div()
        .flex()
        .flex_col()
        .gap_5()
        // Section 1: Appearance Mode
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
        // Section 2: 8 Curated Palettes
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
                        .flex_wrap()
                        .gap_2()
                        .children(ALL_PALETTES.iter().map(|desc| {
                            let is_active = current_palette == desc.id;
                            let palette_id = desc.id;
                            let on_set = on_set_palette.clone();

                            div()
                                .w(px(165.))
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .border_1()
                                .border_color(if is_active { theme.accent } else { theme.border })
                                .bg(if is_active { theme.bg } else { theme.surface })
                                .cursor_pointer()
                                .hover(|s| s.border_color(theme.accent))
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
                                        .font_weight(if is_active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                                        .text_color(if is_active { theme.accent } else { theme.text_primary })
                                        .child(glidemark_core::i18n::localize_palette_name(desc.id, strings).to_string()),
                                )
                        })),
                ),
        )
}

fn render_editor_save_tab(
    auto_save: bool,
    auto_save_on_blur: bool,
    tab_size: usize,
    theme: &ThemeColors,
    strings: &I18nStrings,
    on_toggle_auto_save: Arc<dyn Fn(bool, &mut Window, &mut App)>,
    on_toggle_auto_save_on_blur: Arc<dyn Fn(bool, &mut Window, &mut App)>,
    on_set_tab_size: Arc<dyn Fn(usize, &mut Window, &mut App)>,
) -> impl IntoElement {
    let t_as = on_toggle_auto_save;
    let t_asb = on_toggle_auto_save_on_blur;
    let t_ts2 = on_set_tab_size.clone();
    let t_ts4 = on_set_tab_size;

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Auto Save Row
        .child(
            div()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .bg(theme.bg)
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
                    div()
                        .px_3()
                        .py_1()
                        .rounded_full()
                        .border_1()
                        .border_color(if auto_save { theme.accent } else { theme.border })
                        .bg(if auto_save { theme.accent } else { theme.surface })
                        .text_color(if auto_save { rgb(0xffffff).into() } else { theme.text_muted })
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            t_as(!auto_save, window, cx);
                        })
                        .child(if auto_save { "ON" } else { "OFF" }),
                ),
        )
        // Auto Save On Blur Row
        .child(
            div()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .bg(theme.bg)
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
                    div()
                        .px_3()
                        .py_1()
                        .rounded_full()
                        .border_1()
                        .border_color(if auto_save_on_blur { theme.accent } else { theme.border })
                        .bg(if auto_save_on_blur { theme.accent } else { theme.surface })
                        .text_color(if auto_save_on_blur { rgb(0xffffff).into() } else { theme.text_muted })
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            t_asb(!auto_save_on_blur, window, cx);
                        })
                        .child(if auto_save_on_blur { "ON" } else { "OFF" }),
                ),
        )
        // Tab Size Row
        .child(
            div()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .bg(theme.bg)
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
                        .child(
                            div()
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(if tab_size == 2 { theme.accent } else { theme.border })
                                .bg(if tab_size == 2 { theme.accent } else { theme.surface })
                                .text_color(if tab_size == 2 { rgb(0xffffff).into() } else { theme.text_muted })
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    t_ts2(2, window, cx);
                                })
                                .child("2 Spaces"),
                        )
                        .child(
                            div()
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(if tab_size == 4 { theme.accent } else { theme.border })
                                .bg(if tab_size == 4 { theme.accent } else { theme.surface })
                                .text_color(if tab_size == 4 { rgb(0xffffff).into() } else { theme.text_muted })
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    t_ts4(4, window, cx);
                                })
                                .child("4 Spaces"),
                        ),
                ),
        )
}

fn render_language_tab(
    current_locale: SupportedLocale,
    theme: &ThemeColors,
    on_set_locale: Arc<dyn Fn(SupportedLocale, &mut Window, &mut App)>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(SupportedLocale::all().iter().map(|&(loc, _tag, name)| {
                    let is_active = current_locale.resolve() == loc;
                    let on_set = on_set_locale.clone();

                    div()
                        .w(px(165.))
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .border_1()
                        .border_color(if is_active { theme.accent } else { theme.border })
                        .bg(if is_active { theme.accent } else { theme.bg })
                        .cursor_pointer()
                        .hover(|s| s.border_color(theme.accent))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            on_set(loc, window, cx);
                        })
                        .child(
                            div()
                                .text_xs()
                                .font_weight(if is_active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                                .text_color(if is_active { rgb(0xffffff).into() } else { theme.text_primary })
                                .child(name),
                        )
                })),
        )
}

fn render_about_tab(
    theme: &ThemeColors,
    strings: &I18nStrings,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        // Header
        .child(
            div()
                .p_4()
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
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
        // Shortcuts list
        .child(
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
                .child(render_shortcut_row("⌘O", strings.open_file, theme))
                .child(render_shortcut_row("⌘S", strings.save_file, theme))
                .child(render_shortcut_row("⌘E", strings.export_html, theme))
                .child(render_shortcut_row("⌘B", strings.directory_drawer, theme))
                .child(render_shortcut_row("⌘1 / ⌘2 / ⌘3", format!("{} / {} / {}", strings.source_mode, strings.split_mode, strings.preview_mode), theme))
                .child(render_shortcut_row("⌘,", strings.settings, theme)),
        )
}

fn render_shortcut_row(
    keys: impl Into<String>,
    desc: impl Into<String>,
    theme: &ThemeColors,
) -> impl IntoElement {
    div()
        .px_3()
        .py_1p5()
        .rounded_md()
        .bg(theme.bg)
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
                .rounded_sm()
                .bg(theme.surface)
                .border_1()
                .border_color(theme.border)
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
        .rounded_md()
        .border_1()
        .text_xs()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, handler);

    if active {
        base.bg(theme.accent)
            .border_color(theme.accent)
            .text_color(rgb(0xffffff))
            .font_weight(FontWeight::SEMIBOLD)
            .child(label)
    } else {
        base.bg(theme.bg)
            .border_color(theme.border)
            .text_color(theme.text_primary)
            .hover(|s| s.border_color(theme.accent))
            .child(label)
    }
}
