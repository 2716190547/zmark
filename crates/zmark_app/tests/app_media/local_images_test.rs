use zmark_app::local_images::*;
use zmark_app::state::{AppState, DocKey, ViewMode};
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;
use gpui::{TestAppContext, VisualTestContext};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::path::Path;
use std::sync::Arc;

fn write_image(path: &Path, format: ImageFormat, color: [u8; 4]) {
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 4, Rgba(color)))
        .save_with_format(path, format)
        .unwrap();
}

#[test]
fn local_paths_are_document_relative_and_decoded_once() {
    let doc = Path::new("/notes/中文 空格/doc.md");
    assert_eq!(
        resolve_image_path("../图%20像%23a.png#fragment", Some(doc)).unwrap(),
        Path::new("/notes/中文 空格/../图 像#a.png")
    );
    assert_eq!(
        resolve_image_path("a+b%2520.png", Some(doc)).unwrap(),
        Path::new("/notes/中文 空格/a+b%20.png")
    );
    assert_eq!(
        resolve_image_path("/tmp/a:b.png", None).unwrap(),
        Path::new("/tmp/a:b.png")
    );
    for bad in [
        "",
        "x%00.png",
        "x%FF.png",
        "http://x/a.png",
        "https://x",
        "file:///tmp/x",
        "data:image/png,aaa",
        "x.png?q=1",
    ] {
        assert!(resolve_image_path(bad, Some(doc)).is_err(), "{bad}");
    }
    assert!(resolve_image_path("a.png", None)
        .unwrap_err()
        .contains("未保存"));
}

#[test]
fn raster_snapshots_are_immutable_and_animations_are_static() {
    let dir = tempfile::tempdir().unwrap();
    for (name, format) in [
        ("a.png", ImageFormat::Png),
        ("a.jpg", ImageFormat::Jpeg),
        ("a.webp", ImageFormat::WebP),
    ] {
        let path = dir.path().join(name);
        write_image(&path, format, [219, 112, 147, 255]);
        let image = load_image(path.to_str().unwrap(), None).unwrap();
        std::fs::write(&path, b"replaced original after validation").unwrap();
        let decoded = image::open(image.snapshot.path()).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (8, 4));
        if format != ImageFormat::Jpeg {
            assert_eq!(decoded.to_rgba8().get_pixel(0, 0).0, [219, 112, 147, 255]);
        }
    }
    let webp = dir.path().join("animated.webp");
    let animated = include_bytes!("../fixtures/animated.webp");
    std::fs::write(&webp, animated).unwrap();
    assert!(
        image::codecs::webp::WebPDecoder::new(std::io::Cursor::new(animated))
            .unwrap()
            .has_animation()
    );
    let first = load_image(webp.to_str().unwrap(), None).unwrap();
    assert_eq!(first.snapshot.path().extension().unwrap(), "png");
    assert_eq!(
        image::open(first.snapshot.path())
            .unwrap()
            .to_rgba8()
            .get_pixel(0, 0)
            .0,
        [255, 0, 0, 255]
    );
    let path = dir.path().join("animated.gif");
    let mut encoder = image::codecs::gif::GifEncoder::new(std::fs::File::create(&path).unwrap());
    encoder
        .encode_frames([
            image::Frame::new(RgbaImage::from_pixel(8, 4, Rgba([255, 0, 0, 255]))),
            image::Frame::new(RgbaImage::from_pixel(8, 4, Rgba([0, 0, 255, 255]))),
        ])
        .unwrap();
    drop(encoder);
    let image = load_image(path.to_str().unwrap(), None).unwrap();
    assert_eq!(image.snapshot.path().extension().unwrap(), "png");
    assert_eq!(
        image::open(image.snapshot.path())
            .unwrap()
            .to_rgba8()
            .get_pixel(0, 0)
            .0,
        [255, 0, 0, 255]
    );
}

