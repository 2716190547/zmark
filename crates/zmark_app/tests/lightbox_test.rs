use gpui::{px, size, Modifiers, TestAppContext};
use std::path::PathBuf;
use zmark_app::state::AppState;
use zmark_app::views::lightbox::LightboxState;
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;

#[gpui::test]
async fn lightbox_fit_hud_click_keeps_overlay_open(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let image_path = dir.path().join("image.png");
    image::RgbaImage::new(80, 40).save(&image_path).unwrap();
    let (view, cx) = cx.add_window_view(|window, cx| {
        let view = WorkspaceView::new(AppState::new_isolated(Document::new_scratch()), cx);
        window.focus(&view.focus_handle, cx);
        view
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.open_image_lightbox(image_path.clone(), window, cx);
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    for selector in [
        "hud_btn_fit",
        "hud_btn_zoom_in",
        "hud_btn_zoom_out",
        "hud_zoom_display",
    ] {
        let button = cx.debug_bounds(selector).expect("HUD button is visible");
        cx.simulate_click(button.center(), Modifiers::none());
        cx.update(|window, cx| {
            assert!(
                view.read(cx).lightbox.is_open,
                "{selector} must not close the lightbox"
            );
            let _ = window.draw(cx);
        });
        assert!(cx.debug_bounds("diagram_lightbox_backdrop").is_some());
    }
    let card = cx.debug_bounds("lightbox_diagram_card").unwrap();
    cx.simulate_mouse_down(card.center(), gpui::MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        card.center() + gpui::point(px(12.), px(20.)),
        gpui::MouseButton::Left,
        Modifiers::none(),
    );
    let pan = cx.update(|_, cx| (view.read(cx).lightbox.pan_x, view.read(cx).lightbox.pan_y));
    cx.simulate_mouse_move(
        card.center() + gpui::point(px(50.), px(60.)),
        None,
        Modifiers::none(),
    );
    cx.update(|_, cx| {
        let state = &view.read(cx).lightbox;
        assert!(
            !state.is_dragging,
            "reentering after release must stop dragging"
        );
        assert_eq!(
            (state.pan_x, state.pan_y),
            pan,
            "released pointer must not move the image"
        );
    });
    let close = cx.debug_bounds("hud_btn_close").unwrap();
    cx.simulate_click(close.center(), Modifiers::none());
    cx.update(|_, cx| assert!(!view.read(cx).lightbox.is_open));
    for escape in [false, true] {
        cx.update(|window, cx| {
            view.update(cx, |ws, cx| {
                ws.open_image_lightbox(image_path.clone(), window, cx);
                cx.notify();
            });
            let _ = window.draw(cx);
        });
        if escape {
            cx.simulate_keystrokes("escape");
        } else {
            cx.simulate_click(gpui::point(px(4.), px(4.)), Modifiers::none());
        }
        cx.update(|_, cx| {
            assert!(
                !view.read(cx).lightbox.is_open,
                "background click and Escape must still close"
            )
        });
    }
}

#[gpui::test]
async fn lightbox_fits_live_viewport_and_100_percent_is_natural(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let image_path = dir.path().join("small.png");
    image::RgbaImage::new(80, 40).save(&image_path).unwrap();
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut state = AppState::new_isolated(Document::new_scratch());
        state.drawer_open = true;
        state.set_view_mode(zmark_app::state::ViewMode::Split);
        WorkspaceView::new(state, cx)
    });
    cx.simulate_resize(size(px(1200.), px(800.)));
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.open_image_lightbox(image_path, window, cx);
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    // Wait for the existing 180ms drawer animation to expose the final layout.
    cx.executor().advance_clock(std::time::Duration::from_millis(210));
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let backdrop = cx.debug_bounds("diagram_lightbox_backdrop").unwrap();
    assert_eq!(
        backdrop.size,
        size(px(1200.), px(800.)),
        "lightbox must use the whole client viewport with the drawer open"
    );
    let large = cx.debug_bounds("lightbox_diagram_card").unwrap();
    assert!(
        (f32::from(large.left()) - 24.).abs() < 0.01,
        "fit must use the largest image that leaves the 24px safe margin"
    );
    cx.simulate_resize(size(px(640.), px(480.)));
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let small = cx.debug_bounds("lightbox_diagram_card").unwrap();
    let hud = cx.debug_bounds("lightbox_hud_pill").unwrap();
    assert!(
        small.left() >= px(0.) && small.right() <= px(640.),
        "fit must stay inside the resized viewport: {small:?}"
    );
    assert!(
        small.bottom() < hud.top(),
        "fit must leave room for the HUD"
    );
    assert!(
        small.size.width < large.size.width,
        "fit must track the window, not a fixed 800px box"
    );

    let original = cx.debug_bounds("hud_zoom_display").unwrap();
    cx.simulate_click(original.center(), Modifiers::none());
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let natural = cx.debug_bounds("lightbox_diagram_card").unwrap();
    assert!(
        (f32::from(natural.size.width) - 112.).abs() < 0.01,
        "100% must display the 80px image plus 32px padding: {natural:?}"
    );
    assert!((f32::from(natural.size.height) - 72.).abs() < 0.01);

    let zoom_in = cx.debug_bounds("hud_btn_zoom_in").unwrap();
    cx.simulate_click(zoom_in.center(), Modifiers::none());
    let manual_zoom = cx.update(|_, cx| view.read(cx).lightbox.zoom);
    cx.simulate_resize(size(px(900.), px(600.)));
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    assert_eq!(
        cx.update(|_, cx| view.read(cx).lightbox.zoom),
        manual_zoom,
        "resizing must preserve manual zoom"
    );
    let fit = cx.debug_bounds("hud_btn_fit").unwrap();
    cx.simulate_click(fit.center(), Modifiers::none());
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let fitted = cx.debug_bounds("lightbox_diagram_card").unwrap();
    assert!(
        fitted.size.width > natural.size.width,
        "fit may enlarge small images"
    );
    assert!(fitted.bottom() < cx.debug_bounds("lightbox_hud_pill").unwrap().top());
}

