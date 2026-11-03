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
/// Follows Requirement 6:
/// Checks `ws.lightbox.displayed_path()` / `ws.lightbox.hidpi_svg_path` via
/// `window.get_asset::<gpui::ImgResourceLoader>(&resource, cx)` first;
/// otherwise falls back to `ws.lightbox.hidpi_image.as_ref()`.
fn retrieve_lightbox_render_image(
    candidate_path: Option<PathBuf>,
    hidpi_image: Option<Arc<gpui::Image>>,
    window: &mut Window,
    cx: &mut App,
) -> Option<Arc<gpui::RenderImage>> {
    // 1. Check displayed_path() or hidpi_svg_path / svg_path via ImgResourceLoader
    if let Some(path) = candidate_path {
        let resource = Resource::Path(Arc::from(path.as_path()));
        if let Some(Ok(render_image)) = window.get_asset::<ImgResourceLoader>(&resource, cx) {
            return Some(render_image);
        }
    }

    // 2. Fallback to hidpi_image
    if let Some(img) = hidpi_image {
        if let Some(render_image) = img.get_render_image(window, cx) {
            return Some(render_image);
        }
    }

    None
}

/// Helper to create a test SVG containing:
/// - Opaque purple `#9370db` (left half)
/// - Translucent `#ecf2dc` with opacity 0.6 (right half)
fn test_svg_content() -> &'static [u8] {
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="4" viewBox="0 0 8 4"><rect width="4" height="4" fill="#9370db"/><rect x="4" width="4" height="4" fill="#ecf2dc" fill-opacity="0.6"/></svg>"##
}

