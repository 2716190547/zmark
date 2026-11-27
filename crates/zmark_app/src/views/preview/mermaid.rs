use super::*;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, LazyLock, Mutex},
};

#[derive(Clone, Hash)]
struct Source {
    code: Arc<str>,
    dark: bool,
}
struct SvgAsset;
struct Svg {
    file: tempfile::NamedTempFile,
    bytes: usize,
    size: (f32, f32),
}
impl Asset for SvgAsset {
    type Source = Source;
    type Output = Result<Arc<Svg>, Arc<str>>;
    fn load(
        source: Source,
        cx: &mut App,
    ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
        cx.background_executor().spawn(async move {
            use std::io::Write;
            if source.code.len() > 64 * 1024 || source.code.lines().take(2001).count() > 2000 {
                return Err(Arc::from("Mermaid 输入超过 64KiB 或 2000 行，请拆分图表"));
            }
            let svg = zmark_core::mermaid::render_to_svg_themed(&source.code, source.dark)
                .map_err(Arc::<str>::from)?;
            let mut file = tempfile::Builder::new()
                .prefix("zmark-diagram-")
                .suffix(".svg")
                .tempfile()
                .map_err(|e| Arc::<str>::from(e.to_string()))?;
            file.write_all(svg.as_bytes())
                .map_err(|e| Arc::<str>::from(e.to_string()))?;
            let (w, h) = zmark_core::mermaid::extract_svg_dimensions(&svg);
            let size = (w as f32, h as f32);
            let bytes = (size.0 * size.1 * 16.0).ceil() as usize;
            Ok(Arc::new(Svg { file, bytes, size }))
        })
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct RasterSource {
    path: PathBuf,
    width: u32,
    height: u32,
}
struct RasterAsset;
impl Asset for RasterAsset {
    type Source = RasterSource;
    type Output = Result<Arc<RenderImage>, Arc<str>>;
    fn load(
        source: RasterSource,
        cx: &mut App,
    ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
        let renderer = cx.svg_renderer();
        cx.background_executor().spawn(async move {
            let bytes = std::fs::read(&source.path).map_err(|e| Arc::<str>::from(e.to_string()))?;
            let svg = renderer
                .parse_svg(&bytes)
                .map_err(|e| Arc::<str>::from(e.to_string()))?;
            renderer
                .render_parsed(
                    &svg,
                    SvgSize::ExactSize(size(
                        DevicePixels(source.width as i32),
                        DevicePixels(source.height as i32),
                    )),
                )
                .map_err(|e| Arc::<str>::from(e.to_string()))
        })
    }
}

/// Round upward to limit resize churn; cap allocation independently of SVG size.
fn preview_pixels(natural: (f32, f32), width: f32, scale: f32) -> (u32, u32) {
    let desired = width.max(1.0).min(natural.0.max(1.0)) * scale.clamp(1.0, 4.0);
    let width = (desired / 64.0).ceil() * 64.0;
    let height = width * natural.1.max(1.0) / natural.0.max(1.0);
    let ratio = (4_000_000.0 / (width * height))
        .sqrt()
        .min(4000.0 / width.max(height))
        .min(1.0);
    (
        (width * ratio).floor().max(1.0) as u32,
        (height * ratio).floor().max(1.0) as u32,
    )
}

struct Request {
    source: Source,
    path: Option<PathBuf>,
    frame: u64,
}
#[derive(Default)]
struct Requests {
    frame: u64,
    entries: HashMap<(u64, bool), Request>,
    rasters: HashMap<RasterSource, u64>,
}
static REQUESTS: LazyLock<Mutex<Requests>> = LazyLock::new(|| Mutex::new(Requests::default()));

pub(super) fn retire(evicted: &[PathBuf], window: &mut Window, cx: &mut App) {
    let sources = {
        let mut requests = REQUESTS.lock().unwrap();
        requests.frame += 1;
        let frame = requests.frame;
        let mut sources = Vec::new();
        requests.entries.retain(|_, entry| {
            let remove = entry
                .path
                .as_ref()
                .is_some_and(|path| evicted.contains(path))
                || entry.path.is_none() && entry.frame + 1 < frame;
            if remove {
                sources.push(entry.source.clone());
            }
            !remove
        });
        sources
    };
    release_images(evicted, window, cx);
    for source in sources {
        cx.remove_asset::<SvgAsset>(&source);
    }
}

impl Requests {
    fn take_retired_rasters(&mut self, paths: &[PathBuf]) -> Vec<RasterSource> {
        let mut retired = Vec::new();
        self.rasters.retain(|source, _| {
            if paths.contains(&source.path) {
                retired.push(source.clone());
                false
            } else {
                true
            }
        });
        let mut bytes: u64 = self
            .rasters
            .keys()
            .map(|s| u64::from(s.width) * u64::from(s.height) * 4)
            .sum();
        while bytes > 16 * 1024 * 1024 {
            let oldest = self
                .rasters
                .iter()
                .filter(|(_, frame)| **frame + 1 < self.frame)
                .min_by_key(|(_, frame)| **frame)
                .map(|(source, _)| source.clone());
            let Some(source) = oldest else { break };
            bytes = bytes.saturating_sub(u64::from(source.width) * u64::from(source.height) * 4);
            self.rasters.remove(&source);
            retired.push(source);
        }
        retired
    }
}

pub(super) fn release_images(paths: &[PathBuf], window: &mut Window, cx: &mut App) {
    let retired = REQUESTS.lock().unwrap().take_retired_rasters(paths);
    for source in retired {
        if let Some(Ok(image)) = window.get_asset::<RasterAsset>(&source, cx) {
            crate::views::lightbox::drop_image_from_atlas(cx, image, window);
        }
        cx.remove_asset::<RasterAsset>(&source);
    }
}

pub(super) fn themed_path(
    hash: u64,
    dark: bool,
    window: &mut Window,
    cx: &mut App,
) -> Option<PathBuf> {
    let code = REQUESTS
        .lock()
        .unwrap()
        .entries
        .iter()
        .find(|((candidate, _), _)| *candidate == hash)
        .map(|(_, request)| request.source.code.clone())?;
    let source = Source { code, dark };
    let result = window.use_asset::<SvgAsset>(&source, cx);
    let path = result
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map(|svg| svg.file.path().to_path_buf());
    let mut requests = REQUESTS.lock().unwrap();
    let frame = requests.frame;
    requests.entries.insert(
        (hash, dark),
        Request {
            source,
            path: path.clone(),
            frame,
        },
    );
    drop(requests);
    if let Some(Ok(svg)) = result {
        let mut cache = DIAGRAM_CACHE.lock().unwrap();
        if cache.get(&(hash, dark)).is_none() {
            cache.insert_sized((hash, dark), svg.file.path().to_path_buf(), svg.bytes);
        }
    }
    path
}

#[derive(IntoElement)]
pub(super) struct Mermaid {
    pub code: Arc<str>,
    pub theme: ThemeColors,
    pub strings: &'static I18nStrings,
    pub open: Option<OnOpenLightboxCallback>,
    pub width: f32,
}
impl RenderOnce for Mermaid {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.code.hash(&mut hasher);
        let hash = hasher.finish();
        let key = (hash, self.theme.is_dark);
        let source = Source {
            code: self.code,
            dark: self.theme.is_dark,
        };
        let result = window.use_asset::<SvgAsset>(&source, cx);
        {
            let mut requests = REQUESTS.lock().unwrap();
            let frame = requests.frame;
            let path = result
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .map(|svg| svg.file.path().to_path_buf());
            requests.entries.insert(
                key,
                Request {
                    source,
                    path,
                    frame,
                },
            );
        }
        match result {
            Some(Ok(svg)) => {
                let path = svg.file.path().to_path_buf();
                let display_width = (self.width - 34.0).max(1.0).min(svg.size.0);
                let display = (
                    display_width,
                    display_width * svg.size.1 / svg.size.0.max(1.0),
                );
                let (width, height) =
                    preview_pixels(svg.size, display_width, window.scale_factor());
                let source = RasterSource {
                    path: path.clone(),
                    width,
                    height,
                };
                {
                    let mut requests = REQUESTS.lock().unwrap();
                    let frame = requests.frame;
                    requests.rasters.insert(source.clone(), frame);
                }
                DIAGRAM_CACHE.lock().unwrap().insert_sized(
                    key,
                    path.clone(),
                    width as usize * height as usize * 4,
                );
                let image = match window.use_asset::<RasterAsset>(&source, cx) {
                    Some(Ok(image)) => Some(image),
                    Some(Err(error)) => {
                        return div()
                            .p_3()
                            .text_color(self.theme.text_muted)
                            .child(format!("图表栅格化失败：{error}"))
                            .into_any_element()
                    }
                    None => None,
                };
                render_mermaid_container(
                    path,
                    image,
                    display,
                    hash,
                    &self.theme,
                    self.strings,
                    self.open.as_ref(),
                )
            }
            Some(Err(error)) => div()
                .w_full()
                .my_3()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(self.theme.border)
                .bg(self.theme.bg)
                .text_color(self.theme.text_muted)
                .text_xs()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_color(self.theme.accent)
                        .child("Mermaid Syntax Error"),
                )
                .child(
                    div()
                        .font_family(crate::platform::mono_font())
                        .child(error.to_string()),
                )
                .into_any_element(),
            None => div()
                .w_full()
                .my_3()
                .p_3()
                .text_xs()
                .text_color(self.theme.text_muted)
                .child("Mermaid · 渲染中…")
                .into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Mermaid, Source, SvgAsset};
    use gpui::{AppContext, Context, IntoElement, Render, TestAppContext, Window};
    use std::sync::Arc;
    struct Card {
        dark: bool,
    }
    impl Render for Card {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Mermaid {
                code: Arc::from("graph TD\nStart[Start] --> End[End]"),
                theme: crate::theme::ThemeColors::resolve("default", self.dark),
                strings: zmark_core::i18n::get_strings(zmark_core::i18n::SupportedLocale::En),
                open: None,
                width: 640.0,
            }
        }
    }
    #[gpui::test]
    fn visible_mermaid_uses_background_asset_and_refreshes_the_theme(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, _| Card { dark: false });
        for _ in 0..4 {
            cx.update(|window, cx| {
                super::super::release_evicted_diagram_images(window, cx);
                window.draw(cx).clear(cx);
            });
            cx.run_until_parked();
        }
        let source = Source {
            code: Arc::from("graph TD\nStart[Start] --> End[End]"),
            dark: false,
        };
        let old = cx.update(|_, cx| cx.fetch_asset::<SvgAsset>(&source).unwrap().unwrap());
        assert!(old.file.path().is_file());
        cx.update(|_, cx| {
            view.update(cx, |card, cx| {
                card.dark = true;
                cx.notify();
            })
        });
        for _ in 0..4 {
            cx.update(|window, cx| {
                super::super::release_evicted_diagram_images(window, cx);
                window.draw(cx).clear(cx);
            });
            cx.run_until_parked();
        }
        let dark = Source {
            code: source.code.clone(),
            dark: true,
        };
        let new = cx.update(|_, cx| cx.fetch_asset::<SvgAsset>(&dark).unwrap().unwrap());
        assert_ne!(old.file.path(), new.file.path());
        assert_ne!(
            std::fs::read_to_string(old.file.path()).unwrap(),
            std::fs::read_to_string(new.file.path()).unwrap()
        );
    }

    #[gpui::test]
    fn native_asset_deduplicates_success_and_failure(cx: &mut TestAppContext) {
        let source = Source {
            code: Arc::from("graph TD\nAlpha[中文] --> Beta[Next]\n"),
            dark: false,
        };
        cx.update(|cx| {
            assert!(cx.fetch_asset::<SvgAsset>(&source).is_none());
            assert!(cx.fetch_asset::<SvgAsset>(&source).is_none());
        });
        cx.run_until_parked();
        let svg = cx.update(|cx| cx.fetch_asset::<SvgAsset>(&source).unwrap().unwrap());
        let again = cx.update(|cx| cx.fetch_asset::<SvgAsset>(&source).unwrap().unwrap());
        assert!(
            Arc::ptr_eq(&svg, &again),
            "one completed native asset is reused"
        );
        drop(again);
        let path = svg.file.path().to_path_buf();
        assert!(path.exists());
        let failed = Source {
            code: Arc::from("not-a-valid-mermaid-diagram @@@"),
            dark: false,
        };
        cx.update(|cx| {
            cx.fetch_asset::<SvgAsset>(&failed);
        });
        cx.run_until_parked();
        let error = cx.update(|cx| cx.fetch_asset::<SvgAsset>(&failed).unwrap().err().unwrap());
        let again = cx.update(|cx| cx.fetch_asset::<SvgAsset>(&failed).unwrap().err().unwrap());
        assert!(
            Arc::ptr_eq(&error, &again),
            "errors must not rerender each frame"
        );
        drop(svg);
        cx.update(|cx| {
            cx.remove_asset::<SvgAsset>(&source);
            cx.remove_asset::<SvgAsset>(&failed);
        });
        assert!(
            !path.exists(),
            "native asset owns its immutable temporary SVG"
        );
    }
}

