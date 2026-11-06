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

    // Access 1 so 2 becomes the LRU item
    assert_eq!(cache.get(&(1, false)), Some(file1.clone()));

    // Insert 3 -> 2 should be evicted and file2 deleted
    cache.insert((3, false), file3.clone());
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get(&(2, false)), None);
    assert!(!file2.exists(), "Evicted file should be removed from disk");
    assert_eq!(cache.get(&(1, false)), Some(file1.clone()));
    assert_eq!(cache.get(&(3, false)), Some(file3.clone()));

    let _ = std::fs::remove_dir_all(&temp_dir);
}
