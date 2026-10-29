use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

use crate::theme::ThemeColors;
use crate::views::workspace::WorkspaceView;

#[derive(Debug, Clone, PartialEq)]
pub struct LightboxState {
    pub is_open: bool,
    pub svg_path: Option<PathBuf>,
    pub code_hash: Option<u64>,
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub is_dragging: bool,
    pub drag_start_mouse: Option<(f32, f32)>,
    pub drag_start_pan: (f32, f32),
}

impl Default for LightboxState {
    fn default() -> Self {
        Self::new()
    }
}

impl LightboxState {
    pub fn new() -> Self {
        Self {
            is_open: false,
            svg_path: None,
            code_hash: None,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            is_dragging: false,
            drag_start_mouse: None,
            drag_start_pan: (0.0, 0.0),
        }
    }

    pub fn open(&mut self, svg_path: PathBuf, code_hash: u64) {
        self.is_open = true;
        self.svg_path = Some(svg_path);
        self.code_hash = Some(code_hash);
        self.zoom = 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        self.is_dragging = false;
        self.drag_start_mouse = None;
        self.drag_start_pan = (0.0, 0.0);
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.svg_path = None;
        self.code_hash = None;
        self.zoom = 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        self.is_dragging = false;
        self.drag_start_mouse = None;
    }

    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom + 0.25);
    }

    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom - 0.25);
    }

    pub fn reset(&mut self) {
        self.zoom = 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        self.is_dragging = false;
        self.drag_start_mouse = None;
    }

    pub fn set_zoom(&mut self, new_zoom: f32) {
        self.zoom = new_zoom.clamp(0.2, 5.0);
    }

    pub fn apply_scroll_zoom(&mut self, delta_y: f32) {
        let factor = 1.0 + (delta_y * 0.002);
        self.set_zoom(self.zoom * factor);
    }

    pub fn start_drag(&mut self, mouse_x: f32, mouse_y: f32) {
        self.is_dragging = true;
        self.drag_start_mouse = Some((mouse_x, mouse_y));
        self.drag_start_pan = (self.pan_x, self.pan_y);
    }

    pub fn update_drag(&mut self, mouse_x: f32, mouse_y: f32) {
        if let Some((start_x, start_y)) = self.drag_start_mouse {
            if self.is_dragging {
                self.pan_x = self.drag_start_pan.0 + (mouse_x - start_x);
                self.pan_y = self.drag_start_pan.1 + (mouse_y - start_y);
            }
        }
    }

    pub fn end_drag(&mut self) {
        self.is_dragging = false;
        self.drag_start_mouse = None;
    }
}

