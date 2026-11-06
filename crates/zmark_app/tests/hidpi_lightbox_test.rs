use zmark_app::state::AppState;
use zmark_app::views::lightbox::{
    release_lightbox_image, LightboxDiagramCache, LightboxState,
    TEST_DROP_IMAGE_COUNT, TEST_RELEASE_CALL_COUNT,
};
use zmark_app::views::workspace::{resolve_lightbox_svg_path, WorkspaceView};
use zmark_core::mermaid::{
    apply_texture_pixel_budget, calculate_hidpi_dimensions,
    GPUI_SVG_DECODER_SCALE,
};
use gpui::{ImgResourceLoader, Resource, TestAppContext};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_hidpi_dimensions_multi_scale() {
    let scales = [1.0f32, 1.25f32, 1.5f32, 2.0f32, 3.0f32];
    for &scale in &scales {
        let (w, h) = calculate_hidpi_dimensions(1200.0, 800.0, scale);
        let physical_w = w * GPUI_SVG_DECODER_SCALE;
        let physical_h = h * GPUI_SVG_DECODER_SCALE;
        assert!(
            physical_w * physical_h <= 16_000_000.0 + 1e-4,
            "16MP budget exceeded at scale {}: {}x{} = {}",
            scale,
            physical_w,
            physical_h,
            physical_w * physical_h
        );
        assert!(
            physical_w <= 4000.0 + 1e-4 && physical_h <= 4000.0 + 1e-4,
            "max side <= 4000 exceeded at scale {}",
            scale
        );
    }

    // Negative / zero scale factor fallback (<= 0 -> 1.0)
    let (w_neg, h_neg) = calculate_hidpi_dimensions(1000.0, 600.0, -2.0);
    let (w_zero, h_zero) = calculate_hidpi_dimensions(1000.0, 600.0, 0.0);
    let (w_1, h_1) = calculate_hidpi_dimensions(1000.0, 600.0, 1.0);
    assert_eq!((w_neg, h_neg), (w_1, h_1));
    assert_eq!((w_zero, h_zero), (w_1, h_1));

    // Extreme input (10000x8000) constrained to <= 16MP and max side 4000
    let (ext_w, ext_h) = calculate_hidpi_dimensions(10000.0, 8000.0, 2.0);
    let phys_w = ext_w * GPUI_SVG_DECODER_SCALE;
    let phys_h = ext_h * GPUI_SVG_DECODER_SCALE;
    assert!(phys_w * phys_h <= 16_000_000.0 + 1e-4);
    assert!(phys_w <= 4000.0 + 1e-4);
    assert!(phys_h <= 4000.0 + 1e-4);
}

