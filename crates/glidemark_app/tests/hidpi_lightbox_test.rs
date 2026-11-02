use glidemark_app::views::lightbox::{LightboxDiagramCache, LightboxState};
use glidemark_app::views::workspace::resolve_lightbox_svg_path;
use glidemark_core::mermaid::{
    apply_texture_pixel_budget, calculate_hidpi_dimensions, scale_factor_to_bucket,
};
use gpui::{Image, ImageFormat};
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn test_hidpi_dimensions_multi_scale() {
    let scales = [1.0f32, 1.25f32, 1.5f32, 2.0f32, 3.0f32];
    for &scale in &scales {
        let (w, h) = calculate_hidpi_dimensions(1200.0, 800.0, scale);
        let bucket = scale_factor_to_bucket(scale);
        let effective_scale = (bucket as f64) / 2.0;
        let physical_w = w * effective_scale;
        let physical_h = h * effective_scale;
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
    let effective_scale = (scale_factor_to_bucket(2.0) as f64) / 2.0;
    let phys_w = ext_w * effective_scale;
    let phys_h = ext_h * effective_scale;
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
    let dir = std::env::temp_dir().join("glidemark_test_lru1");
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
    let temp_dir = std::env::temp_dir().join("glidemark_test_is_dark_match");
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
    assert_eq!(state.hidpi_image, None);

    // Close when empty returns None
    assert_eq!(state.close(), None);

    // Open HiDPI
    let svg_path = PathBuf::from("/tmp/original.svg");
    let hidpi_path = PathBuf::from("/tmp/hidpi.svg");
    let image = Arc::new(Image::from_bytes(ImageFormat::Svg, b"<svg id=\"1\"/>".to_vec()));

    state.open_hidpi(
        svg_path.clone(),
        hidpi_path.clone(),
        image.clone(),
        9999,
    );
    assert!(state.is_open);
    assert_eq!(state.svg_path, Some(svg_path));
    assert_eq!(state.hidpi_svg_path, Some(hidpi_path));
    assert_eq!(state.hidpi_image, Some(image.clone()));
    assert_eq!(state.code_hash, Some(9999));
    assert_eq!(state.zoom, 1.0);

    // First close returns old image
    let closed_img = state.close();
    assert_eq!(closed_img, Some(image));
    assert!(!state.is_open);
    assert_eq!(state.hidpi_svg_path, None);
    assert_eq!(state.hidpi_image, None);

    // Second close returns None
    let second_closed = state.close();
    assert_eq!(second_closed, None);
}

#[test]
fn test_document_swapped_and_open_paths_release_vram_safely() {
    let mut state = LightboxState::new();

    // 1. Initial state has no image
    assert_eq!(state.hidpi_image, None);
    assert_eq!(state.close(), None);

    // 2. Open HiDPI image 1
    let svg1 = PathBuf::from("/tmp/doc1_diagram.svg");
    let hidpi1 = PathBuf::from("/tmp/doc1_hidpi.svg");
    let img1 = Arc::new(Image::from_bytes(ImageFormat::Svg, b"<svg id=\"doc1\"/>".to_vec()));
    let evicted0 = state.open_hidpi(svg1.clone(), hidpi1.clone(), img1.clone(), 101);
    assert_eq!(evicted0, None, "Initial open_hidpi should have no evicted image");
    assert_eq!(state.hidpi_image, Some(img1.clone()));

    // 3. Open another diagram (or plain open) without explicit close:
    // Must return img1 so the caller can invoke image.remove_asset(cx)
    let svg2 = PathBuf::from("/tmp/doc2_diagram.svg");
    let evicted1 = state.open(svg2.clone(), 102);
    assert_eq!(
        evicted1,
        Some(img1.clone()),
        "Calling open() while holding a HiDPI image must return the previous image for release"
    );
    assert_eq!(state.hidpi_image, None);

    // 4. Open HiDPI image 2
    let hidpi2 = PathBuf::from("/tmp/doc2_hidpi.svg");
    let img2 = Arc::new(Image::from_bytes(ImageFormat::Svg, b"<svg id=\"doc2\"/>".to_vec()));
    let evicted2 = state.open_hidpi(svg2.clone(), hidpi2.clone(), img2.clone(), 102);
    assert_eq!(evicted2, None);
    assert_eq!(state.hidpi_image, Some(img2.clone()));

    // 5. Open HiDPI image 3 while image 2 is active:
    // Must return img2 so the caller can invoke image.remove_asset(cx)
    let svg3 = PathBuf::from("/tmp/doc3_diagram.svg");
    let hidpi3 = PathBuf::from("/tmp/doc3_hidpi.svg");
    let img3 = Arc::new(Image::from_bytes(ImageFormat::Svg, b"<svg id=\"doc3\"/>".to_vec()));
    let evicted3 = state.open_hidpi(svg3.clone(), hidpi3.clone(), img3.clone(), 103);
    assert_eq!(
        evicted3,
        Some(img2),
        "Calling open_hidpi() while holding another HiDPI image must return the old image for release"
    );
    assert_eq!(state.hidpi_image, Some(img3.clone()));

    // 6. Document swapped simulation:
    // When document is swapped, close_lightbox() calls state.close()
    let swapped_evicted = state.close();
    assert_eq!(
        swapped_evicted,
        Some(img3),
        "Document swapped must release the active HiDPI image for release"
    );
    assert!(!state.is_open);
    assert_eq!(state.hidpi_image, None);
    assert_eq!(state.hidpi_svg_path, None);
}