pub fn render_lightbox(
    state: &LightboxState,
    _theme: &ThemeColors,
    cx: &mut Context<WorkspaceView>,
) -> impl IntoElement {
    let svg_path = state.svg_path.clone();
    let pan_x = state.pan_x;
    let pan_y = state.pan_y;
    let zoom = state.zoom;
    let is_dragging = state.is_dragging;

    div()
        .id("diagram_lightbox_backdrop")
        .absolute()
        .inset_0()
        .bg(rgba(0x00000073)) // rgba(0, 0, 0, 0.45)
        .occlude() // block background mouse interactions
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
            this.lightbox.close();
            cx.notify();
        }))
        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
            if this.lightbox.is_dragging {
                this.lightbox.update_drag(f32::from(event.position.x), f32::from(event.position.y));
                cx.notify();
            }
        }))
        .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _window, cx| {
            if this.lightbox.is_dragging {
                this.lightbox.end_drag();
                cx.notify();
            }
        }))
        .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _window, cx| {
            let delta = match event.delta {
                ScrollDelta::Pixels(p) => -f32::from(p.y),
                ScrollDelta::Lines(l) => -f32::from(l.y) * 20.0,
            };
            this.lightbox.apply_scroll_zoom(delta);
            cx.notify();
        }))
        .child(
            // Centered vector diagram canvas
            div()
                .id("lightbox_diagram_card")
                .relative()
                .left(px(pan_x))
                .top(px(pan_y))
                .cursor(if is_dragging { CursorStyle::ClosedHand } else { CursorStyle::OpenHand })
                .on_mouse_down(MouseButton::Left, cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                    cx.stop_propagation(); // Don't trigger backdrop click
                    this.lightbox.start_drag(f32::from(event.position.x), f32::from(event.position.y));
                    cx.notify();
                }))
                .children(svg_path.map(|path| {
                    img(path)
                        .w(px(800.0 * zoom))
                        .max_w(px(800.0 * zoom))
                }))
        )
        // Floating bottom HUD pill: [ - ] [ Current % ] [ + ] | [ 适应窗口 ] | [ 关闭 (Esc) ]
        .child(
            div()
                .id("lightbox_hud_pill")
                .absolute()
                .bottom(px(32.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .items_center()
                .child(
                    div()
                        .px_3()
                        .py_1p5()
                        .rounded_full()
                        .bg(rgba(0x18181bf2))
                        .border_1()
                        .border_color(rgba(0xffffff26))
                        .shadow_xl()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .text_color(rgb(0xf4f4f5))
                        .font_family("ui-sans-serif, system-ui")
                        .on_mouse_down(MouseButton::Left, |_event, _window, _app| {}) // Stop propagation
                        // [ - ] Zoom out
                        .child(
                            div()
                                .id("hud_btn_zoom_out")
                                .w(px(22.))
                                .h(px(22.))
                                .rounded_full()
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(14.))
                                .hover(|s| s.bg(rgba(0xffffff20)))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                    this.lightbox.zoom_out();
                                    cx.notify();
                                }))
                                .child("−")
                        )
                        // [ 100% ] Click to reset
                        .child(
                            div()
                                .id("hud_zoom_display")
                                .min_w(px(46.))
                                .px_1()
                                .flex()
                                .justify_center()
                                .items_center()
                                .cursor_pointer()
                                .font_weight(FontWeight::MEDIUM)
                                .hover(|s| s.text_color(rgb(0x38bdf8)))
                                .tooltip(|_window, cx| {
                                    cx.new(|_| HudTooltipView { text: "点击重置为 100%".to_string() }).into()
                                })
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                    this.lightbox.reset();
                                    cx.notify();
                                }))
                                .child(format!("{:.0}%", zoom * 100.0))
                        )
                        // [ + ] Zoom in
                        .child(
                            div()
                                .id("hud_btn_zoom_in")
                                .w(px(22.))
                                .h(px(22.))
                                .rounded_full()
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(14.))
                                .hover(|s| s.bg(rgba(0xffffff20)))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                    this.lightbox.zoom_in();
                                    cx.notify();
                                }))
                                .child("+")
                        )
                        // Divider
                        .child(div().w(px(1.)).h(px(12.)).bg(rgba(0xffffff26)))
                        // [ 适应窗口 ]
                        .child(
                            div()
                                .id("hud_btn_fit")
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(0xffffff20)))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                    this.lightbox.reset();
                                    cx.notify();
                                }))
                                .child("适应窗口")
                        )
                        // Divider
                        .child(div().w(px(1.)).h(px(12.)).bg(rgba(0xffffff26)))
                        // [ 关闭 (Esc) ]
                        .child(
                            div()
                                .id("hud_btn_close")
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(0xffffff20)).text_color(rgb(0xf87171)))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                    this.lightbox.close();
                                    cx.notify();
                                }))
                                .child("关闭 (Esc)")
                        )
                )
        )
        .with_animation(
            "lightbox_fade_in",
            Animation::new(std::time::Duration::from_millis(150)),
            |el, delta| el.opacity(delta),
        )
}

struct HudTooltipView {
    text: String,
}

impl Render for HudTooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgba(0x18181bf2))
            .border_1()
            .border_color(rgba(0x3f3f4640))
            .text_size(px(11.))
            .text_color(rgb(0xf4f4f5))
            .font_family("ui-sans-serif, system-ui")
            .shadow_md()
            .child(self.text.clone())
    }
}


