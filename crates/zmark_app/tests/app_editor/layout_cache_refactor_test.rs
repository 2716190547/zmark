use zmark_app::views::editor::DocumentLayoutCache;
use gpui::TestAppContext;

#[gpui::test]
async fn unchanged_lines_reuse_owned_text_and_match_incremental_layout(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    cx.update(|window, _| {
        let lines: Vec<String> = (0..2000)
            .map(|i| format!("line {i}: {}", "text ".repeat(100)))
            .collect();
        let fenced = vec![false; lines.len()];
        let mut cache = DocumentLayoutCache::new();
        let expected = cache.update(window.text_system(), &lines, &fenced, 3000., 14.);
        let original_ptr = cache.entries[1000].text.as_ptr();
        // Two identical passes are enough: the second one is the incremental
        // path, and further repeats only re-assert the same result.
        let before = std::time::Instant::now();
        for _ in 0..2 {
            assert_eq!(
                cache.update(window.text_system(), &lines, &fenced, 3000., 14.),
                expected
            );
        }
        eprintln!(
            "cache_unchanged_2_ms={:.2}",
            before.elapsed().as_secs_f64() * 1000.
        );
        assert_eq!(
            cache.entries[1000].text.as_ptr(),
            original_ptr,
            "unchanged line must retain its text allocation"
        );
        let mut inserted = lines.clone();
        inserted.insert(100, "new line".into());
        let flags = vec![false; inserted.len()];
        let actual = cache.update(window.text_system(), &inserted, &flags, 3000., 14.);
        let mut fresh = DocumentLayoutCache::new();
        assert_eq!(
            actual,
            fresh.update(window.text_system(), &inserted, &flags, 3000., 14.)
        );
        assert_eq!(
            cache.entries[1001].text.as_ptr(),
            original_ptr,
            "suffix reuse must retain its text allocation"
        );
    });
}