#[gpui::test]
async fn lightbox_100_percent_uses_original_svg_not_hidpi_texture(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("diagram.svg");
    std::fs::write(&path, r##"<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900"><rect width="1600" height="900" fill="#123456"/></svg>"##).unwrap();
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut state = AppState::new_isolated(Document::new_scratch());
        state.preferences.reduce_motion = true;
        WorkspaceView::new(state, cx)
    });
    cx.simulate_resize(size(px(640.), px(480.)));
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.open_lightbox(path.clone(), 0xface_bead_1234_5678, window, cx);
            assert!(
                ws.lightbox.hidpi_svg_path.is_some(),
                "test must use the real HiDPI path"
            );
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    let original = cx.debug_bounds("hud_zoom_display").unwrap();
    cx.simulate_click(original.center(), Modifiers::none());
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let card = cx.debug_bounds("lightbox_diagram_card").unwrap();
    assert!(
        (f32::from(card.size.width) - 1632.).abs() < 0.01,
        "100% must preserve the original SVG width even beyond the viewport: {card:?}"
    );
    assert!((f32::from(card.size.height) - 932.).abs() < 0.01);
    cx.update(|window, cx| view.update(cx, |ws, cx| ws.close_lightbox(window, cx)));
    assert!(
        path.exists(),
        "closing must not delete the shared original SVG"
    );
}

