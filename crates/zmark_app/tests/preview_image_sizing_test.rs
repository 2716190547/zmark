//! Tall images must not own a whole viewport of preview height.

use zmark_app::state::{AppState, ViewMode};
use zmark_app::views::preview::MAX_IMAGE_DISPLAY_HEIGHT;
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;
use gpui::TestAppContext;
use image::{Rgb, RgbImage};

fn write_png(path: &std::path::Path, width: u32, height: u32) {
    let mut img = RgbImage::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = Rgb([(x % 255) as u8, (y % 255) as u8, 160]);
    }
    img.save(path).expect("write png");
}

#[gpui::test]
async fn tall_image_block_is_capped(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let tall = dir.path().join("tall.png");
    let wide = dir.path().join("wide.png");
    write_png(&tall, 400, 1600);
    write_png(&wide, 1200, 400);

    let text = format!(
        "# 图片标题\n\n![tall]({})\n\n段落。\n\n![wide]({})\n\n段落二。\n",
        tall.display(),
        wide.display()
    );

    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(text);
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.parse_now_and_refresh();
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    for _ in 0..80 {
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.run_until_parked();
    }

    let count = cx.update(|_, cx| view.read(cx).state.doc().parsed.blocks.len());
    assert!(count >= 4, "expected heading + 2 images + paragraphs, got {count}");

    let mut image_heights = Vec::new();
    for ix in 0..count {
        let bounds = cx.update(|_, cx| view.read(cx).preview_list.bounds_for_item(ix));
        if let Some(bounds) = bounds {
            let h = f32::from(bounds.size.height);
            // Image-only blocks are taller than a text line but far shorter than the page.
            if h > 80.0 {
                image_heights.push(h);
            }
        }
    }
    assert!(
        !image_heights.is_empty(),
        "expected at least one image block to be measured"
    );
    let tallest = image_heights.iter().cloned().fold(0.0_f32, f32::max);
    assert!(
        tallest <= MAX_IMAGE_DISPLAY_HEIGHT + 40.0,
        "a tall image must be capped near {MAX_IMAGE_DISPLAY_HEIGHT}px, got {tallest}px (blocks: {image_heights:?})"
    );
}
