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
            let bytes = diagram_texture_bytes(file.path());
            Ok(Arc::new(Svg { file, bytes }))
        })
    }
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
}
static REQUESTS: LazyLock<Mutex<Requests>> = LazyLock::new(|| Mutex::new(Requests::default()));

pub(super) fn retire(evicted: &[PathBuf], cx: &mut App) {
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
    for source in sources {
        cx.remove_asset::<SvgAsset>(&source);
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
                let mut cache = DIAGRAM_CACHE.lock().unwrap();
                if cache.get(&key).is_none() {
                    cache.insert_sized(key, path.clone(), svg.bytes);
                }
                drop(cache);
                render_mermaid_container(path, hash, &self.theme, self.strings, self.open.as_ref())
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
                        .font_family(crate::platform::MONO_FONT)
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