#[gpui::test]
async fn lightbox_does_not_edit_the_document_behind_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("image.png");
    image::RgbaImage::new(80, 40).save(&path).unwrap();
    let (view, cx) = cx.add_window_view(|window, cx| {
        let view = WorkspaceView::new(AppState::new_isolated(Document::new_scratch()), cx);
        window.focus(&view.focus_handle, cx);
        view
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.open_image_lightbox(path, window, cx);
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    let before = cx.update(|_, cx| view.read(cx).state.editor.to_text());
    cx.simulate_keystrokes("q");
    cx.update(|_, cx| {
        assert_eq!(
            view.read(cx).state.editor.to_text(),
            before,
            "lightbox keys must not modify hidden source"
        );
        assert!(view.read(cx).lightbox.is_open);
    });
    cx.simulate_keystrokes("escape");
    cx.update(|_, cx| assert!(!view.read(cx).lightbox.is_open));
}

#[test]
fn lightbox_fit_extremes_stay_reachable_and_ignore_invalid_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("diagram.svg");
    for (width, height) in [(100_000, 40_000), (1, 1)] {
        std::fs::write(
            &path,
            format!(
                r#"<svg width="{width}" height="{height}" viewBox="0 0 {width} {height}"></svg>"#
            ),
        )
        .unwrap();
        let mut state = LightboxState::new();
        state.open(path.clone(), 1);
        state.update_viewport(size(px(640.), px(480.)));
        let fitted = state.zoom;
        assert!(fitted > 0.0 && fitted.is_finite());
        assert_eq!(state.natural_size, (width as f32, height as f32));
        state.apply_scroll_zoom(0.);
        assert!(state.is_fitted, "zero wheel delta must not leave fit mode");
        state.start_drag(10., 20.);
        state.update_drag(10., 20.);
        assert!(
            state.is_fitted,
            "a click without movement must not leave fit mode"
        );
        state.end_drag();
        state.apply_scroll_zoom(50.);
        assert!(
            (state.zoom / fitted - 1.1).abs() < 0.0001,
            "first zoom must start at fit, not jump to a legacy bound"
        );
        let manual = state.zoom;
        state.update_viewport(size(px(1200.), px(800.)));
        assert_eq!(state.zoom, manual);
        state.reset();
        state.update_viewport(size(px(1200.), px(800.)));
        assert_eq!(state.zoom, 1.0);
        state.set_zoom(f32::NAN);
        assert_eq!(state.zoom, 1.0);
        state.fit();
        state.update_viewport(size(px(0.), px(0.)));
        assert!(state.zoom.is_finite() && state.zoom > 0.0);
        let valid = state.zoom;
        state.update_viewport(size(px(f32::INFINITY), px(f32::NAN)));
        assert_eq!(state.zoom, valid);
    }
}

#[test]
fn test_lightbox_open_and_close() {
    let mut state = LightboxState::new();
    assert!(!state.is_open);
    assert_eq!(state.zoom, 1.0);

    let path = PathBuf::from("/tmp/diagram.svg");
    state.open(path.clone(), 12345);
    assert!(state.is_open);
    assert_eq!(state.svg_path, Some(path));
    assert_eq!(state.code_hash, Some(12345));
    assert_eq!(state.zoom, 1.0);
    assert_eq!(state.pan_x, 0.0);
    assert_eq!(state.pan_y, 0.0);

    state.close();
    assert!(!state.is_open);
    assert_eq!(state.svg_path, None);
    assert_eq!(state.code_hash, None);
}

#[test]
fn test_lightbox_zoom_bounds() {
    let mut state = LightboxState::new();
    state.open(PathBuf::from("/tmp/test.svg"), 1);

    // Zoom in increments
    state.zoom_in();
    assert_eq!(state.zoom, 1.25);
    state.zoom_in();
    assert_eq!(state.zoom, 1.50);

    // Zoom out increments
    state.zoom_out();
    assert_eq!(state.zoom, 1.25);

    // Max zoom clamp 5.0
    state.set_zoom(10.0);
    assert_eq!(state.zoom, 5.0);

    // Min zoom clamp 0.2
    state.set_zoom(0.05);
    assert_eq!(state.zoom, 0.2);

    // Reset zoom
    state.reset();
    assert_eq!(state.zoom, 1.0);
}

#[test]
fn test_lightbox_scroll_zoom() {
    let mut state = LightboxState::new();
    state.open(PathBuf::from("/tmp/test.svg"), 1);

    state.apply_scroll_zoom(100.0);
    assert!(state.zoom > 1.0);

    state.apply_scroll_zoom(-200.0);
    assert!(state.zoom < 1.2);
}