#[test]
fn raster_snapshots_are_downscaled_to_the_texture_budget() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.png");
    DynamicImage::new_rgba8(3000, 1000).save(&path).unwrap();
    let image = load_image(path.to_str().unwrap(), None).unwrap();
    assert!(image.width <= 1600 && image.height <= 1600);
    assert_eq!(image.height, 533);
    let decoded = image::open(image.snapshot.path()).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (image.width, image.height));
    assert_eq!(
        image.decoded_bytes,
        u64::from(image.width) * u64::from(image.height) * 4
    );

    let small = dir.path().join("small.png");
    DynamicImage::new_rgba8(8, 4).save(&small).unwrap();
    let image = load_image(small.to_str().unwrap(), None).unwrap();
    assert_eq!((image.width, image.height), (8, 4));
}

#[test]
fn input_decode_and_svg_budgets_fail_with_reasons() {
    let dir = tempfile::tempdir().unwrap();
    let large = dir.path().join("large.png");
    std::fs::File::create(&large)
        .unwrap()
        .set_len(MAX_INPUT_BYTES + 1)
        .unwrap();
    assert!(load_image(large.to_str().unwrap(), None)
        .unwrap_err()
        .contains("20MiB"));
    let corrupt = dir.path().join("bad.png");
    std::fs::write(&corrupt, b"broken image").unwrap();
    assert!(load_image(corrupt.to_str().unwrap(), None).is_err());
    assert!(
        load_image(dir.path().join("missing.png").to_str().unwrap(), None)
            .unwrap_err()
            .contains("无法读取")
    );
    assert_eq!(check_dimensions(4000, 4000).unwrap(), 64_000_000);
    assert!(check_dimensions(4001, 1).is_err());
    let oversized = dir.path().join("wide.png");
    write_image(&oversized, ImageFormat::Png, [0, 0, 0, 255]);
    DynamicImage::new_rgba8(4001, 1).save(&oversized).unwrap();
    assert!(load_image(oversized.to_str().unwrap(), None).is_err());
    let svg = dir.path().join("a.svg");
    for (body, valid) in [
        (
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="2000" height="2000"><text x="2" y="20">保留文字</text></svg>"#,
            true,
        ),
        (
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="2001" height="20"/>"#,
            false,
        ),
        (
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><image href="/tmp/nonexistent-external.png" width="20" height="20"/></svg>"#,
            false,
        ),
        (
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><image href="data:image/png;base64,aGVsbG8=" width="20" height="20"/></svg>"#,
            false,
        ),
    ] {
        std::fs::write(&svg, body).unwrap();
        let loaded = load_image(svg.to_str().unwrap(), None);
        assert_eq!(loaded.is_ok(), valid, "{body}: {loaded:?}");
        if let Ok(image) = loaded {
            assert_eq!((image.width, image.height), (4000, 4000));
            assert_eq!(
                std::fs::read(image.snapshot.path()).unwrap(),
                body.as_bytes()
            );
        }
    }
}

#[test]
fn cache_rejects_stale_results_evicts_and_enforces_the_texture_budget() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.svg");
    std::fs::write(
        &path,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1400" height="1400"/>"#,
    )
    .unwrap();
    let image = load_image(path.to_str().unwrap(), None).unwrap();
    let mut cache = LocalImages::default();
    cache.begin_frame(Some(DocKey::Scratch(1)), None);
    cache.request("a");
    cache.request("b");
    assert!(cache.complete(cache.generation, "a".into(), Ok(image.clone())));
    assert!(cache.complete(cache.generation, "b".into(), Ok(image.clone())));
    assert!(matches!(
        cache.entries.get("b"),
        Some(ImageState::Failed(_))
    ));
    assert!(cache.decoded_bytes() <= MAX_CACHE_BYTES);
    let old_generation = cache.generation;
    cache.begin_frame(Some(DocKey::Scratch(2)), None);
    cache.request("a");
    assert!(!cache.complete(old_generation, "a".into(), Ok(image.clone())));
    assert!(matches!(cache.entries.get("a"), Some(ImageState::Waiting)));
    assert!(cache.complete(cache.generation, "a".into(), Ok(image.clone())));
    cache.begin_frame(Some(DocKey::Scratch(2)), None);
    cache.evict_hidden();
    assert!(cache.entries.is_empty());
    assert_eq!(cache.decoded_bytes(), 0);
}

