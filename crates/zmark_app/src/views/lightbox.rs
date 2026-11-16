use gpui::prelude::*;
use gpui::*;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};
use zmark_core::i18n::I18nStrings;

use crate::theme::ThemeColors;
use crate::views::workspace::WorkspaceView;

const LIGHTBOX_MARGIN: f32 = 24.0;
const LIGHTBOX_PADDING: f32 = 16.0;
const LIGHTBOX_HUD_SPACE: f32 = 88.0;
const DEFAULT_IMAGE_SIZE: (f32, f32) = (800.0, 640.0);

#[derive(Debug)]
pub struct LightboxDiagramCache {
    entries: HashMap<(u64, bool, u8), PathBuf>,
    order: VecDeque<(u64, bool, u8)>,
    max_size: usize,
}

impl LightboxDiagramCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            max_size,
        }
    }

    pub fn get(&mut self, key: &(u64, bool, u8)) -> Option<PathBuf> {
        if let Some(path) = self.entries.get(key).cloned() {
            if let Some(pos) = self.order.iter().position(|k| k == key) {
                self.order.remove(pos);
            }
            self.order.push_back(*key);
            Some(path)
        } else {
            None
        }
    }

    pub fn insert(&mut self, key: (u64, bool, u8), path: PathBuf) {
        if let Some(pos) = self.order.iter().position(|k| k == &key) {
            self.order.remove(pos);
        }
        if let Some(old_path) = self.entries.insert(key, path.clone()) {
            if old_path != path {
                let _ = std::fs::remove_file(old_path);
            }
        }
        self.order.push_back(key);

        while self.order.len() > self.max_size {
            if let Some(evicted_key) = self.order.pop_front() {
                if let Some(evicted_path) = self.entries.remove(&evicted_key) {
                    let _ = std::fs::remove_file(evicted_path);
                }
            }
        }
    }
}

pub static LIGHTBOX_DIAGRAM_CACHE: LazyLock<Mutex<LightboxDiagramCache>> =
    LazyLock::new(|| Mutex::new(LightboxDiagramCache::new(1)));

