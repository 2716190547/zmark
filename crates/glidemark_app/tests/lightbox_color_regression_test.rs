use glidemark_app::state::AppState;
use glidemark_app::views::lightbox::TEST_DROP_IMAGE_COUNT;
use glidemark_app::views::workspace::WorkspaceView;
use gpui::{App, Asset, ImageAssetLoader, ImgResourceLoader, Resource, TestAppContext, Window};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Retrieves the rendered image displayed by the lightbox.
///
/// Follows P2-3: single production path using GPUI's file `ImgResourceLoader`.
fn retrieve_lightbox_render_image(
    candidate_path: Option<&Path>,
    window: &mut Window,
    cx: &mut App,
) -> Option<Arc<gpui::RenderImage>> {
    let path = candidate_path?;
    let resource = Resource::Path(Arc::from(path));
    window
        .get_asset::<ImgResourceLoader>(&resource, cx)
        .and_then(|r| r.ok())
}

/// Helper to create a test SVG containing:
/// - Opaque purple `#9370db` (left half) -> BGRA [219, 112, 147, 255]
/// - Translucent `#ecf2dc` with opacity 0.6 (right half) -> BGRA [219, 241, 236, 153]
fn test_svg_content() -> &'static [u8] {
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="4" viewBox="0 0 8 4"><rect width="4" height="4" fill="#9370db"/><rect x="4" width="4" height="4" fill="#ecf2dc" fill-opacity="0.6"/></svg>"##
}

/// Helper for light theme SVG:
/// - Opaque purple `#9370db` -> BGRA [219, 112, 147, 255]
fn light_svg_content() -> &'static [u8] {
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="4" viewBox="0 0 8 4"><rect width="8" height="4" fill="#9370db"/></svg>"##
}

/// Helper for dark theme SVG:
/// - Opaque coral `#ff7f50` -> BGRA [80, 127, 255, 255] (R=255, G=127, B=80)
fn dark_svg_content() -> &'static [u8] {
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="4" viewBox="0 0 8 4"><rect width="8" height="4" fill="#ff7f50"/></svg>"##
}

