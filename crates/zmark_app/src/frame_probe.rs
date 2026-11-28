//! Opt-in frame-time probe for animation, scroll and selection acceptance.
//!
//! Enable with `ZMARK_FRAME_PROBE=1` (or set it to an output path). For three
//! seconds every frame interval is recorded, then written once together with a
//! p50/p95/p99/worst summary. `on_next_frame` is one-shot, so each callback
//! re-registers and requests the next frame.
//!
//! `ZMARK_FRAME_PROBE_SCENARIO` picks what is driven while the probe records:
//! - `modes` (default): switch Split/Preview/Source every 300 ms.
//! - `drag-scroll`: a 12 000-line document in Preview; press near the top,
//!   drag past the bottom edge so autoscroll runs, then wheel-scroll while the
//!   button is still held.
//! - `table-select`: a 30 × 6 table (180 cells) in Preview; drag the selection
//!   back and forth across the table.
//! - `resize`: a 12 000-line document in Preview; change the window width
//!   every 16 ms between 900 and 1400 px.
//! - `doc-switch`: two 12 000-line documents (temporary files); switch between
//!   them every 240 ms.
//! - `task-toggle`: a 12 000-line document of task lists; toggle the first
//!   visible task every 240 ms.
//! - `line-height`: a 12 000-line document; alternate the line height
//!   preference between 1.3 and 1.7 every 240 ms.
//! - `bottom-scroll`: the welcome guide (images, a diagram, a table) scrolled
//!   to its end, then wheel-scrolled further down every step.
//!
//! The last three start scrolled to block 500 and print the top block at the
//! end: it must still be block 500 (on `doc-switch`, after an even number of
//! switches), so the viewport did not drift while layouts were rebuilt.
//!
//! Scenarios other than `modes` use a generated scratch document and an
//! isolated preferences file, so they never touch the user's session or config.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use gpui::{
    px, size, App, Entity, EntityId, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PlatformInput, Point, ScrollDelta, ScrollEvent, Window,
};

use crate::state::ViewMode;
use crate::views::workspace::WorkspaceView;

const WINDOW: Duration = Duration::from_secs(3);
const DEFAULT_PATH: &str = "/tmp/zmark_frames.json";
/// Pause between parse/layout and the first recorded frame of a scenario.
const SETTLE: Duration = Duration::from_millis(1500);
/// Synthetic input cadence: one event per 120 Hz frame.
const STEP: Duration = Duration::from_millis(8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    Modes,
    DragScroll,
    TableSelect,
    Resize,
    DocSwitch,
    TaskToggle,
    LineHeight,
    BottomScroll,
}

impl Scenario {
    pub fn name(self) -> &'static str {
        match self {
            Scenario::Modes => "modes",
            Scenario::DragScroll => "drag-scroll",
            Scenario::TableSelect => "table-select",
            Scenario::Resize => "resize",
            Scenario::DocSwitch => "doc-switch",
            Scenario::TaskToggle => "task-toggle",
            Scenario::LineHeight => "line-height",
            Scenario::BottomScroll => "bottom-scroll",
        }
    }

    /// Document a scenario opens instead of the user's session. `None` keeps
    /// the normal startup document.
    pub fn document(self) -> Option<String> {
        match self {
            Scenario::Modes => None,
            Scenario::DragScroll | Scenario::Resize | Scenario::DocSwitch | Scenario::LineHeight => {
                Some(long_document(12_000))
            }
            Scenario::TaskToggle => Some(task_document(12_000)),
            Scenario::BottomScroll => Some(crate::welcome::document(zmark_core::i18n::SupportedLocale::ZhCn).buffer),
            Scenario::TableSelect => Some(table_document(30, 6)),
        }
    }
}

/// The scenario requested by the environment, or `None` when the probe is off.
pub fn requested() -> Option<Scenario> {
    output_path(None)?;
    let scenario = match std::env::var("ZMARK_FRAME_PROBE_SCENARIO").as_deref() {
        Ok("drag-scroll") => Scenario::DragScroll,
        Ok("table-select") => Scenario::TableSelect,
        Ok("resize") => Scenario::Resize,
        Ok("doc-switch") => Scenario::DocSwitch,
        Ok("task-toggle") => Scenario::TaskToggle,
        Ok("line-height") => Scenario::LineHeight,
        Ok("bottom-scroll") => Scenario::BottomScroll,
        _ => Scenario::Modes,
    };
    Some(scenario)
}