#[cfg(any(test, debug_assertions))]
pub static TEST_RELEASE_CALL_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[cfg(any(test, debug_assertions))]
pub static TEST_DROP_IMAGE_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Removes GPU texture from sprite atlas and increments test drop counter in test/debug builds.
#[inline]
pub fn drop_image_from_atlas(
    cx: &mut App,
    render_image: Arc<gpui::RenderImage>,
    window: &mut Window,
) {
    cx.drop_image(render_image, Some(window));
    #[cfg(any(test, debug_assertions))]
    TEST_DROP_IMAGE_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// Unified release function for exclusive lightbox images:
/// 1. Retrieves cached `RenderImage` via `window.get_asset::<ImgResourceLoader>`.
/// 2. If present, calls `drop_image_from_atlas(cx, render, window)` to remove GPU texture from sprite atlas.
/// 3. Purges from loading assets cache via `cx.remove_asset`.
pub fn release_lightbox_image(path: &std::path::Path, window: &mut Window, cx: &mut App) {
    #[cfg(any(test, debug_assertions))]
    TEST_RELEASE_CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let resource = gpui::Resource::Path(std::sync::Arc::from(path));
    if let Some(Ok(render_image)) = window.get_asset::<gpui::ImgResourceLoader>(&resource, cx) {
        drop_image_from_atlas(cx, render_image, window);
    }
    cx.remove_asset::<gpui::ImgResourceLoader>(&resource);
}

#[derive(Debug, Clone, PartialEq)]
pub struct LightboxState {
    pub is_open: bool,
    pub svg_path: Option<PathBuf>,
    pub hidpi_svg_path: Option<PathBuf>,
    pub is_dark: Option<bool>,
    pub code_hash: Option<u64>,
    pub zoom: f32,
    /// Original logical size, independent of the displayed texture's pixel budget.
    pub natural_size: (f32, f32),
    pub is_fitted: bool,
    fit_zoom: f32,
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
            hidpi_svg_path: None,
            is_dark: None,
            code_hash: None,
            zoom: 1.0,
            natural_size: DEFAULT_IMAGE_SIZE,
            is_fitted: false,
            fit_zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            is_dragging: false,
            drag_start_mouse: None,
            drag_start_pan: (0.0, 0.0),
        }
    }

    pub fn displayed_path(&self) -> Option<&PathBuf> {
        self.hidpi_svg_path.as_ref().or(self.svg_path.as_ref())
    }

    /// Opens the lightbox with a shared/fallback SVG path.
    /// If an exclusive HiDPI image was active, returns it for reclamation.
    /// The shared path itself is NOT returned, protecting preview assets from being dropped on close.
    pub fn open(&mut self, svg_path: PathBuf, code_hash: u64) -> Option<PathBuf> {
        let size = lightbox_image_size(&svg_path);
        self.open_with_size(svg_path, code_hash, size)
    }

    pub fn open_with_size(&mut self, svg_path: PathBuf, code_hash: u64, size: (f32, f32)) -> Option<PathBuf> {
        let old_exclusive = self.hidpi_svg_path.take();
        self.natural_size = size;
        self.is_open = true;
        self.svg_path = Some(svg_path);
        self.code_hash = Some(code_hash);
        self.fit_zoom = 1.0;
        self.fit();
        old_exclusive
    }

    /// Opens the lightbox with an exclusive HiDPI SVG path.
    /// Returns any previous exclusive HiDPI image for reclamation.
    pub fn open_hidpi(
        &mut self,
        svg_path: PathBuf,
        hidpi_svg_path: PathBuf,
        code_hash: u64,
    ) -> Option<PathBuf> {
        let old_exclusive = self.open(svg_path, code_hash);
        self.hidpi_svg_path = Some(hidpi_svg_path);
        old_exclusive
    }

    /// Closes the lightbox and returns any exclusive HiDPI SVG path for reclamation.
    /// If only a shared fallback path was displayed, returns None to avoid dropping preview assets.
    pub fn close(&mut self) -> Option<PathBuf> {
        let old_exclusive = self.hidpi_svg_path.take();
        self.is_open = false;
        self.svg_path = None;
        self.is_dark = None;
        self.code_hash = None;
        self.natural_size = DEFAULT_IMAGE_SIZE;
        self.fit_zoom = 1.0;
        self.reset();
        old_exclusive
    }

    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom + 0.25);
    }

    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom - 0.25);
    }

    pub fn reset(&mut self) {
        self.zoom = 1.0;
        self.is_fitted = false;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        self.is_dragging = false;
        self.drag_start_mouse = None;
        self.drag_start_pan = (0.0, 0.0);
    }

    pub fn fit(&mut self) {
        self.reset();
        self.is_fitted = true;
    }

    pub fn update_viewport(&mut self, viewport: Size<Pixels>) {
        let (width, height) = (f32::from(viewport.width), f32::from(viewport.height));
        if !self.is_fitted || !width.is_finite() || !height.is_finite() {
            return;
        }
        let available_width = (width - 2.0 * (LIGHTBOX_MARGIN + LIGHTBOX_PADDING)).max(1.0);
        let available_height =
            (height - LIGHTBOX_HUD_SPACE - 2.0 * (LIGHTBOX_MARGIN + LIGHTBOX_PADDING)).max(1.0);
        self.fit_zoom =
            (available_width / self.natural_size.0).min(available_height / self.natural_size.1);
        self.zoom = self.fit_zoom;
    }

    pub fn set_zoom(&mut self, new_zoom: f32) {
        if !new_zoom.is_finite() {
            return;
        }
        // Keep fit reachable, even when intrinsic dimensions put it outside 20–500%.
        self.zoom = new_zoom.clamp(0.2 * self.fit_zoom.min(1.0), 5.0 * self.fit_zoom.max(1.0));
        self.is_fitted = false;
    }

    pub fn apply_scroll_zoom(&mut self, delta_y: f32) {
        if !delta_y.is_finite() || delta_y == 0.0 {
            return;
        }
        // Reciprocal steps avoid drift and negative factors on large wheel events.
        let factor = 1.1_f32.powf((delta_y / 50.0).clamp(-8.0, 8.0));
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
                let pan = (
                    self.drag_start_pan.0 + (mouse_x - start_x),
                    self.drag_start_pan.1 + (mouse_y - start_y),
                );
                if pan != (self.pan_x, self.pan_y) {
                    self.is_fitted = false;
                }
                (self.pan_x, self.pan_y) = pan;
            }
        }
    }

    pub fn end_drag(&mut self) {
        self.is_dragging = false;
        self.drag_start_mouse = None;
    }
}