#[gpui::test]
async fn test_lightbox_matches_preview_bgra_pixels_opaque_and_alpha(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut doc = glidemark_core::document::Document::new_scratch();
    doc.set_buffer("```mermaid\ngraph LR\n    A[#9370db] --> B[#ecf2dc]\n```".to_string());
    let state = AppState::new_isolated(doc);

    let (view, cx) = cx.add_window_view(|_window, cx| WorkspaceView::new(state, cx));

    cx.update(|window, app| {
        view.update(app, |ws, _cx| {
            ws.window_handle = Some(window.window_handle());
        });
    });

    let temp_dir = std::env::temp_dir().join("glidemark_test_color_opaque_alpha");
    let _ = std::fs::create_dir_all(&temp_dir);
    let code_hash: u64 = 0x9370db_ecf2dc_0001;
    let svg_path = temp_dir.join(format!("{:016x}_light.svg", code_hash));
    std::fs::write(&svg_path, test_svg_content()).unwrap();

    // 1. Verify preview rendered image decode produces correct BGRA pixels:
    // - Opaque purple: [219, 112, 147, 255]
    // - Translucent: [219, 241, 236, 153] (A around 153)
    let preview_resource = Resource::Path(Arc::from(svg_path.as_path()));
    let preview_future = cx.update(|_window, app| -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Arc<gpui::RenderImage>, gpui::ImageCacheError>> + Send>> {
        Box::pin(ImageAssetLoader::load(preview_resource, app))
    });
    let preview_render_image = preview_future
        .await
        .expect("preview SVG decode must succeed");

    let preview_bytes = preview_render_image
        .as_bytes(0)
        .expect("preview render image bytes");
    let preview_opaque = &preview_bytes[0..4];
    let preview_alpha = preview_bytes
        .chunks_exact(4)
        .find(|p| p[3] > 100 && p[3] < 200)
        .expect("preview translucent pixel with alpha ~153 must exist");

    assert_eq!(
        preview_opaque,
        &[219, 112, 147, 255],
        "preview purple must decode to BGRA [219, 112, 147, 255]"
    );
    assert_eq!(
        preview_alpha,
        &[219, 241, 236, 153],
        "preview translucent must decode to BGRA [219, 241, 236, 153]"
    );

    // 2. Open lightbox
    let candidate_path = cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.open_lightbox(svg_path.clone(), code_hash, window, cx);
            cx.notify();
        });
        view.read(app)
            .lightbox
            .displayed_path()
            .cloned()
            .expect("displayed_path must be set upon open")
    });

    // P2-4: Element-loaded proof.
    // BEFORE window.draw paints the img element, get_asset for candidate_path must be None!
    cx.update(|window, app| {
        let res = Resource::Path(Arc::from(candidate_path.as_path()));
        assert!(
            window.get_asset::<ImgResourceLoader>(&res, app).is_none(),
            "candidate asset must NOT be loaded before element paint (P2-4 proof)"
        );
        let _ = window.draw(app);
    });
    cx.run_until_parked();

    // 3. Retrieve rendered image displayed by lightbox after element paint
    let lightbox_render_image = cx
        .update(|window, app| {
            let _ = window.draw(app);
            retrieve_lightbox_render_image(Some(&candidate_path), window, app)
        })
        .expect("lightbox must have a rendered image after element draw");

    let lightbox_bytes = lightbox_render_image
        .as_bytes(0)
        .expect("lightbox render image bytes");
    let lightbox_opaque = &lightbox_bytes[0..4];
    let lightbox_alpha = lightbox_bytes
        .chunks_exact(4)
        .find(|p| p[3] > 100 && p[3] < 200)
        .expect("lightbox translucent pixel with alpha ~153 must exist");

    // 4. Asserts that lightbox pixels match preview's decoded BGRA pixels:
    // - Opaque purple must be [219, 112, 147, 255] (NOT RGBA [147, 112, 219, 255])
    // - Alpha pixel must be [219, 241, 236, 153] (NOT RGBA [142, 145, 132, 153])
    assert_eq!(
        lightbox_opaque, preview_opaque,
        "lightbox purple pixel must match preview decoded BGRA pixels"
    );
    assert_eq!(
        lightbox_opaque,
        &[219, 112, 147, 255],
        "lightbox purple pixel must be BGRA [219, 112, 147, 255] (NOT RGBA [147, 112, 219, 255])"
    );

    assert_eq!(
        lightbox_alpha, preview_alpha,
        "lightbox translucent pixel must match preview decoded BGRA pixels"
    );
    assert_eq!(
        lightbox_alpha,
        &[219, 241, 236, 153],
        "lightbox translucent pixel must be BGRA [219, 241, 236, 153] (NOT RGBA [142, 145, 132, 153])"
    );

    let _ = std::fs::remove_file(svg_path);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[gpui::test]
