//! Real-platform pixels and atlas lifetime: `cargo run -p zmark_app --example media_probe --features test-observation`.
use gpui::*;
use std::{sync::Arc, time::Duration};
use zmark_app::{
    assets::AppAssets,
    local_images::{load_image, LocalImage},
};

struct Probe {
    image: Arc<LocalImage>,
    shared: Arc<LocalImage>,
    step: usize,
}
impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0xffffff))
            .child(if self.step == 3 {
                div().into_any_element()
            } else {
                let image = if self.step == 1 {
                    &self.shared
                } else {
                    &self.image
                };
                img(image.image_source())
                    .w(px(80.))
                    .h(px(40.))
                    .debug_selector(|| "probe_bitmap".into())
                    .into_any_element()
            })
    }
}
fn main() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("alpha.png");
    image::RgbaImage::from_pixel(8, 4, image::Rgba([240, 80, 20, 128]))
        .save(&path)
        .unwrap();
    let image = load_image(path.to_str().unwrap(), None).unwrap();
    let weak = Arc::downgrade(image.render.as_ref().unwrap());
    gpui_platform::application().with_assets(AppAssets).run(move |cx| {
        cx.activate(true);
        let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(200.), px(100.)), cx))), ..Default::default() };
        let window = cx.open_window(options, |_, cx| cx.new(|_| Probe { shared: image.clone(), image, step: 0 })).unwrap();
        let weak = weak.clone();
        cx.spawn(async move |cx| {
            for step in 0..5 {
                cx.background_executor().timer(Duration::from_millis(400)).await;
                cx.update_window(window.into(), |_, window, cx| { window.draw(cx).clear(cx); }).unwrap();
                window.update(cx, |probe, window, cx| {
                    let pixels = window.render_to_image().expect("real GPU readback");
                    let factor = window.scale_factor();
                    let pixel = pixels.get_pixel((20. * factor) as u32, (20. * factor) as u32).0;
                    if step != 3 {
                        for (actual, expected) in pixel.iter().zip([247u8, 167, 137, 255]) {
                            assert!((*actual as i16 - expected as i16).abs() <= 4, "GPU color {pixel:?}");
                        }
                    } else { assert_eq!(pixel, [255, 255, 255, 255]); }
                    eprintln!("MEDIA_PROBE step={step} real RGBA={pixel:?}");
                    if step == 2 || step == 3 {
                        probe.image.release(window, cx);
                        probe.image.release(window, cx);
                    }
                    probe.step = if step == 4 { 0 } else { step + 1 };
                    cx.notify();
                }).unwrap();
            }
            window.update(cx, |_, window, _| window.remove_window()).unwrap();
            cx.update(|_| {});
            assert!(weak.upgrade().is_none(), "closing the real window releases CPU pixels");
            eprintln!("MEDIA_PROBE PASS: real pixels, shared reuse, repeated drop/reupload, CPU release");
            cx.update(|cx| cx.quit());
        }).detach();
    });
    drop(dir);
}
