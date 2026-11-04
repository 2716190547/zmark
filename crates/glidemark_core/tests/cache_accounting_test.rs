use glidemark_core::syntax::{
    heap_usable_size, parse_code_spans, CacheEntry, CodeKey, SyntaxCache, SyntaxRole, SyntaxSpan,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// -----------------------------------------------------------------------------
// Counting Allocator for Cache Accounting Test
// -----------------------------------------------------------------------------

extern "C" {
    fn malloc_size(ptr: *const std::ffi::c_void) -> usize;
}

struct CountingAlloc;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            let sz = unsafe { malloc_size(ptr as *const _) };
            ALLOCATED.fetch_add(sz, Ordering::SeqCst);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if !ptr.is_null() {
            let sz = unsafe { malloc_size(ptr as *const _) };
            ALLOCATED.fetch_sub(sz, Ordering::SeqCst);
        }
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old_sz = if ptr.is_null() { 0 } else { unsafe { malloc_size(ptr as *const _) } };
        let new_ptr = System.realloc(ptr, layout, new_size);
        if !new_ptr.is_null() {
            let new_sz = unsafe { malloc_size(new_ptr as *const _) };
            ALLOCATED.fetch_add(new_sz, Ordering::SeqCst);
            ALLOCATED.fetch_sub(old_sz, Ordering::SeqCst);
        }
        new_ptr
    }
}

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn current_allocated() -> usize {
    ALLOCATED.load(Ordering::SeqCst)
}

// -----------------------------------------------------------------------------
// Test A2: Cache Accounting & Ledger
// -----------------------------------------------------------------------------

