//! Opt-in frame-time probe for animation and scroll acceptance.
//!
//! Enable with `ZMARK_FRAME_PROBE=1` (or set it to an output path). For three
//! seconds every frame interval is recorded, then written once. `on_next_frame`
//! is one-shot, so each callback re-registers and requests the next frame.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use gpui::{App, EntityId, Window};

const WINDOW: Duration = Duration::from_secs(3);
const DEFAULT_PATH: &str = "/tmp/zmark_frames.json";

struct ProbeState {
    started: Instant,
    last: Instant,
    samples: Vec<f64>,
    path: PathBuf,
}

static PROBE: Mutex<Option<ProbeState>> = Mutex::new(None);

pub fn start(window: &mut Window, view_id: EntityId) {
    let Ok(setting) = std::env::var("ZMARK_FRAME_PROBE") else {
        return;
    };
    if setting.is_empty() || setting == "0" {
        return;
    }
    let path = if setting == "1" {
        PathBuf::from(DEFAULT_PATH)
    } else {
        PathBuf::from(setting)
    };
    let now = Instant::now();
    *PROBE.lock().unwrap() = Some(ProbeState {
        started: now,
        last: now,
        samples: Vec::new(),
        path,
    });
    // A background/occluded window may not receive display-link frames.
    window.activate_window();
    window.on_next_frame(move |window, cx| tick(window, cx, view_id));
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
        *guard = None;
        drop(guard);
        let value = serde_json::json!({ "samples_ms": samples });
        let _ = std::fs::write(&path, value.to_string());
        return;
    }
    drop(guard);
    cx.notify(view_id);
    window.on_next_frame(move |window, cx| tick(window, cx, view_id));
}