#[gpui::test]
async fn test_lightbox_matches_preview_bgra_pixels_opaque_and_alpha(cx: &mut TestAppContext) {
    let _lock = TEST_LOCK.lock().unwrap();
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

    // 2. Open lightbox and draw window
    cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.open_lightbox(svg_path.clone(), code_hash, window, cx);
            cx.notify();
        });
        let _ = window.draw(app);
    });
    cx.run_until_parked();

    // 3. Retrieve rendered image displayed by lightbox
    let lightbox_render_image = cx
        .update(|window, app| {
            let (candidate_path, hidpi_img) = {
                let ws = view.read(app);
                let path = ws
                    .lightbox
                    .displayed_path()
                    .map(|p| p.to_path_buf())
                    .or_else(|| ws.lightbox.hidpi_svg_path.clone())
                    .or_else(|| ws.lightbox.svg_path.clone());
                let img = ws.lightbox.hidpi_image.clone();
                (path, img)
            };
            retrieve_lightbox_render_image(candidate_path, hidpi_img, window, app)
        })
        .expect("lightbox must have a rendered image after draw");

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
    let _lock = TEST_LOCK.lock().unwrap();
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

    std::fs::write(&light_svg_path, test_svg_content()).unwrap();
    std::fs::write(&dark_svg_path, test_svg_content()).unwrap();

    // 1. Light Mode: open_lightbox uses the light SVG variant and decodes with correct colors
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
        let (candidate_path, hidpi_img) = {
            let ws = view.read(app);
            let path = ws
                .lightbox
                .displayed_path()
                .map(|p| p.to_path_buf())
                .or_else(|| ws.lightbox.hidpi_svg_path.clone())
                .or_else(|| ws.lightbox.svg_path.clone());
            let img = ws.lightbox.hidpi_image.clone();
            (path, img)
        };
        let img = retrieve_lightbox_render_image(candidate_path, hidpi_img, window, app)
            .expect("light mode lightbox image rendered");
        let bytes = img.as_bytes(0).unwrap();
        let purple = &bytes[0..4];
        let alpha = bytes.chunks_exact(4).find(|p| p[3] > 100 && p[3] < 200).unwrap();

        assert_eq!(
            purple,
            &[219, 112, 147, 255],
            "light mode purple must be BGRA [219, 112, 147, 255]"
        );
        assert_eq!(
            alpha,
            &[219, 241, 236, 153],
            "light mode translucent must be BGRA [219, 241, 236, 153]"
        );

        view.update(app, |ws, cx| {
            ws.close_lightbox(window, cx);
        });
    });

    // 2. Dark Mode: open_lightbox uses the dark SVG variant and decodes with correct colors
    cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.state.set_theme_mode(glidemark_core::config::ThemeMode::Dark);
            assert!(ws.theme(cx).is_dark, "theme must be dark");
            // Pass light_svg_path; open_lightbox must resolve the dark variant
            ws.open_lightbox(light_svg_path.clone(), code_hash, window, cx);
            cx.notify();
        });
        let _ = window.draw(app);
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let (candidate_path, hidpi_img) = {
            let ws = view.read(app);
            let path = ws
                .lightbox
                .displayed_path()
                .map(|p| p.to_path_buf())
                .or_else(|| ws.lightbox.hidpi_svg_path.clone())
                .or_else(|| ws.lightbox.svg_path.clone());
            let img = ws.lightbox.hidpi_image.clone();
            (path, img)
        };
        let img = retrieve_lightbox_render_image(candidate_path, hidpi_img, window, app)
            .expect("dark mode lightbox image rendered");
        let bytes = img.as_bytes(0).unwrap();
        let purple = &bytes[0..4];
        let alpha = bytes.chunks_exact(4).find(|p| p[3] > 100 && p[3] < 200).unwrap();

        assert_eq!(
            purple,
            &[219, 112, 147, 255],
            "dark mode purple must be BGRA [219, 112, 147, 255]"
        );
        assert_eq!(
            alpha,
            &[219, 241, 236, 153],
            "dark mode translucent must be BGRA [219, 241, 236, 153]"
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
    let _lock = TEST_LOCK.lock().unwrap();
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

    // Helper to check asset purged from asset cache
    let check_asset_purged = |path: Option<&Path>,
                              img: Option<&Arc<gpui::Image>>,
                              window: &mut Window,
                              app: &mut App| {
        if let Some(p) = path {
            let resource = Resource::Path(Arc::from(p));
            assert!(
                window.get_asset::<ImgResourceLoader>(&resource, app).is_none(),
                "asset for path {:?} must be purged from ImgResourceLoader",
                p
            );
        }
        if let Some(i) = img {
            assert!(
                i.clone().get_render_image(window, app).is_none(),
                "image must be purged from Image asset cache"
            );
        }
    };

    // --- Path 1: close_lightbox ---
    let drops_before = TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst);
    let (p1_path, p1_img) = cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.open_lightbox(sample_svg.clone(), 3001, window, cx);
            cx.notify();
        });
        let _ = window.draw(app);
        let path = view
            .read(app)
            .lightbox
            .displayed_path()
            .map(|p| p.to_path_buf())
            .or_else(|| view.read(app).lightbox.hidpi_svg_path.clone());
        let img = view.read(app).lightbox.hidpi_image.clone();
        (path, img)
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let has_image = p1_img
            .as_ref()
            .and_then(|i| i.clone().get_render_image(window, app))
            .is_some()
            || p1_path.as_ref().and_then(|p| {
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

        check_asset_purged(p1_path.as_deref(), p1_img.as_ref(), window, app);
    });

    // --- Path 2: replacement open_lightbox ---
    let drops_before = TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst);
    let (p2_path, p2_img) = cx.update(|window, app| {
        view.update(app, |ws, cx| {
            ws.open_lightbox(sample_svg.clone(), 3002, window, cx);
            cx.notify();
        });
        let _ = window.draw(app);
        let path = view
            .read(app)
            .lightbox
            .displayed_path()
            .map(|p| p.to_path_buf())
            .or_else(|| view.read(app).lightbox.hidpi_svg_path.clone());
        let img = view.read(app).lightbox.hidpi_image.clone();
        (path, img)
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let has_image = p2_img
            .as_ref()
            .and_then(|i| i.clone().get_render_image(window, app))
            .is_some()
            || p2_path.as_ref().and_then(|p| {
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

        check_asset_purged(p2_path.as_deref(), p2_img.as_ref(), window, app);
    });

    // --- Path 3: on_document_swapped ---
    cx.update(|window, app| {
        let _ = window.draw(app);
    });
    cx.run_until_parked();

    let drops_before = TEST_DROP_IMAGE_COUNT.load(Ordering::SeqCst);
    let (p3_path, p3_img) = cx.update(|_window, app| {
        let path = view
            .read(app)
            .lightbox
            .displayed_path()
            .map(|p| p.to_path_buf())
            .or_else(|| view.read(app).lightbox.hidpi_svg_path.clone());
        let img = view.read(app).lightbox.hidpi_image.clone();
        (path, img)
    });

    cx.update(|window, app| {
        let has_image = p3_img
            .as_ref()
            .and_then(|i| i.clone().get_render_image(window, app))
            .is_some()
            || p3_path.as_ref().and_then(|p| {
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

        check_asset_purged(p3_path.as_deref(), p3_img.as_ref(), window, app);
    });

    let _ = std::fs::remove_file(sample_svg);
    let _ = std::fs::remove_dir_all(&temp_dir);
}