#[test]
fn test_lightbox_drag_pan() {
    let mut state = LightboxState::new();
    state.open(PathBuf::from("/tmp/test.svg"), 1);

    state.start_drag(100.0, 100.0);
    assert!(state.is_dragging);

    state.update_drag(150.0, 120.0);
    assert_eq!(state.pan_x, 50.0);
    assert_eq!(state.pan_y, 20.0);
    assert!(!state.is_fitted);
    state.update_drag(100.0, 100.0);
    assert_eq!((state.pan_x, state.pan_y), (0.0, 0.0));
    assert!(!state.is_fitted, "dragging back to the start stays manual");

    state.end_drag();
    assert!(!state.is_dragging);
}

#[test]
fn sharp_target_px_follows_zoom_caps_budget_and_skips_raster() {
    use zmark_app::views::lightbox::sharp_target_px;
    let mut state = LightboxState::new();
    assert_eq!(sharp_target_px(&state, 2.0), (0, 0), "closed lightbox has nothing to render");

    let dir = tempfile::tempdir().unwrap();
    let svg = dir.path().join("d.svg");
    std::fs::write(&svg, r#"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="200"/>"#).unwrap();
    state.open(svg, 1);
    state.set_zoom(1.0);
    assert_eq!(sharp_target_px(&state, 2.0), (832, 448), "400x200 at 1x on a 2x screen, rounded up to 64px steps");
    state.set_zoom(5.0);
    let (w, h) = sharp_target_px(&state, 2.0);
    assert!(w.max(h) <= 4096 + 64 && (w as u64) * (h as u64) <= 16_500_000, "capped: {w}x{h}");

    let png = dir.path().join("p.png");
    image::DynamicImage::new_rgba8(10, 10).save(&png).unwrap();
    state.open(png, 2);
    assert_eq!(sharp_target_px(&state, 2.0), (0, 0), "raster images are never re-rendered");
}

#[gpui::test]
async fn zooming_an_svg_lightbox_rerenders_it_sharp_and_closing_releases_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("diagram.svg");
    std::fs::write(&path, r##"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="200" viewBox="0 0 400 200"><rect width="400" height="200" fill="#123456"/></svg>"##).unwrap();
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut state = AppState::new_isolated(Document::new_scratch());
        state.preferences.reduce_motion = true;
        WorkspaceView::new(state, cx)
    });
    cx.simulate_resize(size(px(1200.), px(900.)));
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.open_image_lightbox(path.clone(), window, cx);
            ws.lightbox.set_zoom(2.0);
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    cx.executor().advance_clock(std::time::Duration::from_millis(300));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
        let ws = view.read(cx);
        let (image, px_size) = ws.lightbox_sharp.as_ref().expect("a sharp image is ready after zoom settles");
        assert_eq!(image.size(0).width.0 as u32, px_size.0, "bitmap matches the requested width");
        assert!(px_size.0 >= 800, "2x zoom of a 400px SVG needs at least 800 physical px, got {px_size:?}");
    });
    cx.update(|window, cx| view.update(cx, |ws, cx| ws.close_lightbox(window, cx)));
    cx.update(|_, cx| {
        let ws = view.read(cx);
        assert!(ws.lightbox_sharp.is_none() && ws.lightbox_sharp_task.is_none(), "close releases the sharp image");
    });
}

#[test]
fn wheel_zoom_is_reversible_and_ignores_invalid_deltas() {
    let mut state = LightboxState::new();
    state.open(PathBuf::from("/tmp/test.svg"), 1);
    let original = state.zoom;
    state.apply_scroll_zoom(60.0);
    state.apply_scroll_zoom(-60.0);
    assert!((state.zoom - original).abs() < 0.00001);
    state.apply_scroll_zoom(f32::NAN);
    state.apply_scroll_zoom(f32::INFINITY);
    assert!((state.zoom - original).abs() < 0.00001);
}