async fn test_lightbox_dark_and_light_theme_variants(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let state = AppState::new_isolated(glidemark_core::document::Document::new_scratch());
        WorkspaceView::new(state, cx)
    });

    cx.update(|window, app| {
        view.update(app, |ws, _cx| {
            ws.window_handle = Some(window.window_handle());
        });
    });

    let temp_dir = std::env::temp_dir().join("glidemark_test_theme_variants");
    let _ = std::fs::create_dir_all(&temp_dir);
    let code_hash: u64 = 0xabcdef0123456789;
    let light_svg_path = temp_dir.join(format!("{:016x}_light.svg", code_hash));
    let dark_svg_path = temp_dir.join(format!("{:016x}_dark.svg", code_hash));

    // Light: purple #9370db -> BGRA [219, 112, 147, 255]
    // Dark: coral #ff7f50 -> BGRA [80, 127, 255, 255]
    std::fs::write(&light_svg_path, light_svg_content()).unwrap();
    std::fs::write(&dark_svg_path, dark_svg_content()).unwrap();

    // 1. Light Mode: open_lightbox uses light SVG variant and decodes with purple BGRA
    cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.state.set_theme_mode(glidemark_core::config::ThemeMode::Light);
            assert!(!ws.theme(cx).is_dark, "theme must be light");
            ws.open_lightbox(light_svg_path.clone(), code_hash, window, cx);
            cx.notify();
        });
        let _ = window.draw(app);
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let candidate_path = view.read(app).lightbox.displayed_path().cloned();
        let img = retrieve_lightbox_render_image(candidate_path.as_deref(), window, app)
            .expect("light mode lightbox image rendered");
        let bytes = img.as_bytes(0).unwrap();
        let pixel = &bytes[0..4];

        assert_eq!(
            pixel,
            &[219, 112, 147, 255],
            "light mode purple must be BGRA [219, 112, 147, 255]"
        );
    });

    // 2. P2-5: Live Theme Switch while lightbox is open!
    // Changing theme from Light to Dark without closing lightbox must refresh displayed image to Dark variant
    cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.state.set_theme_mode(glidemark_core::config::ThemeMode::Dark);
            assert!(ws.theme(cx).is_dark, "theme must now be dark");
            cx.notify();
        });
        let _ = window.draw(app);
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let candidate_path = view.read(app).lightbox.displayed_path().cloned();
        let img = retrieve_lightbox_render_image(candidate_path.as_deref(), window, app)
            .expect("dark mode lightbox image rendered after live theme switch");
        let bytes = img.as_bytes(0).unwrap();
        let pixel = &bytes[0..4];

        assert_eq!(
            pixel,
            &[80, 127, 255, 255],
            "live switch to dark mode must render coral BGRA [80, 127, 255, 255]"
        );

        view.update(app, |ws, cx| {
            ws.close_lightbox(window, cx);
        });
    });

    let _ = std::fs::remove_file(light_svg_path);
    let _ = std::fs::remove_file(dark_svg_path);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[gpui::test]
