//! Run with `cargo run --release -p zmark_app --example editor_bench`.
use std::{alloc::{GlobalAlloc, Layout, System}, sync::atomic::{AtomicBool, AtomicUsize, Ordering}, time::Instant};
use zmark_app::{editor_engine::EditorEngine, state::AppState};
use zmark_core::document::Document;

struct Allocator;
static TRACK: AtomicBool = AtomicBool::new(false);
static BYTES: AtomicUsize = AtomicUsize::new(0);
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let result = System.alloc(layout);
        if TRACK.load(Ordering::Relaxed) && !result.is_null() { BYTES.fetch_add(layout.size(), Ordering::Relaxed); }
        result
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) { System.dealloc(pointer, layout); }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = System.realloc(pointer, layout, size);
        if TRACK.load(Ordering::Relaxed) && !result.is_null() { BYTES.fetch_add(size, Ordering::Relaxed); }
        result
    }
}

fn main() {
    println!("bytes,operation,allocated_bytes,p50_ns,p95_ns");
    for size in [10_000, 1_500_000] {
        let line = "日常编辑🙂 paragraph with **text** and `code`.\n";
        let text = line.repeat(size / line.len() + 1);
    for operation in ["insert", "insert_shared"] {
        let mut editor = EditorEngine::new(&text);
        editor.set_cursor(editor.lines.len() / 2, 3);
        let mut samples = Vec::with_capacity(100);
        BYTES.store(0, Ordering::Relaxed);
        for _ in 0..100 {
            let shared = (operation == "insert_shared").then(|| editor.lines.clone());
            let start = Instant::now();
            TRACK.store(true, Ordering::Relaxed);
            editor.insert_text("中");
            TRACK.store(false, Ordering::Relaxed);
            samples.push(start.elapsed().as_nanos());
            drop(shared);
            assert!(editor.undo());
            assert_eq!(editor.to_text(), text);
        }
        samples.sort_unstable();
        println!("{},{operation},{},{},{}", text.len(), BYTES.load(Ordering::Relaxed) / 100, samples[49], samples[94]);
        }

        let mut samples = Vec::with_capacity(100);
        let mut state = AppState::new_isolated(Document::new_scratch_with_text(text.clone()));
        state.editor.set_cursor(state.editor.lines.len() / 2, 3);
        samples.clear();
        BYTES.store(0, Ordering::Relaxed);
        for _ in 0..100 {
            state.editor.insert_text("中");
            state.mark_editor_dirty();
            let start = Instant::now();
            TRACK.store(true, Ordering::Relaxed);
            assert!(state.sync_from_editor());
            TRACK.store(false, Ordering::Relaxed);
            samples.push(start.elapsed().as_nanos());
            assert!(state.editor.undo());
            state.mark_editor_dirty();
            state.sync_from_editor();
            assert_eq!(state.doc().buffer, text);
        }
        samples.sort_unstable();
        println!("{},sync,{},{},{}", text.len(), BYTES.load(Ordering::Relaxed) / 100, samples[49], samples[94]);
    }
}
