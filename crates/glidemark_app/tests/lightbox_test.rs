use glidemark_app::views::lightbox::LightboxState;
use std::path::PathBuf;

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

    state.end_drag();
    assert!(!state.is_dragging);
}