#[cfg(test)]
mod raster_tests {
    use super::{preview_pixels, RasterSource, Requests};

    #[test]
    fn preview_pixels_follow_width_and_dpr_without_filling_the_budget() {
        assert_eq!(preview_pixels((1000.0, 600.0), 400.0, 2.0), (832, 499));
        assert_eq!(
            preview_pixels((1000.0, 600.0), 400.0, 2.0),
            preview_pixels((1000.0, 600.0), 410.0, 2.0)
        );
        let (w1, _) = preview_pixels((1000.0, 600.0), 400.0, 1.0);
        let (w3, _) = preview_pixels((1000.0, 600.0), 400.0, 3.0);
        assert!(w3 > w1);
        for natural in [(1.0, 10000.0), (10000.0, 1.0), (10000.0, 10000.0)] {
            let (w, h) = preview_pixels(natural, 10000.0, 4.0);
            assert!(w <= 4000 && h <= 4000 && u64::from(w) * u64::from(h) <= 4_000_000);
            assert!(w > 0 && h > 0);
        }
    }
    #[test]
    fn raster_budget_protects_visible_variants_and_releases_hidden_sources() {
        let a = RasterSource {
            path: "same.svg".into(),
            width: 2000,
            height: 1500,
        };
        let b = RasterSource {
            path: "same.svg".into(),
            width: 1000,
            height: 1500,
        };
        let mut requests = Requests::default();
        requests.frame = 3;
        requests.rasters.insert(a.clone(), 2);
        requests.rasters.insert(b.clone(), 2);
        assert!(requests.take_retired_rasters(&[]).is_empty());
        requests.rasters.insert(a.clone(), 1);
        assert_eq!(requests.take_retired_rasters(&[]), vec![a]);
        assert_eq!(requests.take_retired_rasters(&["same.svg".into()]), vec![b]);
        assert!(requests.rasters.is_empty());
    }
}
