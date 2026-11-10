use zmark_app::views::preview::DiagramCache;

#[test]
fn test_diagram_cache_lru_cap_and_eviction() {
    let temp_dir = std::env::temp_dir().join("zmark_test_cache");
    let _ = std::fs::create_dir_all(&temp_dir);

    let file1 = temp_dir.join("test1.svg");
    let file2 = temp_dir.join("test2.svg");
    let file3 = temp_dir.join("test3.svg");
    std::fs::write(&file1, "<svg>1</svg>").unwrap();
    std::fs::write(&file2, "<svg>2</svg>").unwrap();
    std::fs::write(&file3, "<svg>3</svg>").unwrap();

    let mut cache = DiagramCache::new(2);
    cache.insert((1, false), file1.clone());
    cache.insert((2, false), file2.clone());
    assert_eq!(cache.len(), 2);

    // Next frame: only 1 is requested again, so 2 becomes the LRU item.
    cache.begin_frame();
    assert_eq!(cache.get(&(1, false)), Some(file1.clone()));

    // Insert 3 -> 2 should be evicted and file2 deleted
    cache.insert((3, false), file3.clone());
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get(&(2, false)), None);
    assert!(!file2.exists(), "Evicted file should be removed from disk");
    assert_eq!(cache.get(&(1, false)), Some(file1.clone()));
    assert_eq!(cache.get(&(3, false)), Some(file3.clone()));

    let evicted = cache.take_evicted();
    assert_eq!(evicted, vec![file2.clone()], "evicted path must be queued for texture release");
    assert!(!cache.has_pending_release());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn diagram_cache_enforces_a_texture_byte_budget() {
    let temp_dir = std::env::temp_dir().join("zmark_test_cache_bytes");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let files: Vec<_> = (1..=3)
        .map(|index| temp_dir.join(format!("test{index}.svg")))
        .collect();
    for file in &files {
        // 500x500 logical -> 1000x1000 at GPUI's 2x SVG scale -> 4 MB RGBA.
        std::fs::write(file, r#"<svg width="500" height="500"></svg>"#).unwrap();
    }

    let mut cache = DiagramCache::with_byte_budget(10, 8_500_000);
    cache.insert((1, false), files[0].clone());
    cache.insert((2, false), files[1].clone());
    assert_eq!(cache.len(), 2);

    // Next frame: 1 scrolled out of view. The third texture pushes the cache
    // over budget, so the oldest unused entry is evicted.
    cache.begin_frame();
    cache.get(&(2, false));
    cache.insert((3, false), files[2].clone());
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get(&(1, false)), None);
    assert!(!files[0].exists());
    assert_eq!(cache.take_evicted(), vec![files[0].clone()]);

    // A single texture larger than the whole budget must still be shown.
    let mut single = DiagramCache::with_byte_budget(10, 1);
    single.insert((9, false), files[2].clone());
    assert_eq!(single.len(), 1);
    assert!(single.take_evicted().is_empty());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// Mirrors the preview loop: each frame starts a new frame, then looks up every
/// visible diagram and re-renders (writes + inserts) the ones that miss.
#[test]
fn visible_diagrams_over_budget_are_kept_and_not_rerendered_every_frame() {
    let temp_dir = std::env::temp_dir().join("zmark_test_cache_visible");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let files: Vec<_> = (1..=3).map(|i| temp_dir.join(format!("t{i}.svg"))).collect();
    let svg = r#"<svg width="500" height="500"></svg>"#; // 4 MB decoded each
    // Budget fits two; all three are on screen.
    let mut cache = DiagramCache::with_byte_budget(10, 8_500_000);
    let mut renders = 0;
    for _ in 0..6 {
        cache.begin_frame();
        let _ = cache.take_evicted();
        for (i, file) in files.iter().enumerate() {
            let key = (i as u64 + 1, false);
            if cache.get(&key).is_none() || !file.exists() {
                renders += 1;
                std::fs::write(file, svg).unwrap();
                cache.insert(key, file.clone());
            }
        }
    }
    assert_eq!(renders, 3, "each visible diagram renders once, not once per frame");
    assert!(files.iter().all(|f| f.exists()), "visible diagram files must survive");

    // Once the third scrolls out of view, the cache shrinks back under budget.
    cache.begin_frame();
    for i in 0..2 {
        cache.get(&(i as u64 + 1, false));
    }
    cache.insert((4, false), {
        let f = temp_dir.join("t4.svg");
        std::fs::write(&f, svg).unwrap();
        f
    });
    cache.begin_frame();
    cache.insert((4, false), temp_dir.join("t4.svg"));
    assert!(cache.len() <= 2, "offscreen entries are evicted, got {}", cache.len());

    let _ = std::fs::remove_dir_all(&temp_dir);
}