fn output_path(scenario: Option<Scenario>) -> Option<PathBuf> {
    let setting = std::env::var("ZMARK_FRAME_PROBE").ok()?;
    if setting.is_empty() || setting == "0" {
        return None;
    }
    if setting != "1" {
        return Some(PathBuf::from(setting));
    }
    Some(match scenario {
        None | Some(Scenario::Modes) => PathBuf::from(DEFAULT_PATH),
        Some(s) => PathBuf::from(format!("/tmp/zmark_frames_{}.json", s.name())),
    })
}

fn long_document(lines: usize) -> String {
    let mut md = String::with_capacity(lines * 64);
    let mut written = 0;
    let mut block = 0;
    while written < lines {
        md.push_str(&format!("## Section {block}\n\n"));
        md.push_str(&format!(
            "Paragraph {block} has enough words to wrap across the reading column when the window is narrow, \
             so layout work matches a real note rather than a single short line.\n\n"
        ));
        md.push_str(&format!("- item {block}.a\n- item {block}.b with `inline code`\n\n"));
        written += 9;
        block += 1;
    }
    md
}

fn task_document(lines: usize) -> String {
    let mut md = String::with_capacity(lines * 48);
    let mut block = 0;
    while md.lines().count() < lines {
        md.push_str(&format!("Tasks for section {block}:\n\n"));
        md.push_str(&format!("- [ ] task {block}.a with a description long enough to wrap\n- [x] task {block}.b\n\n"));
        block += 1;
    }
    md
}

/// Starting block for the layout-rebuild scenarios.
const START_BLOCK: usize = 500;
/// Steps between two rebuild actions (240 ms at 8 ms per step).
const ACTION_EVERY: usize = 30;

fn table_document(rows: usize, cols: usize) -> String {
    let mut md = String::new();
    md.push('|');
    for c in 0..cols {
        md.push_str(&format!(" Column {c} |"));
    }
    md.push_str("\n|");
    for _ in 0..cols {
        md.push_str(" --- |");
    }
    md.push('\n');
    for r in 0..rows {
        md.push('|');
        for c in 0..cols {
            md.push_str(&format!(" cell {r}-{c} text |"));
        }
        md.push('\n');
    }
    md.push_str("\nParagraph after the table.\n");
    md
}

struct ProbeState {
    started: Instant,
    last: Instant,
    samples: Vec<f64>,
    path: PathBuf,
    scenario: Scenario,
}

static PROBE: Mutex<Option<ProbeState>> = Mutex::new(None);

/// Start the requested scenario. Called once after the main window is built.
pub fn run(scenario: Scenario, view: Entity<WorkspaceView>, window: &mut Window, cx: &mut App) {
    // An occluded window receives no display-link frames. Bring the whole
    // app forward, not only the window, so other apps cannot cover it.
    cx.activate(true);
    window.activate_window();
    match scenario {
        Scenario::Modes => {
            start_recording(scenario, window, view.entity_id());
            cx.spawn(async move |app| {
                let modes = [
                    ViewMode::Split,
                    ViewMode::Preview,
                    ViewMode::Source,
                    ViewMode::Split,
                    ViewMode::Preview,
                    ViewMode::Source,
                ];
                for mode in modes {
                    app.background_executor().timer(Duration::from_millis(300)).await;
                    let _ = view.update(app, |workspace, cx| workspace.set_view_mode(mode, cx));
                }
            })
            .detach();
        }
        _ => {
            if scenario == Scenario::DocSwitch {
                let path = std::env::temp_dir().join("zmark-frame-probe-b.md");
                if std::fs::write(&path, long_document(12_000)).is_ok() {
                    view.update(cx, |ws, cx| {
                        if ws.state.open_file(path).is_ok() {
                            ws.on_document_swapped(Some(window), cx);
                        }
                    });
                }
            }
            // `ZMARK_FRAME_PROBE_MODE=split` runs a scenario in Split instead of Preview.
            let mode = if std::env::var("ZMARK_FRAME_PROBE_MODE").as_deref() == Ok("split") { ViewMode::Split } else { ViewMode::Preview };
            view.update(cx, |ws, cx| {
                ws.set_view_mode(mode, cx);
                ws.parse_now_and_refresh();
                cx.notify();
            });
            if scenario == Scenario::BottomScroll {
                let view = view.clone();
                window
                    .spawn(cx, async move |cx| {
                        cx.background_executor().timer(SETTLE / 2).await;
                        let _ = cx.update(|_, cx| view.update(cx, |ws, cx| {
                            let max = ws.preview_list.max_scroll();
                            ws.preview_list.set_scroll_offset(max);
                            cx.notify();
                        }));
                    })
                    .detach();
            }
            if matches!(scenario, Scenario::DocSwitch | Scenario::TaskToggle | Scenario::LineHeight) {
                let view = view.clone();
                window
                    .spawn(cx, async move |cx| {
                        // Let the first layout settle, then park the viewport mid-document.
                        cx.background_executor().timer(SETTLE / 2).await;
                        let _ = cx.update(|_, cx| view.update(cx, |ws, cx| {
                            ws.preview_list.scroll_to(gpui::ListOffset { item_ix: START_BLOCK, offset_in_item: px(0.) });
                            cx.notify();
                        }));
                    })
                    .detach();
            }
            window
                .spawn(cx, async move |cx| {
                    cx.background_executor().timer(SETTLE).await;
                    let ready = cx.update(|window, cx| {
                        let bounds = view.read(cx).preview_content_bounds;
                        if bounds.is_some() {
                            start_recording(scenario, window, view.entity_id());
                        }
                        bounds
                    });
                    let Ok(Some(pane)) = ready else {
                        eprintln!("frame probe: preview pane never reported its bounds");
                        return;
                    };
                    let deadline = Instant::now() + WINDOW;
                    let mut step = 0usize;
                    while Instant::now() < deadline {
                        let _ = cx.update(|window, cx| drive(scenario, step, pane, &view, window, cx));
                        step += 1;
                        cx.background_executor().timer(STEP).await;
                    }
                    let _ = cx.update(|window, cx| finish(scenario, pane, &view, window, cx));
                })
                .detach();
        }
    }
}