fn draw(cx: &mut VisualTestContext) {
    for _ in 0..5 {
        cx.run_until_parked();
        cx.update(|w, cx| {
            let _ = w.draw(cx);
        });
    }
    cx.run_until_parked();
}

#[gpui::test]
async fn preview_elements_load_private_snapshots_and_remeasure(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("中文 图.png");
    write_image(&path, ImageFormat::Png, [219, 112, 147, 255]);
    let docpath = dir.path().join("note.md");
    std::fs::write(&docpath, "before ![alt](中文%20图.png) after\n\n![svg](vector.svg)\n\n![webp](animated.webp)\n\n![missing](missing.png)").unwrap();
    std::fs::write(dir.path().join("vector.svg"), r#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="8"><rect width="12" height="8" fill="red"/></svg>"#).unwrap();
    std::fs::write(
        dir.path().join("animated.webp"),
        include_bytes!("../fixtures/animated.webp"),
    )
    .unwrap();
    let window = cx.add_window(|window, cx| {
        let mut state = AppState::new(Document::from_file(docpath.clone()).unwrap());
        state.set_view_mode(ViewMode::Preview);
        let ws = WorkspaceView::new(state, cx);
        window.focus(&ws.focus_handle);
        ws
    });
    let view = window.root(cx).unwrap();
    let cx = &mut VisualTestContext::from_window(*window, cx);
    cx.update(|w, cx| {
        let _ = w.draw(cx);
    });
    let placeholder_y = cx
        .debug_bounds("local_image_status:missing.png")
        .unwrap()
        .origin
        .y;
    draw(cx);
    assert_ne!(
        cx.debug_bounds("local_image_status:missing.png")
            .unwrap()
            .origin
            .y,
        placeholder_y,
        "Virtual preview remeasures after image dimensions arrive"
    );
    assert!(
        cx.debug_bounds("local_image:中文%20图.png").is_some(),
        "Real image element laid out"
    );
    assert!(cx.debug_bounds("local_image:vector.svg").is_some());
    assert!(cx.debug_bounds("local_image_status:missing.png").is_some());
    cx.update(|window, cx| {
        let ws = view.read(cx);
        assert_eq!(ws.state.doc().parsed.blocks[0].to_plain_text(), "before alt after");
        let images = ws.local_images.lock().unwrap();
        assert!(matches!(images.entries.get("missing.png"), Some(ImageState::Failed(reason)) if reason.contains("无法读取")));
        let sources = [("中文%20图.png", 8, 4), ("vector.svg", 24, 16), ("animated.webp", 8, 4)].map(|(url, w, h)| {
            let Some(ImageState::Ready(image)) = images.entries.get(url) else { panic!("{url} was not loaded"); };
            (url, w, h, gpui::Resource::Path(Arc::from(image.snapshot.path())))
        });
        drop(images);
        for (url, width, height, resource) in sources {
            // Query the same private resource the actual image element displays.
            let render = window.get_asset::<gpui::ImgResourceLoader>(&resource, cx).expect("Production loader completed").unwrap();
            assert_eq!(render.frame_count(), 1);
            assert_eq!((render.size(0).width.0, render.size(0).height.0), (width, height));
            assert!(render.size(0).width.0 <= 4000 && render.size(0).height.0 <= 4000);
            if url.ends_with("png") { assert_eq!(&render.as_bytes(0).unwrap()[..4], &[147, 112, 219, 255]); }
        }
    });
    let before = cx.debug_bounds("local_image:中文%20图.png").unwrap();
    assert!((f32::from(before.size.width) / f32::from(before.size.height) - 2.).abs() < 0.01);
    let same_snapshot = cx.update(|_, cx| {
        let ws = view.read(cx);
        match ws
            .local_images
            .lock()
            .unwrap()
            .entries
            .get("中文%20图.png")
            .unwrap()
        {
            ImageState::Ready(i) => i.snapshot.path().to_path_buf(),
            _ => unreachable!(),
        }
    });
    cx.update(|_, cx| {
        view.update(cx, |ws, cx| {
            ws.state.editor.insert_text("before ");
            ws.parse_now_and_refresh();
            cx.notify();
        })
    });
    draw(cx);
    cx.update(|_, cx| {
        let ws = view.read(cx);
        let images = ws.local_images.lock().unwrap();
        let Some(ImageState::Ready(image)) = images.entries.get("中文%20图.png") else {
            panic!("Image lost during ordinary edit");
        };
        assert_eq!(
            image.snapshot.path(),
            same_snapshot,
            "Typing must reuse decoded images"
        );
    });
    write_image(
        &dir.path().join("missing.png"),
        ImageFormat::Png,
        [0, 0, 255, 255],
    );
    cx.update(|_, cx| view.update(cx, |ws, cx| ws.reset_syntax_attempts_and_refresh(cx)));
    draw(cx);
    assert!(
        cx.debug_bounds("local_image:missing.png").is_some(),
        "Explicit refresh reloads a formerly missing image"
    );
    let old_path = cx.update(|_, cx| {
        let ws = view.read(cx);
        let images = ws.local_images.lock().unwrap();
        match images.entries.get("中文%20图.png").unwrap() {
            ImageState::Ready(i) => i.snapshot.path().to_path_buf(),
            _ => unreachable!(),
        }
    });
    let other = dir.path().join("other");
    std::fs::create_dir(&other).unwrap();
    write_image(
        &other.join("中文 图.png"),
        ImageFormat::Png,
        [0, 255, 0, 255],
    );
    std::fs::write(other.join("note.md"), "![other](中文%20图.png)").unwrap();
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.state.load_file(other.join("note.md")).unwrap();
            ws.on_document_swapped(Some(window), cx);
            cx.notify();
        })
    });
    draw(cx);
    assert!(!old_path.exists(), "Old document snapshot must be released");
    cx.update(|window, cx| {
        let ws = view.read(cx);
        let images = ws.local_images.lock().unwrap();
        let Some(ImageState::Ready(i)) = images.entries.get("中文%20图.png") else {
            panic!("new doc not loaded");
        };
        let resource = gpui::Resource::Path(Arc::from(i.snapshot.path()));
        drop(images);
        let render = window
            .get_asset::<gpui::ImgResourceLoader>(&resource, cx)
            .unwrap()
            .unwrap();
        assert_eq!(&render.as_bytes(0).unwrap()[..4], &[0, 255, 0, 255]);
    });
}