async fn test_lightbox_three_release_paths_drop_texture_and_remove_asset(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let state = AppState::new_isolated(glidemark_core::document::Document::new_scratch());
        WorkspaceView::new(state, cx)
    });

    let temp_dir = std::env::temp_dir().join("glidemark_test_three_release_paths");
    let _ = std::fs::create_dir_all(&temp_dir);
    let sample_svg = temp_dir.join("sample.svg");
    std::fs::write(&sample_svg, test_svg_content()).unwrap();

    cx.update(|window, app| {
        view.update(app, |ws, _cx| {
            ws.window_handle = Some(window.window_handle());
        });
    });

    let check_asset_purged = |path: Option<&Path>, window: &mut Window, app: &mut App| {
        if let Some(p) = path {
            let resource = Resource::Path(Arc::from(p));
            assert!(
                window.get_asset::<ImgResourceLoader>(&resource, app).is_none(),
                "asset for path {:?} must be purged from ImgResourceLoader",
                p
            );
        }
    };

    // --- Path 1: close_lightbox ---
    let drops_before = TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst);
    let p1_path = cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.open_lightbox(sample_svg.clone(), 3001, window, cx);
            cx.notify();
        });
        let _ = window.draw(app);
        view.read(app).lightbox.displayed_path().cloned()
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let has_image = p1_path.as_ref().and_then(|p| {
            window.get_asset::<ImgResourceLoader>(&Resource::Path(Arc::from(p.as_path())), app)
        }).is_some();
        assert!(has_image, "image 3001 must be loaded in cache before close");

        view.update(app, |ws, cx| {
            ws.close_lightbox(window, cx);
            cx.notify();
        });

        assert!(!view.read(app).lightbox.is_open, "lightbox must be closed");
        assert_eq!(
            TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst),
            drops_before + 1,
            "close_lightbox must increment TEST_DROP_IMAGE_COUNT"
        );

        check_asset_purged(p1_path.as_deref(), window, app);
    });

    // --- Path 2: replacement open_lightbox ---
    let drops_before = TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst);
    let p2_path = cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.open_lightbox(sample_svg.clone(), 3002, window, cx);
            cx.notify();
        });
        let _ = window.draw(app);
        view.read(app).lightbox.displayed_path().cloned()
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let has_image = p2_path.as_ref().and_then(|p| {
            window.get_asset::<ImgResourceLoader>(&Resource::Path(Arc::from(p.as_path())), app)
        }).is_some();
        assert!(has_image, "image 3002 must be loaded before replacement");

        // Open 3003 while 3002 is still open: must drop 3002!
        view.update(app, |ws, cx| {
            ws.open_lightbox(sample_svg.clone(), 3003, window, cx);
            cx.notify();
        });

        assert_eq!(
            TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst),
            drops_before + 1,
            "replacement open_lightbox must increment TEST_DROP_IMAGE_COUNT"
        );

        check_asset_purged(p2_path.as_deref(), window, app);
    });

    // --- Path 3: on_document_swapped ---
    cx.update(|window, app| {
        let _ = window.draw(app);
    });
    cx.run_until_parked();

    let drops_before = TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst);
    let p3_path = cx.update(|_window, app| {
        view.read(app).lightbox.displayed_path().cloned()
    });

    cx.update(|window, app| {
        let has_image = p3_path.as_ref().and_then(|p| {
            window.get_asset::<ImgResourceLoader>(&Resource::Path(Arc::from(p.as_path())), app)
        }).is_some();
        assert!(has_image, "image 3003 must be loaded before document swap");

        view.update(app, |ws, cx| {
            ws.on_document_swapped(Some(window), cx);
            cx.notify();
        });

        assert!(
            !view.read(app).lightbox.is_open,
            "lightbox must be closed after document swap"
        );
        assert_eq!(
            TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst),
            drops_before + 1,
            "on_document_swapped must increment TEST_DROP_IMAGE_COUNT"
        );

        check_asset_purged(p3_path.as_deref(), window, app);
    });

    let _ = std::fs::remove_file(sample_svg);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[gpui::test]