fn mouse(window: &mut Window, cx: &mut App, input: PlatformInput) {
    window.dispatch_event(input, cx);
}

fn drive(
    scenario: Scenario,
    step: usize,
    pane: gpui::Bounds<gpui::Pixels>,
    view: &Entity<WorkspaceView>,
    window: &mut Window,
    cx: &mut App,
) {
    let left = pane.origin.x + px(60.);
    let top = pane.origin.y;
    let height = pane.size.height;
    let width = pane.size.width;
    match scenario {
        Scenario::Modes => {}
        Scenario::DragScroll => {
            // ~375 steps in 3 s: press, drag to the bottom edge over 60 steps,
            // hold past it for autoscroll, then wheel while still pressed.
            let press = Point::new(left, top + px(40.));
            if step == 0 {
                mouse(window, cx, PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: press,
                    modifiers: Modifiers::none(),
                    click_count: 1,
                    first_mouse: false,
                }));
            } else if step <= 60 {
                let t = step as f32 / 60.;
                let y = top + px(40.) + (height + px(20.) - px(40.)) * t;
                drag_to(window, cx, Point::new(left + px(100.) * t, y));
            } else if step <= 200 {
                drag_to(window, cx, Point::new(left + px(100.), top + height + px(20.)));
            } else {
                let inside = Point::new(left + px(100.), top + height - px(60.));
                drag_to(window, cx, inside);
                mouse(window, cx, PlatformInput::Scroll(ScrollEvent {
                    position: inside,
                    delta: ScrollDelta::Pixels(Point::new(px(0.), px(-40.))),
                    modifiers: Modifiers::none(),
                    ..Default::default()
                }));
            }
        }
        Scenario::TableSelect => {
            // The table starts at the top of the document. Anchor in the first
            // body row and sweep the head across rows and columns.
            let anchor = Point::new(left, top + px(70.));
            if step == 0 {
                mouse(window, cx, PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: anchor,
                    modifiers: Modifiers::none(),
                    click_count: 1,
                    first_mouse: false,
                }));
                return;
            }
            let phase = (step % 120) as f32 / 120.;
            let tri = if phase < 0.5 { phase * 2. } else { 2. - phase * 2. };
            let x = left + (width - px(140.)) * tri;
            let y = top + px(70.) + (height.min(px(900.)) - px(120.)) * ((step % 37) as f32 / 37.);
            drag_to(window, cx, Point::new(x, y));
        }
        Scenario::Resize => {
            let phase = (step % 60) as f32 / 60.;
            let tri = if phase < 0.5 { phase * 2. } else { 2. - phase * 2. };
            window.resize(size(px(900. + 500. * tri), px(800.)));
        }
        Scenario::BottomScroll => {
            let inside = Point::new(left + px(100.), top + height / 2.);
            mouse(window, cx, PlatformInput::Scroll(ScrollEvent {
                position: inside,
                delta: ScrollDelta::Pixels(Point::new(px(0.), px(-30.))),
                modifiers: Modifiers::none(),
                ..Default::default()
            }));
        }
        _ if step % ACTION_EVERY != ACTION_EVERY - 1 => {}
        Scenario::DocSwitch => {
            view.update(cx, |ws, cx| {
                let docs = ws.state.open_documents();
                let Some(pos) = docs.iter().position(|d| Some(&d.key) == ws.state.active.as_ref()) else { return; };
                let next = docs[(pos + 1) % docs.len()].key.clone();
                ws.state.switch_document(&next);
                ws.on_document_swapped(Some(window), cx);
                cx.notify();
            });
        }
        Scenario::TaskToggle => {
            view.update(cx, |ws, cx| {
                let top = ws.preview_list.logical_scroll_top().item_ix;
                let parsed = ws.state.doc().parsed.clone();
                let task = parsed.blocks.iter().skip(top).find_map(|block| match block {
                    zmark_core::ast::BlockElement::List { items, .. } => items
                        .iter()
                        .find_map(|item| item.marker.clone().zip(item.checked)),
                    _ => None,
                });
                if let Some((marker, checked)) = task {
                    ws.toggle_task(marker, checked, cx);
                }
            });
        }
        Scenario::LineHeight => {
            view.update(cx, |ws, cx| {
                let next = if (step / ACTION_EVERY) % 2 == 0 { 1.3 } else { 1.7 };
                ws.state.set_line_height(next);
                cx.notify();
            });
        }
    }
}

