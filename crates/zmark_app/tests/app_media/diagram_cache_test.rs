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
    cache.finish_frame(None);
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
    cache.finish_frame(None);
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
        cache.finish_frame(None);
    }
    assert_eq!(renders, 3, "each visible diagram renders once, not once per frame");
    assert!(files.iter().all(|f| f.exists()), "visible diagram files must survive");

    // No insertion is needed: a completed frame that no longer uses the third
    // diagram must bring the cache back under budget.
    cache.begin_frame();
    for i in 0..2 { cache.get(&(i as u64 + 1, false)); }
    cache.finish_frame(None);
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.take_evicted(), vec![files[2].clone()]);
    assert!(!files[2].exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn offscreen_lightbox_source_stays_pinned_until_close() {
    let dir = tempfile::tempdir().unwrap();
    let svg = r#"<svg width="500" height="500"></svg>"#;
    let paths: Vec<_> = (0..2).map(|i| dir.path().join(format!("{i}.svg"))).collect();
    let mut cache = DiagramCache::with_byte_budget(10, 1);
    for (i, path) in paths.iter().enumerate() {
        std::fs::write(path, svg).unwrap();
        cache.insert((i as u64, false), path.clone());
    }
    cache.finish_frame(None); // both are visible, even over budget
    assert!(cache.take_evicted().is_empty());
    cache.begin_frame();
    cache.finish_frame(Some(&paths[0]));
    assert!(paths[0].exists());
    assert_eq!(cache.take_evicted(), vec![paths[1].clone()]);
    cache.begin_frame();
    cache.finish_frame(None); // lightbox closed; source is now reclaimable
    assert_eq!(cache.take_evicted(), vec![paths[0].clone()]);
    assert_eq!(cache.len(), 0);
}

#[test]
fn hidden_decoded_assets_are_released_once_without_deleting_svg_sources() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<_> = (0..3).map(|i| dir.path().join(format!("{i}.svg"))).collect();
    let mut cache = DiagramCache::new(16);
    for (index, path) in paths.iter().enumerate() {
        std::fs::write(path, r#"<svg width="20" height="20"></svg>"#).unwrap();
        cache.insert((index as u64, false), path.clone());
    }
    cache.begin_frame();
    cache.get(&(0, false)); // visible in the new document
    cache.finish_frame(Some(&paths[1])); // the lightbox still owns this one
    assert_eq!(cache.take_hidden_images(Some(&paths[1])), vec![paths[2].clone()]);
    assert!(cache.take_hidden_images(Some(&paths[1])).is_empty());
    assert!(paths.iter().all(|path| path.exists()));
    assert!(cache.take_evicted().is_empty(), "keep SVG assets and Mermaid layout results");

    cache.begin_frame(); // pure source, no lightbox or preview
    let mut released = cache.take_hidden_images(None);
    released.sort();
    assert_eq!(released, paths[..2]);
    assert!(cache.take_hidden_images(None).is_empty());
    assert_eq!(cache.get(&(2, false)), Some(paths[2].clone()), "returning to preview reuses the same SVG");
    cache.begin_frame();
    assert_eq!(cache.take_hidden_images(None), vec![paths[2].clone()], "a reloaded bitmap may be released again");
}