#[test]
fn test_stage_a_cache_accounting_and_ledger() {
    // Warm-up to initialize any lazy statics/SyntaxSet before taking allocation measurements
    let _ = parse_code_spans("fn init() {}", Some("rust"));

    // 1. Key sharing: cloning CodeKey does not allocate duplicate string buffers
    {
        let payload: Arc<str> = Arc::from("shared_payload_string_buffer_check");
        let k1 = CodeKey::new(Some("Rust"), payload.clone());
        let before_alloc = current_allocated();
        let k2 = k1.clone();
        let after_alloc = current_allocated();
        assert_eq!(
            after_alloc.saturating_sub(before_alloc),
            0,
            "Cloning CodeKey sharing Arc must not allocate duplicate string buffers"
        );
        assert!(Arc::ptr_eq(&k1.payload, &k2.payload));
        assert_eq!(k1, k2);
    }

    // 2. Single entry: verify reported cache.current_bytes() accurately reflects
    // entry_heap_bytes + container overhead within [1.0, 1.15] ratio
    {
        let mut cache = SyntaxCache::new();
        let code = "fn compute_sum(a: i32, b: i32) -> i32 { a + b }";
        let spans_vec = parse_code_spans(code, Some("rust"));
        let spans_slice = spans_vec.as_slice();

        let before_insert = current_allocated();
        let payload: Arc<str> = Arc::from(code);
        let key = CodeKey::new(Some("Rust"), payload);
        let spans: Arc<[SyntaxSpan]> = Arc::from(spans_slice);
        let admitted = cache.insert(key, spans, &HashSet::new());
        let after_insert = current_allocated();
        assert!(admitted, "Single entry within budget must be admitted");

        let measured_delta = after_insert.saturating_sub(before_insert);
        let reported_bytes = cache.current_bytes();
        println!("Single entry: measured_delta={}, reported_bytes={}", measured_delta, reported_bytes);
        let ratio = measured_delta as f64 / reported_bytes as f64;
        assert!(
            ratio >= 0.95 && ratio <= 1.15,
            "Single entry heap-to-reported ratio must be within [0.95, 1.15], got: ratio={}, measured={}, reported={}",
            ratio,
            measured_delta,
            reported_bytes
        );
    }

    // 3. 6 distinct entries: verify accounting scales accurately
    {
        let mut cache = SyntaxCache::new();
        let mut raw_data = Vec::with_capacity(6);

        for i in 0..6 {
            let code = format!("fn distinct_fn_{}() -> usize {{ let x = {}; x * 2 }}", i, i * 100);
            let spans_vec = parse_code_spans(&code, Some("rust"));
            raw_data.push((code, spans_vec));
        }

        let before_insert = current_allocated();
        for (code, spans_vec) in &raw_data {
            let payload: Arc<str> = Arc::from(code.as_str());
            let key = CodeKey::new(Some("Rust"), payload);
            let spans: Arc<[SyntaxSpan]> = Arc::from(spans_vec.as_slice());
            let admitted = cache.insert(key, spans, &HashSet::new());
            assert!(admitted);
        }
        let after_insert = current_allocated();

        let measured_delta = after_insert.saturating_sub(before_insert);
        let reported_bytes = cache.current_bytes();
        let ratio = measured_delta as f64 / reported_bytes as f64;
        println!("6 entries: measured_delta={}, reported_bytes={}, ratio={}", measured_delta, reported_bytes, ratio);
        assert!(
            ratio >= 0.95 && ratio <= 1.15,
            "6 entries heap-to-reported ratio must be within [0.95, 1.15], got: ratio={}, measured={}, reported={}",
            ratio,
            measured_delta,
            reported_bytes
        );
    }

    // 4. LRU eviction: oldest non-retained item is evicted when memory approaches max_bytes
    {
        let mut cache = SyntaxCache::new();
        let sample_spans: Arc<[SyntaxSpan]> = Arc::new([SyntaxSpan {
            range: 0..5,
            role: SyntaxRole::Plain,
        }]);

        let key1 = SyntaxCache::key("code_block_1", Some("rust"));
        let key2 = SyntaxCache::key("code_block_2", Some("rust"));
        let key3 = SyntaxCache::key("code_block_3", Some("rust"));

        let heap1 = SyntaxCache::entry_heap_bytes(&key1, &sample_spans);
        let heap2 = SyntaxCache::entry_heap_bytes(&key2, &sample_spans);
        let container_overhead = heap_usable_size(2 * std::mem::size_of::<CacheEntry>());
        cache.max_bytes = container_overhead + heap1 + heap2; // exactly fits 2 entries

        let retained = HashSet::new();
        assert!(cache.insert(key1.clone(), sample_spans.clone(), &retained));
        assert!(cache.insert(key2.clone(), sample_spans.clone(), &retained));
        assert_eq!(cache.entries.len(), 2);

        // Insert key3 -> key1 (oldest non-retained) must be evicted
        assert!(cache.insert(key3.clone(), sample_spans.clone(), &retained));
        assert_eq!(cache.entries.len(), 2);
        assert!(cache.get(&key1).is_none(), "Oldest key1 must be evicted");
        assert!(cache.get(&key2).is_some(), "Key2 must be present");
        assert!(cache.get(&key3).is_some(), "Key3 must be present");
    }

    // 5. Retained protection: retained keys are protected from eviction
    {
        let mut cache = SyntaxCache::new();
        let sample_spans: Arc<[SyntaxSpan]> = Arc::new([SyntaxSpan {
            range: 0..5,
            role: SyntaxRole::Plain,
        }]);

        let key1 = SyntaxCache::key("code_block_1", Some("rust"));
        let key2 = SyntaxCache::key("code_block_2", Some("rust"));
        let key3 = SyntaxCache::key("code_block_3", Some("rust"));

        let heap1 = SyntaxCache::entry_heap_bytes(&key1, &sample_spans);
        let heap2 = SyntaxCache::entry_heap_bytes(&key2, &sample_spans);
        let container_overhead = heap_usable_size(2 * std::mem::size_of::<CacheEntry>());
        cache.max_bytes = container_overhead + heap1 + heap2;

        let mut retained = HashSet::new();
        assert!(cache.insert(key1.clone(), sample_spans.clone(), &retained));
        assert!(cache.insert(key2.clone(), sample_spans.clone(), &retained));

        // Retain key1
        retained.insert(key1.clone());

        // Insert key3 -> key2 (younger than key1, but key1 is retained!) must be evicted
        assert!(cache.insert(key3.clone(), sample_spans.clone(), &retained));
        assert_eq!(cache.entries.len(), 2);
        assert!(cache.get(&key1).is_some(), "Retained key1 must remain protected in cache");
        assert!(cache.get(&key2).is_none(), "Non-retained key2 must be evicted");
        assert!(cache.get(&key3).is_some(), "Newly admitted key3 must be present");
    }

    // 6. Admission rejection: when cache cannot fit an entry even after evicting non-retained items
    {
        let mut cache = SyntaxCache::new();
        let sample_spans: Arc<[SyntaxSpan]> = Arc::new([SyntaxSpan {
            range: 0..5,
            role: SyntaxRole::Plain,
        }]);

        let key1 = SyntaxCache::key("large_code_block", Some("rust"));
        let entry_heap = SyntaxCache::entry_heap_bytes(&key1, &sample_spans);
        cache.max_bytes = entry_heap; // Not enough for entry_heap + size_of::<CacheEntry>() container overhead

        let retained = HashSet::new();
        let admitted = cache.insert(key1, sample_spans.clone(), &retained);
        assert!(!admitted, "insert must return false when entry size exceeds max_bytes");

        // When all current entries are retained and space is insufficient
        let mut cache2 = SyntaxCache::new();
        let k_a = SyntaxCache::key("code_a", Some("rust"));
        let k_b = SyntaxCache::key("code_b", Some("rust"));
        let heap_a = SyntaxCache::entry_heap_bytes(&k_a, &sample_spans);
        let overhead = heap_usable_size(std::mem::size_of::<CacheEntry>());
        cache2.max_bytes = overhead + heap_a; // can only hold 1 entry

        let mut all_retained = HashSet::new();
        assert!(cache2.insert(k_a.clone(), sample_spans.clone(), &all_retained));
        all_retained.insert(k_a.clone());

        let admitted2 = cache2.insert(k_b, sample_spans, &all_retained);
        assert!(!admitted2, "insert must return false when non-retained eviction cannot free enough space");
    }

    // 7. Bounded boundary test: small max_bytes (200,000) filled to capacity, verify real drop differential
    {
        let mut cache = SyntaxCache::new();
        cache.max_bytes = 200_000;
        let retained = HashSet::new();
        let base = current_allocated();
        let mut i = 0;
        while i < 2000 {
            let code = format!("fn f{}() {{ let s = \"x{}\"; let n = {}; }}\n", i, i, i);
            let spans = parse_code_spans(&code, Some("rust"));
            let key = CodeKey::new(
                glidemark_core::syntax::resolve_canonical_language(Some("rust")),
                Arc::from(code.as_str()),
            );
            if cache.get(&key).is_some() {
                i += 1;
                continue;
            }
            if !cache.insert(key, Arc::from(spans), &retained) {
                break;
            }
            i += 1;
        }
        let during = current_allocated();
        let ledger = cache.current_bytes();
        drop(cache);
        let after = current_allocated();
        let owned_real = during.saturating_sub(after);
        let _residual = after.saturating_sub(base);

        assert!(
            owned_real <= 200_000,
            "Actual cache-owned bytes ({}) must not exceed max_bytes (200,000)",
            owned_real
        );
        let diff = (owned_real as i64 - ledger as i64).abs();
        assert!(
            diff <= 256,
            "Cache owned real ({}) must match ledger ({}) within tight tolerance (diff={})",
            owned_real,
            ledger,
            diff
        );
    }
}
