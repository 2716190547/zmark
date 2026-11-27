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
}

impl Scenario {
    pub fn name(self) -> &'static str {
        match self {
            Scenario::Modes => "modes",
            Scenario::DragScroll => "drag-scroll",
            Scenario::TableSelect => "table-select",
            Scenario::Resize => "resize",
        }
    }

    /// Document a scenario opens instead of the user's session. `None` keeps
    /// the normal startup document.
    pub fn document(self) -> Option<String> {
        match self {
            Scenario::Modes => None,
            Scenario::DragScroll | Scenario::Resize => Some(long_document(12_000)),
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
    // A background/occluded window may not receive display-link frames.
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
            view.update(cx, |ws, cx| {
                ws.set_view_mode(ViewMode::Preview, cx);
                ws.parse_now_and_refresh();
                cx.notify();
            });
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
                        let _ = cx.update(|window, cx| drive(scenario, step, pane, window, cx));
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
    eprintln!(
        "frame probe [{}]: selection {:?}, scroll top block {}",
        scenario.name(),
        selection,
        top.item_ix
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