fn lightbox_image_size(path: &std::path::Path) -> (f32, f32) {
    let (w, h) = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
    {
        std::fs::read_to_string(path)
            .map(|svg| zmark_core::mermaid::extract_svg_dimensions(&svg))
            .map(|(w, h)| (w as f32, h as f32))
            .unwrap_or(DEFAULT_IMAGE_SIZE)
    } else {
        image::image_dimensions(path)
            .map(|(w, h)| (w as f32, h as f32))
            .unwrap_or(DEFAULT_IMAGE_SIZE)
    };
    if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 {
        (w.max(1.0), h.max(1.0))
    } else {
        DEFAULT_IMAGE_SIZE
    }
}

/// Physical pixel size to re-rasterize the open SVG at, or (0, 0) when no re-render applies
/// (raster image, nothing open). Rounded to 64px steps so small zoom changes reuse the result,
/// and capped at 4096px per side and 16 megapixels.
pub fn sharp_target_px(state: &LightboxState, scale_factor: f32) -> (u32, u32) {
    let is_svg = state
        .svg_path
        .as_ref()
        .is_some_and(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg")));
    if !state.is_open || !is_svg || !scale_factor.is_finite() || scale_factor <= 0.0 {
        return (0, 0);
    }
    let (mut w, mut h) = (
        state.natural_size.0 * state.zoom * scale_factor,
        state.natural_size.1 * state.zoom * scale_factor,
    );
    let shrink = (4096.0 / w.max(h)).min((16_000_000.0 / (w * h)).sqrt()).min(1.0);
    (w, h) = (w * shrink, h * shrink);
    let step = |v: f32| (((v / 64.0).ceil() as u32).saturating_mul(64)).clamp(64, 4000);
    (step(w), step(h))
}

pub fn render_lightbox(
    state: &mut LightboxState,
    sharp: Option<Arc<RenderImage>>,
    viewport: Size<Pixels>,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    reduce_motion: bool,
    cx: &mut Context<WorkspaceView>,
) -> impl IntoElement {
    state.update_viewport(viewport);
    let (image_width, image_height) = state.natural_size;
    let pan_x = state.pan_x;
    let pan_y = state.pan_y;
    let zoom = state.zoom;
    let is_dragging = state.is_dragging;

    let backdrop = div()
        .id("diagram_lightbox_backdrop")
        .debug_selector(|| "diagram_lightbox_backdrop".into())
        .absolute()
        .inset_0()
        .bg(rgba(0x000000e0)) // ~88% deep black
        .occlude() // block background mouse interactions
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, window, cx| {
                this.close_lightbox(window, cx);
                cx.notify();
            }),
        )
        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
            if this.lightbox.is_dragging {
                if event.pressed_button == Some(MouseButton::Left) {
                    this.lightbox
                        .update_drag(f32::from(event.position.x), f32::from(event.position.y));
                } else {
                    this.lightbox.end_drag();
                }
                cx.notify();
            }
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _, _window, cx| {
                if this.lightbox.is_dragging {
                    this.lightbox.end_drag();
                    cx.notify();
                }
            }),
        )
        .on_scroll(cx.listener(|this, event: &ScrollEvent, _window, cx| {
            let delta = match event.delta {
                ScrollDelta::Pixels(p) => f32::from(p.y),
                ScrollDelta::Lines(l) => l.y * 20.0,
            };
            // Windows wheel-up is positive; retain macOS's existing natural-scroll mapping.
            let delta = if cfg!(target_os = "windows") { delta } else { -delta };
            this.lightbox.apply_scroll_zoom(delta);
            cx.stop_propagation();
            cx.notify();
        }))
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top_0()
                .bottom(px(LIGHTBOX_HUD_SPACE))
                .p(px(LIGHTBOX_MARGIN))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .id("lightbox_diagram_card")
                        .debug_selector(|| "lightbox_diagram_card".into())
                        .relative()
                        .left(px(pan_x))
                        .top(px(pan_y))
                        .rounded_xl()
                        .p(px(LIGHTBOX_PADDING))
                        .shadow_lg()
                        .bg(theme.surface)
                        .cursor(if is_dragging {
                            CursorStyle::ClosedHand
                        } else {
                            CursorStyle::OpenHand
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                                cx.stop_propagation();
                                this.lightbox.start_drag(
                                    f32::from(event.position.x),
                                    f32::from(event.position.y),
                                );
                                cx.notify();
                            }),
                        )
                        .children(match (sharp, state.displayed_path()) {
                            (Some(image), _) => Some(img(image)),
                            (None, Some(path)) => Some(img(path.clone())),
                            _ => None,
                        }.map(|image| {
                            image
                                .w(px(image_width * zoom))
                                .h(px(image_height * zoom))
                                .object_fit(ObjectFit::Contain)
                        })),
                ),
        )
        // Floating bottom HUD pill: [ - ] [ Current % ] [ + ] | [ 适应窗口 ] | [ 关闭 (Esc) ]
        .child(
            div()
                .id("lightbox_hud_pill")
                .debug_selector(|| "lightbox_hud_pill".into())
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
                        .font_family(crate::platform::UI_FONT)
                        .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
                            cx.stop_propagation()
                        })
                        // [ - ] Zoom out
                        .child(
                            div()
                                .id("hud_btn_zoom_out")
                                .debug_selector(|| "hud_btn_zoom_out".into())
                                .w(px(22.))
                                .h(px(22.))
                                .rounded_full()
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(14.))
                                .hover(|s| s.bg(rgba(0xffffff20)))
                                .tooltip({
                                    let t = strings.lightbox_zoom_out.to_string();
                                    move |_window, cx| {
                                        cx.new(|_| HudTooltipView { text: t.clone() }).into()
                                    }
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _window, cx| {
                                        this.lightbox.zoom_out();
                                        cx.notify();
                                    }),
                                )
                                .child("−"),
                        )
                        // [ 100% ] Click to reset
                        .child(
                            div()
                                .id("hud_zoom_display")
                                .debug_selector(|| "hud_zoom_display".into())
                                .min_w(px(46.))
                                .px_1()
                                .flex()
                                .justify_center()
                                .items_center()
                                .cursor_pointer()
                                .font_weight(FontWeight::MEDIUM)
                                .hover(|s| s.text_color(rgb(0x38bdf8)))
                                .tooltip({
                                    let t = strings.lightbox_reset.to_string();
                                    move |_window, cx| {
                                        cx.new(|_| HudTooltipView { text: t.clone() }).into()
                                    }
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _window, cx| {
                                        this.lightbox.reset();
                                        cx.notify();
                                    }),
                                )
                                .child(format!("{:.0}%", zoom * 100.0)),
                        )
                        // [ + ] Zoom in
                        .child(
                            div()
                                .id("hud_btn_zoom_in")
                                .debug_selector(|| "hud_btn_zoom_in".into())
                                .w(px(22.))
                                .h(px(22.))
                                .rounded_full()
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(14.))
                                .hover(|s| s.bg(rgba(0xffffff20)))
                                .tooltip({
                                    let t = strings.lightbox_zoom_in.to_string();
                                    move |_window, cx| {
                                        cx.new(|_| HudTooltipView { text: t.clone() }).into()
                                    }
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _window, cx| {
                                        this.lightbox.zoom_in();
                                        cx.notify();
                                    }),
                                )
                                .child("+"),
                        )
                        // Divider
                        .child(div().w(px(1.)).h(px(12.)).bg(rgba(0xffffff26)))
                        // [ Fit to window ]
                        .child(
                            div()
                                .id("hud_btn_fit")
                                .debug_selector(|| "hud_btn_fit".into())
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(0xffffff20)))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _window, cx| {
                                        this.lightbox.fit();
                                        cx.notify();
                                    }),
                                )
                                .child(strings.lightbox_fit_window),
                        )
                        // Divider
                        .child(div().w(px(1.)).h(px(12.)).bg(rgba(0xffffff26)))
                        // [ Close (Esc) ]
                        .child(
                            div()
                                .id("hud_btn_close")
                                .debug_selector(|| "hud_btn_close".into())
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(0xffffff20)).text_color(rgb(0xf87171)))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        this.close_lightbox(window, cx);
                                        cx.notify();
                                    }),
                                )
                                .child(format!("{} (Esc)", strings.lightbox_close)),
                        ),
                ),
        );
    crate::views::motion::animated(
        backdrop,
        reduce_motion,
        "lightbox_fade_in",
        150,
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
            .font_family(crate::platform::UI_FONT)
            .shadow_md()
            .child(self.text.clone())
    }
}
