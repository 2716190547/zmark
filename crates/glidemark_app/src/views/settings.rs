use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use glidemark_core::config::ThemeMode;
use glidemark_core::i18n::{I18nStrings, SupportedLocale};

use crate::theme::{ThemeColors, ALL_PALETTES};

pub fn render_settings_modal(
    current_mode: ThemeMode,
    current_palette: &str,
    current_locale: SupportedLocale,
    theme: &ThemeColors,
    strings: &I18nStrings,
    on_close: Arc<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>,
    on_set_mode: Arc<dyn Fn(ThemeMode, &mut Window, &mut App)>,
    on_set_palette: Arc<dyn Fn(&'static str, &mut Window, &mut App)>,
    on_set_locale: Arc<dyn Fn(SupportedLocale, &mut Window, &mut App)>,
) -> impl IntoElement {
    let on_close_click = on_close.clone();

    // Backdrop
    div()
        .absolute()
        .inset_0()
        .bg(hsla(0.0, 0.0, 0.0, 0.5))
        .flex()
        .items_center()
        .justify_center()
        .child(
            // Dialog box
            div()
                .w(px(540.))
                .max_h(px(640.))
                .bg(theme.surface)
                .border_1()
                .border_color(theme.border)
                .rounded_xl()
                .p_6()
                .flex()
                .flex_col()
                .gap_5()
                .child(
                    // Header
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child(strings.settings),
                        )
                        .child(
                            div()
                                .w(px(28.))
                                .h(px(28.))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(theme.text_muted)
                                .hover(|s| s.bg(theme.bg).text_color(theme.text_primary))
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |e, window, cx| {
                                    on_close_click(e, window, cx);
                                })
                                .child("✕"),
                        ),
                )
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
                        .child({
                            let m1 = on_set_mode.clone();
                            let m2 = on_set_mode.clone();
                            let m3 = on_set_mode.clone();
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
                                ))
                        }),
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
                                        .w(px(120.))
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
                                                .child(desc.name),
                                        )
                                })),
                        ),
                )
                // Section 3: 10 Supported Languages
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
                                .child(strings.language),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_2()
                                .children(SupportedLocale::all().iter().map(|&(loc, _tag, name)| {
                                    let is_active = current_locale.resolve() == loc;
                                    let on_set = on_set_locale.clone();

                                    div()
                                        .px_3()
                                        .py_1p5()
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
                        ),
                )
                // Footer Hint
                .child(
                    div()
                        .pt_2()
                        .border_t_1()
                        .border_color(theme.border)
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("GlideMark · Rust + GPUI 120fps"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("Press Esc or ✕ to close"),
                        ),
                ),
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