#[test]
fn test_apply_texture_pixel_budget_edge_cases() {
    // a) width="100%" and height="100%"
    let svg_percent =
        r#"<svg width="100%" height="100%" viewBox="0 0 1000 600"><circle cx="10" cy="10" r="5"/></svg>"#;
    let res_pct = apply_texture_pixel_budget(svg_percent, 1600.0, 960.0);
    assert!(res_pct.contains(r#"width="1600.00""#));
    assert!(res_pct.contains(r#"height="960.00""#));
    assert!(res_pct.contains(r#"viewBox="0 0 1000 600""#));
    assert!(!res_pct.contains("100%"));

    // b) missing width/height
    let svg_no_wh =
        r#"<svg viewBox="0 0 800 500"><rect x="0" y="0" width="100" height="50"/></svg>"#;
    let res_no_wh = apply_texture_pixel_budget(svg_no_wh, 1200.0, 750.0);
    assert!(res_no_wh.contains(r#"width="1200.00""#));
    assert!(res_no_wh.contains(r#"height="750.00""#));
    assert!(res_no_wh.contains(r#"viewBox="0 0 800 500""#));

    // c) only viewBox
    let svg_only_vb =
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 480"><path d="M0,0"/></svg>"#;
    let res_only_vb = apply_texture_pixel_budget(svg_only_vb, 960.0, 720.0);
    assert!(res_only_vb.contains(r#"width="960.00""#));
    assert!(res_only_vb.contains(r#"height="720.00""#));
    assert!(res_only_vb.contains(r#"viewBox="0 0 640 480""#));
}

#[test]
fn test_lightbox_diagram_cache_lru1_eviction_and_file_cleanup() {
    let dir = std::env::temp_dir().join("zmark_test_lru1");
    let _ = std::fs::create_dir_all(&dir);
    let file1 = dir.join("diagram_1.svg");
    let file2 = dir.join("diagram_2.svg");
    std::fs::write(&file1, "<svg id=\"1\"></svg>").unwrap();
    std::fs::write(&file2, "<svg id=\"2\"></svg>").unwrap();

    assert!(file1.exists());
    assert!(file2.exists());

    let mut cache = LightboxDiagramCache::new(1);

    cache.insert((1, false, 2), file1.clone());
    assert!(file1.exists());
    assert_eq!(cache.get(&(1, false, 2)), Some(file1.clone()));

    cache.insert((2, false, 2), file2.clone());
    // File 1 must be evicted and deleted from disk
    assert!(!file1.exists(), "Evicted file1 should be deleted from disk");
    assert!(file2.exists(), "Active file2 should remain on disk");
    assert_eq!(cache.get(&(1, false, 2)), None);
    assert_eq!(cache.get(&(2, false, 2)), Some(file2.clone()));

    // Cleanup
    let _ = std::fs::remove_file(file2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_open_lightbox_uses_same_is_dark_variant_as_preview() {
    let code_hash: u64 = 0xa1b2c3d4e5f60718;
    let temp_dir = std::env::temp_dir().join("zmark_test_is_dark_match");
    let _ = std::fs::create_dir_all(&temp_dir);

    let light_svg_path = temp_dir.join(format!("{:016x}_light.svg", code_hash));
    let dark_svg_path = temp_dir.join(format!("{:016x}_dark.svg", code_hash));

    std::fs::write(&light_svg_path, r##"<svg id="preview_light"><rect fill="#ffffff"/></svg>"##).unwrap();
    std::fs::write(&dark_svg_path, r##"<svg id="preview_dark"><rect fill="#1e1e1e"/></svg>"##).unwrap();

    // 1. Dark mode active: even when called with light_svg_path, resolving must return the dark variant
    let resolved_dark = resolve_lightbox_svg_path(&light_svg_path, code_hash, true);
    assert_eq!(resolved_dark, dark_svg_path);
    let resolved_dark_content = std::fs::read_to_string(&resolved_dark).unwrap();
    assert!(resolved_dark_content.contains("preview_dark"));
    assert!(!resolved_dark_content.contains("preview_light"));

    // 2. Light mode active: even when called with dark_svg_path, resolving must return the light variant
    let resolved_light = resolve_lightbox_svg_path(&dark_svg_path, code_hash, false);
    assert_eq!(resolved_light, light_svg_path);
    let resolved_light_content = std::fs::read_to_string(&resolved_light).unwrap();
    assert!(resolved_light_content.contains("preview_light"));
    assert!(!resolved_light_content.contains("preview_dark"));

    // 3. Fallback when matching variant does not exist on disk: retains original svg_path safely
    let non_existent_hash: u64 = 0x9999999999999999;
    let fallback_path = temp_dir.join(format!("{:016x}_custom.svg", non_existent_hash));
    let resolved_fallback = resolve_lightbox_svg_path(&fallback_path, non_existent_hash, true);
    assert_eq!(resolved_fallback, fallback_path);

    // Clean up
    let _ = std::fs::remove_file(light_svg_path);
    let _ = std::fs::remove_file(dark_svg_path);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_lightbox_state_lifecycle_and_image_release() {
    let mut state = LightboxState::new();
    assert!(!state.is_open);
    assert_eq!(state.hidpi_svg_path, None);

    // Close when empty returns None
    assert_eq!(state.close(), None);

    // Open HiDPI
    let svg_path = PathBuf::from("/tmp/original.svg");
    let hidpi_path = PathBuf::from("/tmp/hidpi.svg");

    state.open_hidpi(
        svg_path.clone(),
        hidpi_path.clone(),
        9999,
    );
    assert!(state.is_open);
    assert_eq!(state.svg_path, Some(svg_path));
    assert_eq!(state.hidpi_svg_path, Some(hidpi_path.clone()));
    assert_eq!(state.code_hash, Some(9999));
    assert_eq!(state.zoom, 1.0);

    // First close returns old exclusive path
    let closed_img = state.close();
    assert_eq!(closed_img, Some(hidpi_path));
    assert!(!state.is_open);
    assert_eq!(state.hidpi_svg_path, None);

    // Second close returns None
    let second_closed = state.close();
    assert_eq!(second_closed, None);
}

#[test]
fn test_document_swapped_and_open_paths_release_vram_safely() {
    let mut state = LightboxState::new();

    // 1. Initial state has no image
    assert_eq!(state.close(), None);

    // 2. Open HiDPI image 1
    let svg1 = PathBuf::from("/tmp/doc1_diagram.svg");
    let hidpi1 = PathBuf::from("/tmp/doc1_hidpi.svg");
    let evicted0 = state.open_hidpi(svg1.clone(), hidpi1.clone(), 101);
    assert_eq!(evicted0, None, "Initial open_hidpi should have no evicted image");
    assert_eq!(state.hidpi_svg_path, Some(hidpi1.clone()));

    // 3. Open another diagram via plain open() without explicit close:
    // Must return hidpi1 so caller can invoke release
    let svg2 = PathBuf::from("/tmp/doc2_diagram.svg");
    let evicted1 = state.open(svg2.clone(), 102);
    assert_eq!(
        evicted1,
        Some(hidpi1),
        "Calling open() while holding an exclusive HiDPI image must return it for release"
    );
    assert_eq!(state.hidpi_svg_path, None);

    // 4. Open HiDPI image 2:
    // Previous state was plain open() (fallback/shared svg_path), so no exclusive image was held
    let hidpi2 = PathBuf::from("/tmp/doc2_hidpi.svg");
    let evicted2 = state.open_hidpi(svg2.clone(), hidpi2.clone(), 102);
    assert_eq!(evicted2, None, "Previous plain open had no exclusive HiDPI image to evict");
    assert_eq!(state.hidpi_svg_path, Some(hidpi2.clone()));

    // 5. Open HiDPI image 3 while image 2 is active:
    // Must return hidpi2 so the caller can invoke release
    let svg3 = PathBuf::from("/tmp/doc3_diagram.svg");
    let hidpi3 = PathBuf::from("/tmp/doc3_hidpi.svg");
    let evicted3 = state.open_hidpi(svg3.clone(), hidpi3.clone(), 103);
    assert_eq!(
        evicted3,
        Some(hidpi2),
        "Calling open_hidpi() while holding another exclusive HiDPI image must return it for release"
    );
    assert_eq!(state.hidpi_svg_path, Some(hidpi3.clone()));

    // 6. Document swapped simulation:
    // When document is swapped, close_lightbox() calls state.close()
    let swapped_evicted = state.close();
    assert_eq!(
        swapped_evicted,
        Some(hidpi3),
        "Document swapped must release the active exclusive HiDPI image"
    );
    assert!(!state.is_open);
    assert_eq!(state.hidpi_svg_path, None);
}

#[gpui::test]
async fn test_release_lightbox_image_texture_drop_and_asset_cache_behavior(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK.lock().unwrap();
    let temp_dir = std::env::temp_dir().join("zmark_test_release_behavior");
    let _ = std::fs::create_dir_all(&temp_dir);
    let sample_svg1 = temp_dir.join("sample1.svg");
    let sample_svg2 = temp_dir.join("sample2.svg");
    std::fs::write(&sample_svg1, r#"<svg width="100" height="100" xmlns="http://www.w3.org/2000/svg"><rect width="100" height="100" fill="red"/></svg>"#).unwrap();
    std::fs::write(&sample_svg2, r#"<svg width="50" height="50" xmlns="http://www.w3.org/2000/svg"><circle cx="25" cy="25" r="20" fill="blue"/></svg>"#).unwrap();

    let p1 = sample_svg1.clone();
    struct TestPathView {
        path: PathBuf,
    }
    impl gpui::Render for TestPathView {
        fn render(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
            gpui::img(self.path.clone())
        }
    }

    let (_view, cx) = cx.add_window_view(|_window, _cx| {
        TestPathView { path: p1 }
    });

    let initial_releases = TEST_RELEASE_CALL_COUNT.load(Ordering::SeqCst);
    let initial_drops = TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst);

    cx.update(|window, app| {
        let _ = window.draw(app);
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let res1 = Resource::Path(Arc::from(sample_svg1.as_path()));
        let cached1 = window.get_asset::<ImgResourceLoader>(&res1, app);
        assert!(cached1.is_some(), "get_asset should return cached render image after paint");

        // Negative control: sample_svg2 calls remove_asset -> does NOT drop texture via cx.drop_image
        let res2 = Resource::Path(Arc::from(sample_svg2.as_path()));
        let count_before_remove = TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst);
        app.remove_asset::<ImgResourceLoader>(&res2);
        assert_eq!(
            TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst),
            count_before_remove,
            "remove_asset alone must NOT trigger drop_image"
        );

        // Unified release function release_lightbox_image on sample_svg1:
        // Must call cx.drop_image AND remove_asset
        release_lightbox_image(&sample_svg1, window, app);

        assert_eq!(
            TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst),
            initial_drops + 1,
            "release_lightbox_image must call cx.drop_image"
        );
        assert_eq!(
            TEST_RELEASE_CALL_COUNT.load(Ordering::SeqCst),
            initial_releases + 1,
            "release_lightbox_image call count must increment"
        );
        // After release, asset cache is also cleared
        assert!(window.get_asset::<ImgResourceLoader>(&res1, app).is_none());
    });

    let _ = std::fs::remove_file(sample_svg1);
    let _ = std::fs::remove_file(sample_svg2);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[gpui::test]
async fn test_workspace_lightbox_three_paths_release_vram_and_atlas(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK.lock().unwrap();
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let state = AppState::new_isolated(zmark_core::document::Document::new_scratch());
        WorkspaceView::new(state, cx)
    });

    let temp_dir = std::env::temp_dir().join("zmark_test_three_paths");
    let _ = std::fs::create_dir_all(&temp_dir);
    let sample_svg = temp_dir.join("sample.svg");
    std::fs::write(&sample_svg, r#"<svg width="200" height="150" viewBox="0 0 200 150"><circle cx="50" cy="50" r="40"/></svg>"#).unwrap();

    // Record window_handle on workspace
    cx.update(|window, app| {
        view.update(app, |ws, _cx| {
            ws.window_handle = Some(window.window_handle());
        });
    });

    // Path 1: close_lightbox
    let drops_before = TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst);
    let releases_before = TEST_RELEASE_CALL_COUNT.load(Ordering::SeqCst);

    let path_p1 = cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.open_lightbox(sample_svg.clone(), 1001, window, cx);
            cx.notify();
        });
        let _ = window.draw(app);
        view.read(app).lightbox.displayed_path().cloned().expect("path must be open")
    });

    // Run layout & paint of WorkspaceView (which renders the open lightbox img element)
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let res = Resource::Path(Arc::from(path_p1.as_path()));
        assert!(window.get_asset::<ImgResourceLoader>(&res, app).is_some());

        // Call close_lightbox
        view.update(app, |ws, cx| {
            ws.close_lightbox(window, cx);
            cx.notify();
        });

        assert!(!view.read(app).lightbox.is_open);
        assert_eq!(TEST_RELEASE_CALL_COUNT.load(Ordering::SeqCst), releases_before + 1);
        assert_eq!(TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst), drops_before + 1);
        assert!(window.get_asset::<ImgResourceLoader>(&res, app).is_none());
    });

    // Path 2: open_lightbox replacement at top
    let drops_before = TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst);
    let releases_before = TEST_RELEASE_CALL_COUNT.load(Ordering::SeqCst);

    let path_p2a = cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.open_lightbox(sample_svg.clone(), 1002, window, cx);
            cx.notify();
        });
        let _ = window.draw(app);
        view.read(app).lightbox.displayed_path().cloned().expect("path 1002 must be open")
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let res2a = Resource::Path(Arc::from(path_p2a.as_path()));
        assert!(window.get_asset::<ImgResourceLoader>(&res2a, app).is_some());

        // Open another diagram while 1002 is still open:
        // Top of open_lightbox must close and release 1002!
        view.update(app, |ws, cx| {
            ws.open_lightbox(sample_svg.clone(), 1003, window, cx);
            cx.notify();
        });
        assert_eq!(TEST_RELEASE_CALL_COUNT.load(Ordering::SeqCst), releases_before + 1);
        assert_eq!(TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst), drops_before + 1);
        assert!(window.get_asset::<ImgResourceLoader>(&res2a, app).is_none());
    });

    // Path 3: on_document_swapped (with Some(window))
    let drops_before = TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst);
    let releases_before = TEST_RELEASE_CALL_COUNT.load(Ordering::SeqCst);

    let path_p3 = cx.update(|window, app| {
        let _ = window.draw(app);
        view.read(app).lightbox.displayed_path().cloned().expect("path 1003 must be open")
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let res3 = Resource::Path(Arc::from(path_p3.as_path()));
        assert!(window.get_asset::<ImgResourceLoader>(&res3, app).is_some());

        view.update(app, |ws, cx| {
            ws.on_document_swapped(Some(window), cx);
            cx.notify();
        });
        assert!(!view.read(app).lightbox.is_open);
        assert_eq!(TEST_RELEASE_CALL_COUNT.load(Ordering::SeqCst), releases_before + 1);
        assert_eq!(TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst), drops_before + 1);
        assert!(window.get_asset::<ImgResourceLoader>(&res3, app).is_none());
    });

    // Path 4: on_document_swapped (with None, using window_handle)
    let drops_before = TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst);
    let releases_before = TEST_RELEASE_CALL_COUNT.load(Ordering::SeqCst);

    let path_p4 = cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.open_lightbox(sample_svg.clone(), 1004, window, cx);
            cx.notify();
        });
        let _ = window.draw(app);
        view.read(app).lightbox.displayed_path().cloned().expect("path 1004 must be open")
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let res4 = Resource::Path(Arc::from(path_p4.as_path()));
        assert!(window.get_asset::<ImgResourceLoader>(&res4, app).is_some());
    });

    // Simulate async prompt return: view.update on AppContext without holding window
    view.update(cx, |ws, cx| {
        ws.on_document_swapped(None, cx);
    });

    cx.update(|window, app| {
        let res4 = Resource::Path(Arc::from(path_p4.as_path()));
        assert!(!view.read(app).lightbox.is_open);
        assert_eq!(TEST_RELEASE_CALL_COUNT.load(Ordering::SeqCst), releases_before + 1);
        assert_eq!(TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst), drops_before + 1);
        assert!(window.get_asset::<ImgResourceLoader>(&res4, app).is_none());
    });

    let _ = std::fs::remove_file(sample_svg);
    let _ = std::fs::remove_dir_all(&temp_dir);
}