#[gpui::test]
async fn switching_documents_with_an_image_in_flight_uses_current_base(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    for (folder, color) in [("a", [255, 0, 0, 255]), ("b", [0, 255, 0, 255])] {
        let base = dir.path().join(folder);
        std::fs::create_dir(&base).unwrap();
        write_image(&base.join("same.png"), ImageFormat::Png, color);
        std::fs::write(base.join("note.md"), "![same](same.png)").unwrap();
    }
    let window = cx.add_window(|_, cx| {
        let mut state = AppState::new(Document::from_file(dir.path().join("a/note.md")).unwrap());
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    let view = window.root(cx).unwrap();
    let cx = &mut VisualTestContext::from_window(*window, cx);
    // Begin A's deferred load, then change the document before scheduling its completion.
    cx.update(|w, cx| {
        let _ = w.draw(cx);
    });
    let generation = cx.update(|_, cx| {
        let ws = view.read(cx);
        assert!(
            ws.local_image_task.is_some(),
            "A load must actually be in flight"
        );
        let generation = ws.local_images.lock().unwrap().generation;
        generation
    });
    cx.update(|w, cx| {
        view.update(cx, |ws, cx| {
            ws.state.load_file(dir.path().join("b/note.md")).unwrap();
            ws.on_document_swapped(Some(w), cx);
            cx.notify();
        });
        let _ = w.draw(cx);
    });
    draw(cx);
    let source = cx.update(|_, cx| {
        let ws = view.read(cx);
        let images = ws.local_images.lock().unwrap();
        assert!(images.generation > generation);
        let Some(ImageState::Ready(image)) = images.entries.get("same.png") else {
            panic!("current image not ready");
        };
        assert_eq!(
            image::open(image.snapshot.path())
                .unwrap()
                .to_rgba8()
                .get_pixel(0, 0)
                .0,
            [0, 255, 0, 255]
        );
        gpui::Resource::Path(Arc::from(image.snapshot.path()))
    });
    cx.update(|w, cx| {
        let rendered = w
            .get_asset::<gpui::ImgResourceLoader>(&source, cx)
            .unwrap()
            .unwrap();
        assert_eq!(&rendered.as_bytes(0).unwrap()[..4], &[0, 255, 0, 255]);
    });
}

#[gpui::test]
async fn clicking_a_preview_image_opens_the_lightbox_and_escape_closes_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    write_image(&dir.path().join("pic.png"), ImageFormat::Png, [10, 120, 200, 255]);
    std::fs::write(dir.path().join("vector.svg"), r#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="8"><rect width="12" height="8" fill="red"/></svg>"#).unwrap();
    let docpath = dir.path().join("note.md");
    std::fs::write(&docpath, "![pic](pic.png)\n\n![vec](vector.svg)").unwrap();
    let window = cx.add_window(|window, cx| {
        let mut state = AppState::new_isolated(Document::from_file(docpath.clone()).unwrap());
        state.set_view_mode(ViewMode::Preview);
        let ws = WorkspaceView::new(state, cx);
        window.focus(&ws.focus_handle);
        ws
    });
    let view = window.root(cx).unwrap();
    let cx = &mut VisualTestContext::from_window(*window, cx);
    draw(cx);

    for (name, selector, wanted_ext) in [
        ("pic.png", "local_image:pic.png", "png"),
        ("vector.svg", "local_image:vector.svg", "svg"),
    ] {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{name} laid out"));
        cx.simulate_click(bounds.center(), gpui::Modifiers::none());
        cx.update(|_, cx| {
            let ws = view.read(cx);
            assert!(ws.lightbox.is_open, "clicking {name} opens the lightbox");
            let shown = ws.lightbox.displayed_path().expect("lightbox shows an image");
            assert!(
                shown.extension().is_some_and(|e| e.eq_ignore_ascii_case(wanted_ext)),
                "{name} is shown as its own snapshot, got {shown:?}"
            );
        });
        cx.simulate_keystrokes("escape");
        cx.update(|_, cx| assert!(!view.read(cx).lightbox.is_open, "escape closes the lightbox"));
    }
}

#[test]
fn lightbox_fit_size_keeps_aspect_and_fits_800_by_640() {
    use zmark_app::views::lightbox::lightbox_fit_size;
    let dir = tempfile::tempdir().unwrap();
    let wide = dir.path().join("wide.png");
    let tall = dir.path().join("tall.png");
    DynamicImage::new_rgba8(1600, 400).save(&wide).unwrap();
    DynamicImage::new_rgba8(300, 1200).save(&tall).unwrap();
    let (w, h) = lightbox_fit_size(&wide);
    assert_eq!((w, h), (800.0, 200.0));
    let (w, h) = lightbox_fit_size(&tall);
    assert!((h - 640.0).abs() < 0.01, "a tall image is limited by height, got {h}");
    assert!((w - 160.0).abs() < 0.01);
    // Unreadable files fall back to the full box instead of failing to draw.
    assert_eq!(lightbox_fit_size(&dir.path().join("missing.png")), (800.0, 640.0));
}