fn drag_to(window: &mut Window, cx: &mut App, position: Point<gpui::Pixels>) {
    mouse(window, cx, PlatformInput::MouseMove(MouseMoveEvent {
        position,
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    }));
}

fn finish(
    scenario: Scenario,
    pane: gpui::Bounds<gpui::Pixels>,
    view: &Entity<WorkspaceView>,
    window: &mut Window,
    cx: &mut App,
) {
    if matches!(scenario, Scenario::DragScroll | Scenario::TableSelect) {
        mouse(window, cx, PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: pane.center(),
            modifiers: Modifiers::none(),
            click_count: 1,
        }));
    }
    let ws = view.read(cx);
    let selection = ws.preview_selection.map(|s| {
        format!(
            "anchor block {} char {} -> head block {} char {}",
            s.anchor.block_ix, s.anchor.char_offset, s.head.block_ix, s.head.char_offset
        )
    });
    let top = ws.preview_list.logical_scroll_top();
    let active = ws.state.doc().display_name_with_fallback("untitled");
    eprintln!(
        "frame probe [{}]: selection {:?}, scroll top block {}, active {}",
        scenario.name(),
        selection,
        top.item_ix,
        active
    );
}

fn start_recording(scenario: Scenario, window: &mut Window, view_id: EntityId) {
    let Some(path) = output_path(Some(scenario)) else {
        return;
    };
    let now = Instant::now();
    *PROBE.lock().unwrap() = Some(ProbeState {
        started: now,
        last: now,
        samples: Vec::new(),
        path,
        scenario,
    });
    window.on_next_frame(move |window, cx| tick(window, cx, view_id));
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let ix = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[ix]
}

fn summary(samples: &[f64]) -> serde_json::Value {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = percentile(&sorted, 0.50);
    serde_json::json!({
        "frames": sorted.len(),
        "p50_ms": p50,
        "p95_ms": percentile(&sorted, 0.95),
        "p99_ms": percentile(&sorted, 0.99),
        "worst_ms": sorted.last().copied().unwrap_or(0.0),
        "over_16_7ms": sorted.iter().filter(|&&v| v > 16.7).count(),
        "refresh_hz_est": if p50 > 0.0 { 1000.0 / p50 } else { 0.0 },
    })
}

fn tick(window: &mut Window, cx: &mut App, view_id: EntityId) {
    let mut guard = PROBE.lock().unwrap();
    let Some(state) = guard.as_mut() else {
        return;
    };
    let now = Instant::now();
    let elapsed = now.duration_since(state.last);
    state.last = now;
    if elapsed < Duration::from_secs(1) {
        state.samples.push(elapsed.as_secs_f64() * 1000.0);
    }
    if now.duration_since(state.started) >= WINDOW {
        let samples = std::mem::take(&mut state.samples);
        let path = state.path.clone();
        let scenario = state.scenario;
        *guard = None;
        drop(guard);
        let summary = summary(&samples);
        eprintln!("frame probe [{}]: {summary} -> {}", scenario.name(), path.display());
        let value = serde_json::json!({
            "scenario": scenario.name(),
            "summary": summary,
            "samples_ms": samples,
        });
        let _ = std::fs::write(&path, value.to_string());
        return;
    }
    drop(guard);
    cx.notify(view_id);
    window.on_next_frame(move |window, cx| tick(window, cx, view_id));
}