async fn test_actual_decoded_render_image_budget(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let state = AppState::new_isolated(glidemark_core::document::Document::new_scratch());
        WorkspaceView::new(state, cx)
    });

    cx.update(|window, app| {
        view.update(app, |ws, _cx| {
            ws.window_handle = Some(window.window_handle());
        });
    });

    let temp_dir = std::env::temp_dir().join("glidemark_test_decoded_budget");
    let _ = std::fs::create_dir_all(&temp_dir);

    let normal_svg = temp_dir.join("normal.svg");
    std::fs::write(
        &normal_svg,
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="600" viewBox="0 0 1000 600"><rect width="1000" height="600" fill="#9370db"/></svg>"##,
    )
    .unwrap();

    let ultrawide_svg = temp_dir.join("ultrawide.svg");
    std::fs::write(
        &ultrawide_svg,
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="3000" height="100" viewBox="0 0 3000 100"><rect width="3000" height="100" fill="#9370db"/></svg>"##,
    )
    .unwrap();

    let ultratall_svg = temp_dir.join("ultratall.svg");
    std::fs::write(
        &ultratall_svg,
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="3000" viewBox="0 0 100 3000"><rect width="100" height="3000" fill="#9370db"/></svg>"##,
    )
    .unwrap();

    let cases: Vec<(&str, PathBuf, f32, u64)> = vec![
        ("normal_sf1", normal_svg.clone(), 1.0, 0x1001),
        ("normal_sf1.5", normal_svg.clone(), 1.5, 0x1002),
        ("normal_sf2", normal_svg.clone(), 2.0, 0x1003),
        ("normal_sf3", normal_svg.clone(), 3.0, 0x1004),
        ("ultrawide_sf1", ultrawide_svg.clone(), 1.0, 0x2001),
        ("ultrawide_sf2", ultrawide_svg.clone(), 2.0, 0x2002),
        ("ultratall_sf1", ultratall_svg.clone(), 1.0, 0x3001),
        ("ultratall_sf2", ultratall_svg.clone(), 2.0, 0x3002),
        // Hot cache check: re-opening 0x1003 with sf=2.0 hits cached _v2.svg
        ("hot_cache_normal_sf2", normal_svg.clone(), 2.0, 0x1003),
    ];

    for (name, path, sf, hash) in cases {
        let candidate_path = cx.update(|window, app| {
            view.update(app, |ws, cx| {
                ws.open_lightbox_with_scale(path.clone(), hash, sf, window, cx);
                cx.notify();
            });
            let _ = window.draw(app);
            view.read(app)
                .lightbox
                .displayed_path()
                .cloned()
                .expect("displayed_path must be set")
        });

        // Ensure newly generated files use _v2.svg suffix to avoid old unbudgeted cache files
        assert!(
            candidate_path.to_string_lossy().ends_with("_v2.svg")
                || candidate_path == path,
            "cached file {:?} must have _v2 suffix",
            candidate_path
        );

        cx.run_until_parked();

        cx.update(|window, app| {
            let _ = window.draw(app);
            let resource = Resource::Path(Arc::from(candidate_path.as_path()));
            let render_img = window
                .get_asset::<ImgResourceLoader>(&resource, app)
                .expect("asset must be loaded in cache by element draw")
                .expect("RenderImage decode must succeed");

            let size = render_img.size(0);
            let pw = size.width.0 as u64;
            let ph = size.height.0 as u64;
            let area = pw * ph;
            let max_side = pw.max(ph);

            eprintln!(
                "[BUDGET TEST] {}: sf={} => decoded {}x{} (area={} px = {:.2} MP, max_side={})",
                name,
                sf,
                pw,
                ph,
                area,
                (area as f64) / 1_000_000.0,
                max_side
            );

            assert!(
                max_side <= 4000,
                "Case {}: physical max side {} exceeds 4000px budget!",
                name,
                max_side
            );
            assert!(
                area <= 16_000_000,
                "Case {}: physical pixels {} exceed 16MP budget!",
                name,
                area
            );

            view.update(app, |ws, cx| {
                ws.close_lightbox(window, cx);
            });
        });
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[gpui::test]
async fn test_fallback_shared_asset_not_dropped_on_lightbox_close(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let state = AppState::new_isolated(glidemark_core::document::Document::new_scratch());
        WorkspaceView::new(state, cx)
    });

    cx.update(|window, app| {
        view.update(app, |ws, _cx| {
            ws.window_handle = Some(window.window_handle());
        });
    });

    let temp_dir = std::env::temp_dir().join("glidemark_test_ownership_fallback");
    let _ = std::fs::create_dir_all(&temp_dir);
    let shared_svg = temp_dir.join("shared_preview.svg");
    std::fs::write(&shared_svg, test_svg_content()).unwrap();
    let shared_res = Resource::Path(Arc::from(shared_svg.as_path()));

    // 1. Lightbox opens in fallback mode (using shared_svg without exclusive HiDPI)
    let drops_before = TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst);
    cx.update(|window, app| {
        view.update(app, |ws, _cx| {
            let evicted = ws.lightbox.open(shared_svg.clone(), 7777);
            assert_eq!(evicted, None, "open with fallback has no previous exclusive image");
        });
        let _ = window.draw(app);
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        assert!(
            window.get_asset::<ImgResourceLoader>(&shared_res, app).is_some(),
            "shared asset must be loaded in cache by element paint"
        );
    });

    // 2. Close lightbox: must NOT drop or purge preview's shared_svg!
    cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.close_lightbox(window, cx);
        });

        assert!(!view.read(app).lightbox.is_open);
        assert_eq!(
            TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst),
            drops_before,
            "closing fallback lightbox must NOT drop image from atlas"
        );
        assert!(
            window.get_asset::<ImgResourceLoader>(&shared_res, app).is_some(),
            "shared preview asset MUST REMAIN cached in ImgResourceLoader after fallback lightbox close"
        );
    });

    // 3. Contrast: open exclusive HiDPI lightbox, verify closing DOES drop exclusive image but leaves shared intact
    let hidpi_candidate = cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.open_lightbox(shared_svg.clone(), 8888, window, cx);
            cx.notify();
        });
        let _ = window.draw(app);
        view.read(app).lightbox.hidpi_svg_path.clone().expect("must have exclusive HiDPI path")
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let hidpi_res = Resource::Path(Arc::from(hidpi_candidate.as_path()));
        assert!(window.get_asset::<ImgResourceLoader>(&hidpi_res, app).is_some());

        let drops_before_close = TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst);
        view.update(app, |ws, cx| {
            ws.close_lightbox(window, cx);
        });

        assert_eq!(
            TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst),
            drops_before_close + 1,
            "closing exclusive HiDPI lightbox MUST drop exclusive image from atlas"
        );
        assert!(
            window.get_asset::<ImgResourceLoader>(&hidpi_res, app).is_none(),
            "exclusive HiDPI asset must be purged from ImgResourceLoader"
        );
        assert!(
            window.get_asset::<ImgResourceLoader>(&shared_res, app).is_some(),
            "shared preview asset MUST STILL REMAIN cached"
        );
    });

    let _ = std::fs::remove_file(shared_svg);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[gpui::test]
async fn test_lightbox_write_failure_fallback_without_blank_screen(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let state = AppState::new_isolated(glidemark_core::document::Document::new_scratch());
        WorkspaceView::new(state, cx)
    });

    cx.update(|window, app| {
        view.update(app, |ws, _cx| {
            ws.window_handle = Some(window.window_handle());
        });
    });

    let temp_dir = std::env::temp_dir().join("glidemark_test_write_failure");
    let _ = std::fs::create_dir_all(&temp_dir);
    let fallback_svg = temp_dir.join("fallback.svg");
    std::fs::write(&fallback_svg, test_svg_content()).unwrap();

    let code_hash: u64 = 0xeeee_ffff_0000;

    // Create a directory where the HiDPI file would be written, causing std::fs::write to fail
    let cache_dir = std::env::temp_dir().join("glidemark").join("mermaid_hidpi");
    let _ = std::fs::create_dir_all(&cache_dir);

    let (expected_unwritable_path, scale_bucket) = cx.update(|window, app| {
        let is_dark = view.read(app).theme(app).is_dark;
        let variant_str = if is_dark { "dark" } else { "light" };
        let sb = glidemark_core::mermaid::scale_factor_to_bucket(window.scale_factor());
        let path = cache_dir.join(format!("{:016x}_{}_{}_v2.svg", code_hash, variant_str, sb));
        (path, sb)
    });

    let _ = std::fs::create_dir(&expected_unwritable_path);

    cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.open_lightbox(fallback_svg.clone(), code_hash, window, cx);
            cx.notify();
        });
        let _ = window.draw(app);
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let ws = view.read(app);
        assert!(ws.lightbox.is_open, "lightbox must be open in fallback mode");
        assert_eq!(
            ws.lightbox.displayed_path(),
            Some(&fallback_svg),
            "fallback to fallback_svg must occur without blank screen"
        );
        assert_eq!(
            ws.lightbox.hidpi_svg_path,
            None,
            "no hidpi_svg_path created"
        );

        // Verify LIGHTBOX_DIAGRAM_CACHE does NOT have any entry for this failed write
        let mut cache = glidemark_app::views::lightbox::LIGHTBOX_DIAGRAM_CACHE.lock().unwrap();
        assert_eq!(
            cache.get(&(code_hash, ws.lightbox.is_dark.unwrap_or(false), scale_bucket)),
            None,
            "failed generation must NOT be inserted into cache"
        );
    });

    let _ = std::fs::remove_dir(&expected_unwritable_path);
    let _ = std::fs::remove_file(fallback_svg);
    let _ = std::fs::remove_dir_all(&temp_dir);
}
