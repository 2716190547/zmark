use gpui::prelude::*;
use gpui::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::actions::{
    ExportHtml, OpenFileDialog, SaveFile, ToggleDrawer, TogglePreviewMode, ToggleSettings,
    ToggleSourceMode, ToggleSplitMode,
};
use crate::state::{parse_debounce, AppState, CloseDecision, ViewMode};
use crate::views::editor::render_editor_pane;
use crate::views::lightbox::{render_lightbox, LightboxState};
use crate::views::settings::SettingsWindowView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollOrigin {
    Editor,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    Source,
    Preview,
}

use crate::syntax::{VisibleSyntaxFailure, WarmState};

pub struct WorkspaceView {
    pub state: AppState,
    pub focus_handle: FocusHandle,
    pub editor_scroll_handle: UniformListScrollHandle,
    pub preview_list: ListState,
    pub preview_scroll_focus: FocusHandle,
    pub preview_scroll_bounds: Option<Bounds<Pixels>>,
    preview_viewport_size: Option<Size<Pixels>>,
    pub preview_scroll_drag: Option<(Bounds<Pixels>, usize, f32, f32, f32)>,
    preview_drag_ratio: Option<f32>,
    preview_measured_max: Option<(f32, f32, f32, f32)>,
    pub preview_scroll_activation_observed: bool,
    pub preview_scroll_end_pending: bool,
    preview_scroll_pending_seek: Option<(usize, f32)>,
    preview_scroll_seek_generation: u64,
    pub drawer_scroll_handle: ScrollHandle,
    pub split_ratio: f32,
    pub is_resizing_split: bool,
    pub is_renaming: bool,
    pub rename_buffer: String,
    pub workspace_width: f32,
    auto_save_tasks: HashMap<crate::state::DocKey, (u64, Task<()>)>,
    next_auto_save_id: u64,
    pub parse_task: Option<Task<()>>,
    pub parse_paused: bool,
    pub drawer_closing: bool,
    pub settings_window: Option<WindowHandle<SettingsWindowView>>,
    pub copied_code_hash: Option<(u64, std::time::Instant)>,
    pub last_scroll_origin: Option<(ScrollOrigin, std::time::Instant)>,
    pub lightbox: LightboxState,
    pub window_handle: Option<AnyWindowHandle>,
    pub error_banner: Option<String>,
    pub search_focused: bool,
    pub preview_focused: bool,
    pub search_input: Option<Entity<crate::views::drawer::SearchInput>>,
    pub preview_selection: Option<crate::views::preview::PreviewSelectionRange>,
    pub is_preview_selecting: bool,
    pub preview_autoscroll_task: Option<gpui::Task<()>>,
    pub preview_mouse_window_pos: Option<Point<Pixels>>,
    pub preview_content_bounds: Option<Bounds<Pixels>>,
    pub editor_content_bounds: Option<Bounds<Pixels>>,
    pub document_layout_cache: crate::views::editor::DocumentLayoutCache,
    pub cached_visual_rev: usize,
    pub cached_visual_width_bits: u64,
    pub cached_visual_font_bits: u32,
    pub cached_visual_lines: Arc<Vec<crate::editor_engine::VisualLine>>,
    pub rendered_regions: Arc<crate::views::preview::RegionsRegistry>,
    pub local_images: crate::local_images::SharedImages,
    pub local_image_task: Option<gpui::Task<()>>,
    pub syntax_active_task: Option<gpui::Task<()>>,
    pub syntax_pending_request: Option<SyntaxBatchRequest>,
    pub syntax_cancel_flag: Arc<AtomicBool>,
    pub syntax_request_counter: u64,
    pub syntax_cache: Arc<std::sync::Mutex<glidemark_core::syntax::SyntaxCache>>,
    pub syntax_attempts: std::collections::HashMap<glidemark_core::syntax::CodeKey, (u8, u8)>,
    pub syntax_block_states: std::collections::HashMap<glidemark_core::syntax::CodeKey, SyntaxBlockState>,
    pub syntax_defer_armed: bool,
    pub last_visible_syntax_keys: std::collections::HashSet<glidemark_core::syntax::CodeKey>,
    pub source_code_blocks: Arc<Vec<glidemark_core::ast::ParsedCodeBlock>>,
    pub source_code_blocks_rev: usize,
    pub syntax_warm_states: std::collections::HashMap<&'static str, WarmState>,
    pub syntax_warm_task_id: u64,
    pub syntax_warm_task: Option<gpui::Task<()>>,
    pub syntax_warm_pending: bool,
    pub syntax_invalid_positions: std::collections::HashSet<(glidemark_core::syntax::CodeKey, usize)>,
    pub current_reading_pane: Option<PaneKind>,
    pub last_reading_pane: Option<PaneKind>,
    pub current_ordered_candidates: Vec<glidemark_core::syntax::CodeKey>,
    pub last_evaluated_reading_pane: Option<PaneKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxBlockState {
    Ready,
    BudgetExceeded,
    ResourceConstrained,
    HardLimit,
    UnsupportedLanguage,
    MappingInvalid,
    InitFailed,
    WaitingWarm,
}

impl SyntaxBlockState {
    pub fn localized_reason<'a>(&self, strings: &'a glidemark_core::i18n::I18nStrings) -> Option<&'a str> {
        match self {
            SyntaxBlockState::Ready | SyntaxBlockState::UnsupportedLanguage | SyntaxBlockState::WaitingWarm => None,
            SyntaxBlockState::BudgetExceeded => Some(strings.syntax_budget_exceeded),
            SyntaxBlockState::ResourceConstrained => Some(strings.syntax_resource_constrained),
            SyntaxBlockState::HardLimit => Some(strings.syntax_hard_limit),
            SyntaxBlockState::MappingInvalid => Some(strings.syntax_mapping_invalid),
            SyntaxBlockState::InitFailed => Some(strings.syntax_init_failed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SyntaxBatchRequest {
    pub request_id: u64,
    pub doc_key: Option<crate::state::DocKey>,
    pub revision: usize,
    pub blocks: Vec<glidemark_core::syntax::CodeKey>,
    pub ordered_candidates: Vec<glidemark_core::syntax::CodeKey>,
    pub reading_pane: Option<PaneKind>,
}

impl WorkspaceView {
    pub fn new(state: AppState, cx: &mut Context<Self>) -> Self {
        let preview_list = ListState::new(
            state.doc().parsed.blocks.len(),
            ListAlignment::Top,
            px(600.),
        );
        let search_focus = cx.focus_handle();
        let view_search = cx.entity().downgrade();
        let search_placeholder = state.strings.search_placeholder;
        let search_input = cx.new(|_| {
            let view = view_search.clone();
            crate::views::drawer::SearchInput::new(search_focus)
                .with_placeholder(search_placeholder)
                .on_change(move |new_q, _window, cx| {
                    let view = view.clone();
                    let q = new_q.to_string();
                    cx.spawn(async move |_this, app| {
                        let _ = view.update(app, |workspace, cx| {
                            workspace.update_search_query(&q, cx);
                        });
                    }).detach();
                })
        });
        let mut view = Self {
            state,
            focus_handle: cx.focus_handle(),
            preview_scroll_focus: cx.focus_handle().tab_stop(true),
            preview_scroll_bounds: None,
            preview_viewport_size: None,
            preview_scroll_drag: None,
            preview_drag_ratio: None,
            preview_measured_max: None,
            preview_scroll_activation_observed: false,
            preview_scroll_end_pending: false,
            preview_scroll_pending_seek: None,
            preview_scroll_seek_generation: 0,
            editor_scroll_handle: UniformListScrollHandle::new(),
            preview_list,
            drawer_scroll_handle: ScrollHandle::new(),
            split_ratio: 0.5,
            is_resizing_split: false,
            is_renaming: false,
            rename_buffer: String::new(),
            workspace_width: 1200.0,
            auto_save_tasks: HashMap::new(),
            next_auto_save_id: 0,
            parse_task: None,
            parse_paused: false,
            drawer_closing: false,
            settings_window: None,
            copied_code_hash: None,
            last_scroll_origin: None,
            lightbox: LightboxState::new(),
            window_handle: None,
            error_banner: None,
            search_focused: false,
            preview_focused: false,
            search_input: Some(search_input),
            preview_selection: None,
            is_preview_selecting: false,
            preview_autoscroll_task: None,
            preview_mouse_window_pos: None,
            preview_content_bounds: None,
            editor_content_bounds: None,
            document_layout_cache: crate::views::editor::DocumentLayoutCache::new(),
            cached_visual_rev: usize::MAX,
            cached_visual_width_bits: 0,
            cached_visual_font_bits: 0,
            cached_visual_lines: Arc::new(Vec::new()),
            rendered_regions: Arc::new(std::sync::Mutex::new(Vec::new())),
            local_images: Arc::new(std::sync::Mutex::new(crate::local_images::LocalImages::default())),
            local_image_task: None,
            syntax_active_task: None,
            syntax_pending_request: None,
            syntax_cancel_flag: Arc::new(AtomicBool::new(false)),
            syntax_request_counter: 0,
            syntax_cache: Arc::new(std::sync::Mutex::new(glidemark_core::syntax::SyntaxCache::new())),
            syntax_attempts: std::collections::HashMap::new(),
            syntax_block_states: std::collections::HashMap::new(),
            syntax_defer_armed: false,
            last_visible_syntax_keys: std::collections::HashSet::new(),
            source_code_blocks: Arc::new(Vec::new()),
            source_code_blocks_rev: usize::MAX,
            syntax_warm_states: {
                let mut map = std::collections::HashMap::new();
                for &lang in &glidemark_core::syntax::SUPPORTED_CANONICAL_LANGUAGES {
                    map.insert(lang, WarmState::Waiting);
                }
                map
            },
            syntax_warm_task_id: 0,
            syntax_warm_task: None,
            syntax_warm_pending: false,
            syntax_invalid_positions: std::collections::HashSet::new(),
            current_reading_pane: None,
            last_reading_pane: None,
            current_ordered_candidates: Vec::new(),
            last_evaluated_reading_pane: None,
        };

        cx.on_release(|view, cx| {
            let mut images = view.local_images.lock().unwrap();
            for (_, state) in std::mem::take(&mut images.entries) {
                if let crate::local_images::ImageState::Ready(image) = state { images.retired.push(image); }
            }
            if let Some(image) = images.loading.take() { images.retired.push(image); }
            for image in images.retired.drain(..) {
                let path = image.snapshot.path().to_path_buf();
                if let Some(handle) = view.window_handle {
                    let _ = handle.update(cx, |_, window, cx| {
                        crate::views::lightbox::release_lightbox_image(&path, window, cx);
                    });
                }
                cx.remove_asset::<gpui::ImgResourceLoader>(&gpui::Resource::Path(Arc::from(path.as_path())));
            }
        }).detach();
        view.start_warm_coordination_task(cx);

        view
    }

    /// Swaps in a new document: the preview list drops cached heights and starts at the top.
    pub fn reset_preview_list(&mut self) {
        self.preview_measured_max = None;
        self.cancel_pending_preview_seek();
        self.end_preview_scroll_drag();
        self.preview_selection = None;
        self.is_preview_selecting = false;
        self.preview_focused = false;
        self.preview_autoscroll_task = None;
        self.preview_mouse_window_pos = None;
        self.preview_list.reset(self.state.doc().parsed.blocks.len());
        self.rendered_regions.lock().unwrap().clear();
        self.syntax_cache.lock().unwrap().clear();
        self.syntax_attempts.clear();
        self.syntax_block_states.clear();
        self.last_visible_syntax_keys.clear();
        self.current_ordered_candidates.clear();
        self.syntax_defer_armed = false;
        self.syntax_cancel_flag.store(true, Ordering::Relaxed);
        self.syntax_pending_request = None;
    }

    pub fn update_search_query(&mut self, query: &str, cx: &mut Context<Self>) {
        self.state.set_search_query(query);
        if let Some(job) = self.state.prepare_search_job() {
            cx.spawn(async move |this, app| {
                let result = app.background_executor().spawn(async move {
                    job.execute()
                }).await;
                let _ = this.update(app, |view, cx| {
                    if view.state.apply_search_result(result) {
                        cx.notify();
                    }
                });
            }).detach();
        }
        cx.notify();
    }

    pub fn process_local_images(&mut self, cx: &mut Context<Self>) {
        let next = {
            let mut images = self.local_images.lock().unwrap();
            images.evict_hidden();
            if !images.retired.is_empty() { cx.notify(); return; }
            if self.local_image_task.is_some() { return; }
            images.wanted.iter().find(|url| matches!(images.entries.get(*url), Some(crate::local_images::ImageState::Waiting)))
                .map(|url| (images.generation, url.clone(), images.document.clone()))
        };
        let Some((generation, url, document)) = next else { return; };
        self.local_image_task = Some(cx.spawn(async move |this, app| {
            let load_url = url.clone();
            let (mut result, candidates) = app.background_executor().spawn(async move {
                let result = crate::local_images::load_image(&load_url, document.as_deref());
                let candidates = if result.is_err() && crate::local_images::is_windows_absolute_image(&load_url) {
                    Some(crate::local_images::image_recovery_candidates(&load_url, document.as_deref()))
                } else { None };
                (result, candidates)
            }).await;
            // Await GPUI's own resource decode before starting another image. This
            // preserves its color channel path and validates the actual bitmap.
            if let Ok(image) = &result {
                let image = image.clone();
                let allowed = this.update(app, |view, _| {
                    let mut images = view.local_images.lock().unwrap();
                    if images.generation != generation || !images.wanted.contains(&url) { return false; }
                    if images.decoded_bytes() + image.decoded_bytes > crate::local_images::MAX_CACHE_BYTES { return false; }
                    images.loading = Some(image.clone());
                    true
                }).unwrap_or(false);
                if allowed {
                    let resource = gpui::Resource::Path(Arc::from(image.snapshot.path()));
                    if let Ok(task) = app.update(|cx| cx.fetch_asset::<gpui::ImgResourceLoader>(&resource).0) {
                        match task.await {
                            Ok(render) => {
                                let size = render.size(0);
                                if size.width.0 < 1 || size.height.0 < 1 || render.frame_count() != 1
                                    || crate::local_images::check_dimensions(size.width.0 as u32, size.height.0 as u32).is_err()
                                    || size.width.0 as u32 != image.width || size.height.0 as u32 != image.height
                                    || render.as_bytes(0).is_none_or(|bytes| bytes.len() as u64 > crate::local_images::MAX_DECODED_BYTES)
                                {
                                    result = Err("图片实际解码尺寸或帧数超限".into());
                                }
                            }
                            Err(error) => result = Err(format!("图片解码失败：{error}")),
                        }
                    } else {
                        result = Err("图片加载已关闭".into());
                    }
                } else {
                    result = Err("图片已退出当前文档或可见图片缓存超过 64MiB".into());
                }
            }
            let _ = this.update(app, |view, cx| {
                view.local_image_task = None;
                let changed = {
                    let mut images = view.local_images.lock().unwrap();
                    let changed = images.complete(generation, url.clone(), result);
                    if changed {
                        if let (Some(candidates), Some(crate::local_images::ImageState::Failed(_))) = (candidates, images.entries.get(&url)) {
                            images.entries.insert(url.clone(), crate::local_images::ImageState::RecoveryCandidates(candidates));
                        }
                    }
                    if let Some(image) = images.loading.take() {
                        let retained = matches!(images.entries.get(&url), Some(crate::local_images::ImageState::Ready(i)) if Arc::ptr_eq(i, &image));
                        if !retained { images.retired.push(image); }
                    }
                    changed || !images.retired.is_empty()
                };
                if changed { view.preview_measured_max = None; view.cancel_pending_preview_seek(); view.end_preview_scroll_drag(); cx.notify(); }
                view.process_local_images(cx);
            });
        }));
    }

    /// Re-measures the preview after an edit, keeping the current scroll position.
    pub fn invalidate_preview_list(&mut self) {
        self.preview_measured_max = None;
        self.cancel_pending_preview_seek();
        self.end_preview_scroll_drag();
        let count = self.state.doc().parsed.blocks.len();
        let top = self.preview_list.logical_scroll_top();
        if count != self.preview_list.item_count() {
            self.preview_list.reset(count);
            self.preview_list.scroll_to(top);
        } else {
            self.preview_list.splice(0..count, count);
            self.preview_list.scroll_to(top);
        }
    }

    /// Debounces a reparse based on document size; very large documents wait for an explicit refresh.
    pub fn schedule_parse(&mut self, cx: &mut Context<Self>) {
        let debounce = parse_debounce(self.state.doc().buffer.len());
        self.parse_task = None;
        self.parse_paused = debounce.is_none();
        self.schedule_syntax_parse(cx);
        if let Some(delay) = debounce {
            self.parse_task = Some(cx.spawn(async move |this, app| {
                app.background_executor().timer(delay).await;
                let _ = this.update(app, |this, cx| {
                    if this.state.parse_now() {
                        this.syntax_warm_pending = false;
                        this.start_warm_coordination_task(cx);
                    }
                    this.invalidate_preview_list();
                    if this.state.view_mode == ViewMode::Split {
                        this.align_preview_to_cursor();
                    }
                    cx.notify();
                });
            }));
        }
        cx.notify();
    }

    /// Parses immediately (manual save, explicit refresh, or switching into the preview).
    pub fn parse_now_and_refresh(&mut self) {
        self.parse_task = None;
        self.parse_paused = false;
        self.preview_selection = None;
        self.is_preview_selecting = false;
        self.search_focused = false;
        self.state.sync_from_editor();
        if self.state.parse_now() {
            self.syntax_warm_pending = true;
            self.invalidate_preview_list();
            if self.state.view_mode == ViewMode::Split {
                self.align_preview_to_cursor();
            }
        }
    }

    /// Explicit user action via Refresh Preview button: resets attempt counters and triggers refresh.
    pub fn reset_syntax_attempts_and_refresh(&mut self, cx: &mut Context<Self>) {
        self.local_images.lock().unwrap().refresh();
        self.syntax_attempts.clear();
        self.syntax_block_states.retain(|_, s| matches!(s, SyntaxBlockState::Ready));
        self.last_visible_syntax_keys.clear();
        self.current_ordered_candidates.clear();
        for state in self.syntax_warm_states.values_mut() {
            if *state == WarmState::InitFailed {
                *state = WarmState::Waiting;
            }
        }
        self.parse_now_and_refresh();
        self.start_warm_coordination_task(cx);
        self.process_visible_syntax_defer(cx);
        cx.notify();
    }

    pub fn start_warm_coordination_task(&mut self, cx: &mut Context<Self>) {
        self.syntax_warm_task_id += 1;
        let task_id = self.syntax_warm_task_id;
        self.syntax_warm_task = Some(cx.spawn(async move |this, app| {
            let languages = this.read_with(app, |this, _| {
                this.state.doc().parsed.code_blocks.iter().filter_map(|block| block.key.language).collect::<std::collections::HashSet<_>>()
            }).unwrap_or_default();
            for &lang in &glidemark_core::syntax::SUPPORTED_CANONICAL_LANGUAGES {
                if !languages.contains(lang) { continue; }
                let should_run = this.read_with(app, |this, _cx| {
                    if task_id != this.syntax_warm_task_id {
                        return false;
                    }
                    this.syntax_warm_states.get(lang).copied() != Some(WarmState::Ready)
                }).unwrap_or(false);

                if !should_run {
                    continue;
                }

                let ok = app.background_executor().spawn(async move {
                    glidemark_core::syntax::validate_language_warm(lang)
                }).await;

                let success = if ok {
                    true
                } else {
                    app.background_executor().spawn(async move {
                        glidemark_core::syntax::validate_language_warm(lang)
                    }).await
                };

                let new_state = if success { WarmState::Ready } else { WarmState::InitFailed };

                let cont = this.update(app, |this, cx| {
                    if task_id == this.syntax_warm_task_id {
                        this.syntax_warm_states.insert(lang, new_state);
                        this.last_visible_syntax_keys.clear();
                        this.process_visible_syntax_defer(cx);
                        cx.notify();
                        true
                    } else {
                        false
                    }
                }).unwrap_or(false);

                if !cont {
                    break;
                }
            }
        }));
    }

    pub fn active_reading_pane(&self) -> PaneKind {
        self.current_reading_pane
            .or(self.last_reading_pane)
            .unwrap_or_else(|| {
                if matches!(self.state.view_mode, ViewMode::Preview) {
                    PaneKind::Preview
                } else {
                    PaneKind::Source
                }
            })
    }

    pub fn reevaluate_admission(&mut self, order: &[glidemark_core::syntax::CodeKey]) {
        let mut cache = self.syntax_cache.lock().unwrap();
        let mut retained = std::collections::HashSet::new();
        for key in order {
            if let Some(spans) = cache.get(key) {
                if cache.insert(key.clone(), spans, &retained) {
                    retained.insert(key.clone());
                    self.syntax_block_states.insert(key.clone(), SyntaxBlockState::Ready);
                } else {
                    self.syntax_block_states.insert(key.clone(), SyntaxBlockState::ResourceConstrained);
                }
            }
        }
    }

    fn failure_for_occurrence(&self, key: &glidemark_core::syntax::CodeKey, open_line: usize, is_valid: bool) -> Option<VisibleSyntaxFailure> {
        let state = if !is_valid || self.syntax_invalid_positions.contains(&(key.clone(), open_line)) {
            SyntaxBlockState::MappingInvalid
        } else if key.language.is_some_and(|lang| self.syntax_warm_states.get(lang) == Some(&WarmState::InitFailed)) {
            SyntaxBlockState::InitFailed
        } else {
            *self.syntax_block_states.get(key)?
        };
        state.localized_reason(self.state.strings).map(|reason| VisibleSyntaxFailure { source_line: open_line + 1, reason })
    }

    pub fn collect_visible_editor_failures(&self) -> Vec<VisibleSyntaxFailure> {
        let mut failures = Vec::new();
        if !matches!(self.state.view_mode, ViewMode::Source | ViewMode::Split) {
            return failures;
        }
        let Some(editor_bounds) = self.editor_content_bounds else {
            return failures;
        };
        if f32::from(editor_bounds.size.height) <= 0.0 {
            return failures;
        }
        let offset_y = (-f32::from(self.editor_scroll_handle.0.borrow().base_handle.offset().y)).max(0.0);
        let row_h = crate::views::editor::calculate_line_row_height(
            self.state.preferences.font_size,
            glidemark_core::config::Preferences::sanitize_line_height(self.state.preferences.line_height),
        );
        let start_vis = (offset_y / row_h).floor().max(0.0) as usize;
        let visible_h = f32::from(editor_bounds.size.height);
        let visible_line_count = (visible_h / row_h).ceil().max(1.0) as usize;
        let end_vis = start_vis + visible_line_count + 1;

        let start_phys = self.cached_visual_lines.get(start_vis).map(|vl| vl.physical_line_idx).unwrap_or(0);
        let end_phys = self.cached_visual_lines.get(end_vis.min(self.cached_visual_lines.len().saturating_sub(1))).map(|vl| vl.physical_line_idx).unwrap_or(usize::MAX);

        for b in self.source_code_blocks.iter() {
            let b_start = b.fence_open_line;
            let b_end = b.fence_close_line.unwrap_or_else(|| {
                b.lines.last().map(|l| l.source_line_idx).unwrap_or(b_start)
            });
            if b_start <= end_phys && b_end >= start_phys && !b.key.payload.is_empty() {
                if let Some(failure) = self.failure_for_occurrence(&b.key, b.fence_open_line, b.is_valid) {
                    failures.push(failure);
                }
            }
        }
        failures
    }

    pub fn collect_visible_preview_failures(&self) -> Vec<VisibleSyntaxFailure> {
        let mut failures = Vec::new();
        if !matches!(self.state.view_mode, ViewMode::Preview | ViewMode::Split) {
            return failures;
        }
        let Some(preview_bounds) = self.preview_content_bounds else {
            return failures;
        };
        let regions = self.rendered_regions.lock().unwrap();
        let mut seen = std::collections::HashSet::new();
        for r in regions.iter() {
            if let (Some(key), Some(open_line)) = (&r.content_key, r.fence_open_line) {
                if !key.payload.is_empty() && preview_bounds.intersects(&r.bounds)
                    && seen.insert((key.clone(), open_line))
                {
                    if let Some(block) = self.state.doc().parsed.code_blocks.iter()
                        .find(|b| b.key == *key && b.fence_open_line == open_line)
                    {
                        if let Some(failure) = self.failure_for_occurrence(key, open_line, block.is_valid) {
                            failures.push(failure);
                        }
                    }
                }
            }
        }
        failures
    }

    /// Aligns preview virtual list position to current editor cursor block.
    pub fn align_preview_to_cursor(&mut self) {
        self.cancel_pending_preview_seek();
        self.end_preview_scroll_drag();
        if self.state.doc().is_parse_stale() || self.state.doc().parsed.anchors.is_empty() {
            return;
        }
        let cursor_line = self.state.editor.cursor().0 + 1;
        let map = glidemark_core::anchor::SyncScrollMap::new(self.state.doc().parsed.anchors.clone());
        let (block_index, _) = map.source_line_to_preview_progress(cursor_line);
        self.preview_list.scroll_to(ListOffset {
            item_ix: block_index,
            offset_in_item: px(0.),
        });
    }

    fn editor_text_start_offset(&self, window: &Window) -> f32 {
        let gutter = crate::views::editor::editor_gutter_width(window.text_system(), self.state.editor.lines.len(), self.state.preferences.font_size);
        crate::views::editor::editor_text_start_offset(gutter)
    }

    pub fn compute_editor_max_width_px(&self, window: &Window) -> f32 {
        let drawer_w = if self.state.drawer_open { px(crate::views::editor::DRAWER_WIDTH) } else { px(0.) };
        let available_w = match self.state.view_mode {
            ViewMode::Split => (window.viewport_size().width - drawer_w) * self.split_ratio,
            _ => window.viewport_size().width - drawer_w,
        };
        (f32::from(available_w) - self.editor_text_start_offset(window)).max(100.0)
    }

    pub fn compute_fenced_lines(&self, line_count: usize) -> Vec<bool> {
        let mut mask = vec![false; line_count];
        for block in self.source_code_blocks.iter() {
            let start = block.fence_open_line;
            let end = block.fence_close_line.unwrap_or_else(|| {
                block.lines.last().map(|l| l.source_line_idx).unwrap_or(start)
            });
            for i in start..=end.min(line_count.saturating_sub(1)) {
                mask[i] = true;
            }
        }
        mask
    }

    pub fn compute_visual_lines(&mut self, window: &Window) -> Arc<Vec<crate::editor_engine::VisualLine>> {
        let max_width_px = self.compute_editor_max_width_px(window);
        let font_size = self.state.preferences.font_size;
        let rev = self.state.editor.revision;
        let width_bits = max_width_px.round() as u64;
        let font_bits = font_size.to_bits();

        let ast_blocks_changed = !self.state.doc().is_parse_stale()
            && (self.source_code_blocks_rev != rev || *self.source_code_blocks != self.state.doc().parsed.code_blocks);

        if !self.cached_visual_lines.is_empty()
            && self.cached_visual_rev == rev
            && self.cached_visual_width_bits == width_bits
            && self.cached_visual_font_bits == font_bits
            && !ast_blocks_changed
        {
            return self.cached_visual_lines.clone();
        }

        if !self.state.doc().is_parse_stale() {
            if ast_blocks_changed || self.source_code_blocks.is_empty() {
                self.source_code_blocks = Arc::new(self.state.doc().parsed.code_blocks.clone());
                self.source_code_blocks_rev = rev;
                self.syntax_invalid_positions.clear();
                for b in self.state.doc().parsed.code_blocks.iter() {
                    if !b.is_valid {
                        self.syntax_invalid_positions.insert((b.key.clone(), b.fence_open_line));
                    }
                }
            }
        }

        let fenced_lines = self.compute_fenced_lines(self.state.editor.lines.len());
        let lines = self.document_layout_cache.update(
            window.text_system(),
            &self.state.editor.lines,
            &fenced_lines,
            max_width_px,
            font_size,
        );
        let lines_arc = Arc::new(lines);
        self.cached_visual_rev = rev;
        self.cached_visual_width_bits = width_bits;
        self.cached_visual_font_bits = font_bits;
        self.cached_visual_lines = lines_arc.clone();
        lines_arc
    }

    pub fn process_visible_syntax_defer(&mut self, cx: &mut Context<Self>) {
        self.syntax_defer_armed = false;

        struct CandidateItem {
            key: glidemark_core::syntax::CodeKey,
            pane: PaneKind,
            dist_to_center: f32,
            doc_line: usize,
        }

        let mut candidates: Vec<CandidateItem> = Vec::new();

        // 1. Preview candidates from rendered preview regions that intersect preview content bounds
        if matches!(self.state.view_mode, ViewMode::Preview | ViewMode::Split) {
            if let Some(preview_bounds) = self.preview_content_bounds {
                let preview_center_y = f32::from(preview_bounds.origin.y + preview_bounds.size.height * 0.5);
                let regions = self.rendered_regions.lock().unwrap();
                for r in regions.iter() {
                    if let Some(ref key) = r.content_key {
                        if !key.payload.is_empty() && preview_bounds.intersects(&r.bounds) {
                            let Some(block) = self.state.doc().parsed.code_blocks.iter().find(|b|
                                Some(b.fence_open_line) == r.fence_open_line && b.key == *key) else { continue; };
                            let doc_line = block.fence_open_line;
                            if !block.is_valid || self.syntax_invalid_positions.contains(&(key.clone(), doc_line)) {
                                continue;
                            }

                            let region_mid_y = f32::from(r.bounds.origin.y + r.bounds.size.height * 0.5);
                            let dist = (region_mid_y - preview_center_y).abs();
                            candidates.push(CandidateItem {
                                key: key.clone(),
                                pane: PaneKind::Preview,
                                dist_to_center: dist,
                                doc_line,
                            });
                        }
                    }
                }
            }
        }

        // 2. Editor candidates from measured bounds
        if matches!(self.state.view_mode, ViewMode::Source | ViewMode::Split) {
            if let Some(editor_bounds) = self.editor_content_bounds {
                if f32::from(editor_bounds.size.height) > 0.0 {
                    let offset_y = (-f32::from(self.editor_scroll_handle.0.borrow().base_handle.offset().y)).max(0.0);
                    let row_h = crate::views::editor::calculate_line_row_height(
                        self.state.preferences.font_size,
                        glidemark_core::config::Preferences::sanitize_line_height(self.state.preferences.line_height),
                    );
                    let visible_h = f32::from(editor_bounds.size.height);
                    let viewport_center_y = offset_y + visible_h * 0.5;
                    let start_vis = (offset_y / row_h).floor().max(0.0) as usize;
                    let visible_line_count = (visible_h / row_h).ceil().max(1.0) as usize;
                    let end_vis = start_vis + visible_line_count + 1;

                    let start_phys = self.cached_visual_lines.get(start_vis).map(|vl| vl.physical_line_idx).unwrap_or(0);
                    let end_phys = self.cached_visual_lines.get(end_vis.min(self.cached_visual_lines.len().saturating_sub(1))).map(|vl| vl.physical_line_idx).unwrap_or(usize::MAX);

                    for b in self.source_code_blocks.iter() {
                        let b_start = b.fence_open_line;
                        let b_end = b.fence_close_line.unwrap_or_else(|| {
                            b.lines.last().map(|l| l.source_line_idx).unwrap_or(b_start)
                        });
                        if b_start <= end_phys && b_end >= start_phys && !b.key.payload.is_empty() {
                            if self.syntax_invalid_positions.contains(&(b.key.clone(), b.fence_open_line)) {
                                continue;
                            }
                            let block_mid_line = (b_start + b_end) as f32 * 0.5;
                            let block_mid_y = block_mid_line * row_h;
                            let dist = (block_mid_y - viewport_center_y).abs();
                            candidates.push(CandidateItem {
                                key: b.key.clone(),
                                pane: PaneKind::Source,
                                dist_to_center: dist,
                                doc_line: b.fence_open_line,
                            });
                        }
                    }
                }
            } else if matches!(self.state.view_mode, ViewMode::Source) {
                // In SourceOnly without bounds, wait for prepaint
                return;
            }
        }

        // Sort candidates:
        // 1) active reading pane first
        // 2) relative distance to viewport center (smaller first)
        // 3) document line order (smaller first)
        let reading_pane = self.active_reading_pane();
        candidates.sort_by(|a, b| {
            let a_reading = a.pane == reading_pane;
            let b_reading = b.pane == reading_pane;
            if a_reading != b_reading {
                return b_reading.cmp(&a_reading);
            }
            match a.dist_to_center.partial_cmp(&b.dist_to_center) {
                Some(core::cmp::Ordering::Equal) | None => {}
                Some(ord) => return ord,
            }
            a.doc_line.cmp(&b.doc_line)
        });

        // Deduplicate: preserve highest priority position
        let mut ordered_visible_keys: Vec<glidemark_core::syntax::CodeKey> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for c in candidates {
            if seen.insert(c.key.clone()) {
                ordered_visible_keys.push(c.key);
            }
        }

        let order_changed = ordered_visible_keys != self.current_ordered_candidates
            || self.current_reading_pane != self.last_evaluated_reading_pane;
        let current_keys_set: std::collections::HashSet<_> = ordered_visible_keys.iter().cloned().collect();

        if !order_changed && current_keys_set == self.last_visible_syntax_keys {
            return;
        }

        self.current_ordered_candidates = ordered_visible_keys.clone();
        self.last_evaluated_reading_pane = self.current_reading_pane;
        self.last_visible_syntax_keys = current_keys_set;

        // Find missing keys that need parsing
        let mut missing_blocks = Vec::new();
        let mut cache = self.syntax_cache.lock().unwrap();

        for key in &ordered_visible_keys {
            if let Some(lang) = key.language {
                if let Some(state) = self.syntax_warm_states.get(lang) {
                    match state {
                        WarmState::Waiting => {
                            self.syntax_block_states.insert(key.clone(), SyntaxBlockState::WaitingWarm);
                            continue;
                        }
                        WarmState::InitFailed => {
                            self.syntax_block_states.insert(key.clone(), SyntaxBlockState::InitFailed);
                            continue;
                        }
                        WarmState::Ready => {}
                    }
                }
            }

            if cache.get(key).is_none() {
                if key.language.is_none() || key.payload.is_empty() {
                    let spans = Arc::from([glidemark_core::syntax::SyntaxSpan {
                        range: 0..key.payload.len(),
                        role: glidemark_core::syntax::SyntaxRole::Plain,
                    }]);
                    let empty_retained = std::collections::HashSet::new();
                    cache.insert(key.clone(), spans, &empty_retained);
                    continue;
                }

                let (gen, count) = self.syntax_attempts.entry(key.clone()).or_insert((0, 0));
                if *count < 2 {
                    *count += 1;
                    missing_blocks.push(key.clone());
                } else if *gen == 0 && glidemark_core::syntax::is_syntax_set_ready() {
                    *gen = 1;
                    *count = 1;
                    missing_blocks.push(key.clone());
                }
            }
        }
        drop(cache);

        if missing_blocks.is_empty() {
            self.reevaluate_admission(&ordered_visible_keys);
            // Only repaint: no new syntax work means the block layout is unchanged, and
            // re-splicing the virtual list on every wheel tick flickered and reset heights.
            cx.notify();
            return;
        }

        self.syntax_request_counter += 1;
        let request = SyntaxBatchRequest {
            request_id: self.syntax_request_counter,
            doc_key: self.state.active.clone(),
            revision: self.state.editor.revision,
            blocks: missing_blocks,
            ordered_candidates: self.current_ordered_candidates.clone(),
            reading_pane: self.current_reading_pane,
        };

        if self.syntax_active_task.is_some() {
            self.syntax_cancel_flag.store(true, Ordering::Relaxed);
            self.syntax_pending_request = Some(request);
        } else {
            self.run_syntax_batch(request, cx);
        }
    }

    pub fn schedule_syntax_parse(&mut self, cx: &mut Context<Self>) {
        self.process_visible_syntax_defer(cx);
    }

    pub fn run_syntax_batch(&mut self, request: SyntaxBatchRequest, cx: &mut Context<Self>) {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.syntax_cancel_flag = cancel_flag.clone();
        let cancel = cancel_flag.clone();
        let blocks = request.blocks.clone();

        self.syntax_active_task = Some(cx.spawn(async move |this, app| {
            let parsed_results = app.background_executor().spawn(async move {
                let mut results: Vec<(glidemark_core::syntax::CodeKey, glidemark_core::syntax::ParseResult)> = Vec::with_capacity(blocks.len());
                for key in blocks {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let res = glidemark_core::syntax::parse_code_spans_bounded(
                        &key.payload,
                        key.language,
                        Some(&cancel),
                    );
                    results.push((key, res));
                }
                results
            }).await;

            let _ = this.update(app, |this, cx| {
                this.syntax_active_task = None;
                let current_doc_key = this.state.active.clone();
                let current_rev = this.state.editor.revision;
                let current_req_id = this.syntax_request_counter;

                if request.doc_key == current_doc_key
                    && request.revision == current_rev
                    && request.request_id == current_req_id
                {
                    let mut retry_blocks = Vec::new();
                    let mut new_ready = std::collections::HashMap::new();

                    for (key, res) in parsed_results {
                        match res {
                            glidemark_core::syntax::ParseResult::Ready(spans) => {
                                // Admission below uses only the current ordered candidates;
                                // results outside that snapshot are dropped with this map.
                                new_ready.insert(key, spans);
                            }
                            glidemark_core::syntax::ParseResult::BudgetExceeded => {
                                let (_gen, count) = this.syntax_attempts.entry(key.clone()).or_insert((0, 1));
                                if *count < 2 {
                                    *count += 1;
                                    retry_blocks.push(key);
                                } else {
                                    this.syntax_block_states.insert(key, SyntaxBlockState::BudgetExceeded);
                                }
                            }
                            glidemark_core::syntax::ParseResult::Cancelled => {}
                            glidemark_core::syntax::ParseResult::HardLimit => {
                                this.syntax_block_states.insert(key, SyntaxBlockState::HardLimit);
                            }
                            glidemark_core::syntax::ParseResult::UnsupportedLanguage => {
                                this.syntax_block_states.insert(key, SyntaxBlockState::UnsupportedLanguage);
                            }
                            glidemark_core::syntax::ParseResult::InitFailed => {
                                this.syntax_block_states.insert(key, SyntaxBlockState::InitFailed);
                            }
                        }
                    }

                    let snapshot_matches = request.ordered_candidates == this.current_ordered_candidates
                        && request.reading_pane == this.current_reading_pane;

                    let eval_order = if snapshot_matches {
                        request.ordered_candidates.clone()
                    } else {
                        this.current_ordered_candidates.clone()
                    };

                    let mut cache = this.syntax_cache.lock().unwrap();
                    let mut retained = std::collections::HashSet::new();

                    for candidate in &eval_order {
                        if let Some(spans) = new_ready.remove(candidate) {
                            if cache.insert(candidate.clone(), spans, &retained) {
                                retained.insert(candidate.clone());
                                this.syntax_block_states.insert(candidate.clone(), SyntaxBlockState::Ready);
                            } else {
                                this.syntax_block_states.insert(candidate.clone(), SyntaxBlockState::ResourceConstrained);
                            }
                        } else if let Some(spans) = cache.get(candidate) {
                            if cache.insert(candidate.clone(), spans, &retained) {
                                retained.insert(candidate.clone());
                                this.syntax_block_states.insert(candidate.clone(), SyntaxBlockState::Ready);
                            } else {
                                this.syntax_block_states.insert(candidate.clone(), SyntaxBlockState::ResourceConstrained);
                            }
                        }
                    }
                    drop(cache);

                    if !retry_blocks.is_empty() {
                        this.syntax_request_counter += 1;
                        let retry_req = SyntaxBatchRequest {
                            request_id: this.syntax_request_counter,
                            doc_key: current_doc_key,
                            revision: current_rev,
                            blocks: retry_blocks,
                            ordered_candidates: this.current_ordered_candidates.clone(),
                            reading_pane: this.current_reading_pane,
                        };
                        this.run_syntax_batch(retry_req, cx);
                    }

                    this.invalidate_preview_list();
                    cx.notify();
                }

                if let Some(pending) = this.syntax_pending_request.take() {
                    this.run_syntax_batch(pending, cx);
                }
            });
        }));
    }

    /// Synchronizes editor wheel scrolling to the preview pane with 100ms echo suppression.
    pub fn handle_editor_scroll_wheel(&mut self, _event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_pending_preview_seek();
        self.end_preview_scroll_drag();
        if self.state.view_mode != ViewMode::Split || self.state.doc().is_parse_stale() || self.state.doc().parsed.anchors.is_empty() {
            return;
        }
        let now = std::time::Instant::now();
        if let Some((origin, time)) = self.last_scroll_origin {
            if origin == ScrollOrigin::Preview && now.duration_since(time) < std::time::Duration::from_millis(100) {
                return;
            }
        }
        self.last_scroll_origin = Some((ScrollOrigin::Editor, now));

        let visual_lines = self.compute_visual_lines(window);

        let scroll_y = (-f32::from(self.editor_scroll_handle.0.borrow().base_handle.offset().y)).max(0.0);
        let row_h = crate::views::editor::calculate_line_row_height(
            self.state.preferences.font_size,
            self.state.preferences.line_height,
        ).max(1.0);
        let top_vis_index = ((scroll_y / row_h).floor() as usize)
            .min(visual_lines.len().saturating_sub(1));
        let vl_opt = visual_lines.get(top_vis_index);
        let phys_line_idx = vl_opt.map(|vl| vl.physical_line_idx).unwrap_or(0);
        let phys_line = phys_line_idx + 1;
        let subline_idx = vl_opt.map(|vl| vl.subline_idx).unwrap_or(0);
        let total_sublines = visual_lines
            .iter()
            .filter(|vl| vl.physical_line_idx == phys_line_idx)
            .count()
            .max(1);

        let map = glidemark_core::anchor::SyncScrollMap::new(self.state.doc().parsed.anchors.clone());
        let (block_index, inner_progress) = map.source_line_to_preview_progress(phys_line);
        // Scale the in-block progress by the block's measured height. The old fixed
        // 100px scale made the preview crawl inside tall blocks (laggy, "not following").
        let fraction = calculate_subline_scroll_fraction(inner_progress, subline_idx, total_sublines);
        let block_height = self
            .preview_list
            .bounds_for_item(block_index)
            .map(|bounds| f32::from(bounds.size.height))
            .unwrap_or(0.0);
        let offset_px = fraction * block_height;

        self.preview_list.scroll_to(ListOffset {
            item_ix: block_index,
            offset_in_item: px(offset_px),
        });
        cx.notify();
    }

    /// Synchronizes preview wheel scrolling to the editor pane with 100ms echo suppression.
    pub fn handle_preview_scroll_wheel(&mut self, _event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_pending_preview_seek();
        self.end_preview_scroll_drag();
        self.sync_preview_scroll(window, cx);
    }

    fn sync_preview_scroll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.view_mode != ViewMode::Split || self.state.doc().is_parse_stale() || self.state.doc().parsed.anchors.is_empty() {
            return;
        }
        let now = std::time::Instant::now();
        if let Some((origin, time)) = self.last_scroll_origin {
            if origin == ScrollOrigin::Editor && now.duration_since(time) < std::time::Duration::from_millis(100) {
                return;
            }
        }
        self.last_scroll_origin = Some((ScrollOrigin::Preview, now));

        let top = self.preview_list.logical_scroll_top();
        let top_block = top.item_ix;
        // Carry the in-block progress across, otherwise the editor only moves when the
        // preview crosses a whole block ("not following" inside tall blocks).
        let block_height = self
            .preview_list
            .bounds_for_item(top_block)
            .map(|bounds| f32::from(bounds.size.height))
            .unwrap_or(0.0);
        let progress = if block_height > 1.0 {
            (f32::from(top.offset_in_item) / block_height).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let map = glidemark_core::anchor::SyncScrollMap::new(self.state.doc().parsed.anchors.clone());
        let target_line = map.preview_to_source_line(top_block, progress);
        let phys_line_idx = target_line.saturating_sub(1);

        let visual_lines = self.compute_visual_lines(window);
        let target_vis_idx = visual_lines
            .iter()
            .position(|vl| vl.physical_line_idx == phys_line_idx)
            .unwrap_or_else(|| visual_lines.len().saturating_sub(1));

        self.editor_scroll_handle.scroll_to_item_strict(
            target_vis_idx,
            ScrollStrategy::Top,
        );
        cx.notify();
    }

    fn end_preview_scroll_drag(&mut self) {
        self.preview_drag_ratio = None;
        if self.preview_scroll_drag.take().is_some() {
            self.preview_list.scrollbar_drag_ended();
        }
    }

    fn release_preview_scroll_drag(&mut self) {
        if self.preview_scroll_drag.is_some() {
            let ratio = self.preview_drag_ratio;
            self.end_preview_scroll_drag();
            self.preview_drag_ratio = ratio;
        }
    }

    fn cancel_pending_preview_seek(&mut self) {
        if self.preview_scroll_drag.is_none() {
            self.preview_drag_ratio = None;
        }
        self.preview_scroll_seek_generation = self.preview_scroll_seek_generation.wrapping_add(1);
        self.preview_scroll_end_pending = false;
        self.preview_scroll_pending_seek = None;
    }

    fn preview_scroll_to_ratio(&mut self, ratio: f32, count: usize, single_max: f32, window: &mut Window, cx: &mut Context<Self>) {
        if count == 0 { return; }
        let ratio = ratio.clamp(0.0, 1.0);
        if self.preview_scroll_drag.is_some() {
            self.preview_drag_ratio = Some(ratio);
        }
        self.cancel_pending_preview_seek();
        if let Some((max, _, _, _)) = self.preview_measured_max {
            self.preview_list.set_offset_from_scrollbar(point(px(0.), px(-ratio * max)));
        } else if count == 1 {
            self.preview_list.scroll_to(ListOffset { item_ix: 0, offset_in_item: px(single_max * ratio) });
        } else if ratio >= 1.0 {
            self.preview_scroll_end_pending = true;
            self.preview_list.scroll_to(ListOffset { item_ix: count - 1, offset_in_item: px(0.) });
        } else {
            let scaled = ratio * count as f32;
            let item_ix = (scaled.floor() as usize).min(count - 1);
            let local = scaled - item_ix as f32;
            let item_height = self.preview_list.bounds_for_item(item_ix).map(|b| f32::from(b.size.height)).unwrap_or(0.0);
            self.preview_list.scroll_to(ListOffset { item_ix, offset_in_item: px(local * item_height) });
            if local > 0.0 && item_height <= 0.0 {
                self.preview_scroll_pending_seek = Some((item_ix, local));
            }
        }
        self.last_scroll_origin = None;
        self.sync_preview_scroll(window, cx);
        cx.notify();
    }

    fn render_preview_scrollbar(&self, theme: &crate::theme::ThemeColors, cx: &mut Context<Self>) -> AnyElement {
        let count = self.state.doc().parsed.blocks.len();
        if self.preview_scroll_end_pending || self.preview_scroll_pending_seek.is_some() {
            let entity = cx.entity().clone();
            let generation = self.preview_scroll_seek_generation;
            cx.defer(move |cx| {
                let _ = entity.update(cx, |this, cx| {
                    if this.preview_scroll_seek_generation != generation { return; }
                    let count = this.state.doc().parsed.blocks.len();
                    if count == 0 { this.cancel_pending_preview_seek(); return; }
                    if this.preview_scroll_end_pending {
                        if let Some(bounds) = this.preview_list.bounds_for_item(count - 1) {
                            let viewport = this.preview_list.viewport_bounds();
                            let delta = f32::from(bounds.bottom() - viewport.bottom());
                            if delta > 1.0 {
                                let mut top = this.preview_list.logical_scroll_top();
                                top.offset_in_item += px(delta);
                                this.preview_list.scroll_to(top);
                            }
                            this.preview_scroll_end_pending = false;
                            cx.notify();
                        }
                    } else if let Some((item_ix, local)) = this.preview_scroll_pending_seek {
                        if let Some(bounds) = this.preview_list.bounds_for_item(item_ix) {
                            this.preview_list.scroll_to(ListOffset {
                                item_ix,
                                offset_in_item: px(local * f32::from(bounds.size.height)),
                            });
                            this.preview_scroll_pending_seek = None;
                            cx.notify();
                        }
                    }
                });
            });
        }
        let viewport = self.preview_list.viewport_bounds();
        let top = self.preview_list.logical_scroll_top();
        let single_max = if count == 1 {
            self.preview_list.bounds_for_item(0).map(|bounds| (f32::from(bounds.size.height) - f32::from(viewport.size.height)).max(0.0)).unwrap_or(0.0)
        } else { 0.0 };
        let all_fit = if let Some((max, _, _, _)) = self.preview_measured_max { max <= 0.0 } else {
            count > 0 && top.item_ix == 0 && top.offset_in_item == px(0.)
                && self.preview_list.bounds_for_item(count - 1).is_some_and(|last| last.bottom() <= viewport.bottom())
        };
        let enabled = !all_fit && (self.preview_measured_max.is_some() || count > 1 || single_max > 0.0);
        let at_end = !all_fit && count > 0 && self.preview_list.bounds_for_item(count - 1)
            .is_some_and(|last| last.bottom() <= viewport.bottom() + px(1.) && last.bottom() >= viewport.top());
        let ratio = if let Some(ratio) = self.preview_drag_ratio { ratio } else if let Some((max, _, _, _)) = self.preview_measured_max.filter(|(max, _, _, _)| *max > 0.0) {
            (-f32::from(self.preview_list.scroll_px_offset_for_scrollbar().y) / max).clamp(0.0, 1.0)
        } else if at_end { 1.0 } else if count > 1 {
            let local = self.preview_list.bounds_for_item(top.item_ix)
                .map(|b| (f32::from(top.offset_in_item) / f32::from(b.size.height).max(1.0)).clamp(0.0, 1.0))
                .unwrap_or(0.0);
            ((top.item_ix.min(count - 1) as f32 + local) / count as f32).clamp(0.0, 1.0)
        } else if single_max > 0.0 { (f32::from(top.offset_in_item) / single_max).clamp(0.0, 1.0) }
        else { 0.0 };
        let height = self.preview_scroll_bounds.map(|b| f32::from(b.size.height)).unwrap_or(f32::from(viewport.size.height)).max(1.0);
        let thumb_height = if enabled { 28.0_f32.min(height) } else { height };
        let thumb_top = ratio * (height - thumb_height).max(0.0);
        let bounds_view = cx.entity().clone();
        let drag_view = cx.weak_entity();
        div().id("preview_scrollbar").debug_selector(|| "preview_scrollbar".to_string())
            .track_focus(&self.preview_scroll_focus).tab_stop(true)
            .w(px(14.)).h_full().relative().bg(theme.surface)
            .child(canvas(move |bounds, window, cx| {
                bounds_view.update(cx, |this, cx| {
                    if this.preview_scroll_bounds.is_some_and(|old| old != bounds) {
                        this.cancel_pending_preview_seek();
                        this.end_preview_scroll_drag();
                    }
                    this.preview_scroll_bounds = Some(bounds);
                    let viewport = this.preview_list.viewport_bounds();
                    if this.preview_viewport_size.is_some_and(|old| old != viewport.size) {
                        this.cancel_pending_preview_seek();
                        this.end_preview_scroll_drag();
                        cx.notify();
                    }
                    this.preview_viewport_size = Some(viewport.size);
                    let width = f32::from(viewport.size.width);
                    let height = f32::from(viewport.size.height);
                    let rem = f32::from(window.rem_size());
                    if this.preview_measured_max.is_some_and(|(_, old_width, old_height, old_rem)| width != old_width || height != old_height || rem != old_rem) {
                        this.preview_measured_max = None;
                        this.cancel_pending_preview_seek();
                        this.end_preview_scroll_drag();
                        cx.notify();
                    }
                    let count = this.state.doc().parsed.blocks.len();
                    let top = this.preview_list.logical_scroll_top();
                    if this.preview_measured_max.is_none() && count > 0 && count <= 128
                        && top.item_ix == 0 && top.offset_in_item == px(0.)
                        && (0..count).all(|ix| this.preview_list.bounds_for_item(ix).is_some())
                    {
                        // The preview list has pt_6 and pb_6: 1.5 rem each.
                        let last = this.preview_list.bounds_for_item(count - 1).unwrap();
                        let max = (f32::from(last.bottom() - viewport.bottom()) + rem * 3.0).max(0.0);
                        this.preview_measured_max = Some((max, width, height, rem));
                        cx.notify();
                    }
                });
            }, move |_, _, window, _| {
                let release_view = drag_view.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture { return; }
                    let _ = drag_view.update(cx, |this, cx| {
                        if let Some((bounds, count, single_max, grab_offset, thumb)) = this.preview_scroll_drag {
                            if event.pressed_button != Some(MouseButton::Left) {
                                this.release_preview_scroll_drag();
                                cx.notify();
                            } else {
                                let available = (f32::from(bounds.size.height) - thumb).max(1.0);
                                let ratio = (f32::from(event.position.y - bounds.origin.y) - grab_offset) / available;
                                this.preview_scroll_to_ratio(ratio, count, single_max, window, cx);
                            }
                        }
                    });
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        let _ = release_view.update(cx, |this, cx| {
                            if this.preview_scroll_drag.is_some() { this.release_preview_scroll_drag(); cx.notify(); }
                        });
                    }
                });
            }).absolute().size_full())
            .child(div().debug_selector(|| "preview_scrollbar_thumb".to_string()).absolute().left(px(4.)).top(px(thumb_top)).w(px(6.)).h(px(thumb_height))
                .rounded_full().bg(theme.text_muted.opacity(if enabled { 0.55 } else { 0.20 })))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                window.focus(&this.preview_scroll_focus);
                this.preview_focused = true;
                this.is_preview_selecting = false;
                this.preview_autoscroll_task = None;
                let Some(bounds) = this.preview_scroll_bounds else { return; };
                if !enabled { return; }
                this.preview_list.scrollbar_drag_started();
                let local_y = f32::from(event.position.y - bounds.origin.y);
                let grab_offset = if (thumb_top..thumb_top + thumb_height).contains(&local_y) { local_y - thumb_top } else { thumb_height / 2.0 };
                this.preview_scroll_drag = Some((bounds, count, single_max, grab_offset, thumb_height));
                let available = (f32::from(bounds.size.height) - thumb_height).max(1.0);
                let ratio = (local_y - grab_offset) / available;
                this.preview_scroll_to_ratio(ratio, count, single_max, window, cx);
            }))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.release_preview_scroll_drag(); cx.notify();
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                let mods = event.keystroke.modifiers;
                if mods.platform || mods.control || mods.alt { return; }
                let top = this.preview_list.logical_scroll_top();
                let current = if let Some(ratio) = this.preview_drag_ratio { ratio }
                else if let Some((max, _, _, _)) = this.preview_measured_max.filter(|(max, _, _, _)| *max > 0.0) {
                    (-f32::from(this.preview_list.scroll_px_offset_for_scrollbar().y) / max).clamp(0.0, 1.0)
                } else if count > 1 {
                    let local = this.preview_list.bounds_for_item(top.item_ix).map(|b| (f32::from(top.offset_in_item) / f32::from(b.size.height).max(1.0)).clamp(0.0, 1.0)).unwrap_or(0.0);
                    ((top.item_ix.min(count - 1) as f32 + local) / count as f32).clamp(0.0, 1.0)
                } else if single_max > 0.0 { f32::from(top.offset_in_item) / single_max } else { 0.0 };
                let step = if let Some((max, _, _, _)) = this.preview_measured_max.filter(|(max, _, _, _)| *max > 0.0) {
                    (40.0 / max).min(1.0)
                } else if count > 1 { 1.0 / count as f32 } else if single_max > 0.0 { (40.0 / single_max).min(1.0) } else { 0.0 };
                let next = match event.keystroke.key.as_str() {
                    "up" | "left" => Some(current - step), "down" | "right" => Some(current + step),
                    "pageup" => Some(current - (step * 8.0).max(0.1)), "pagedown" => Some(current + (step * 8.0).max(0.1)),
                    "home" => Some(0.0), "end" => Some(1.0), _ => None,
                };
                if let Some(next) = next {
                    this.preview_scroll_to_ratio(next, count, single_max, window, cx);
                    cx.stop_propagation();
                }
            })).into_any_element()
    }

    /// A new document starts from a preview scrolled to the top, parsing only if needed.
    /// Reclaims GPU texture memory by dropping any active HiDPI lightbox image.
    pub fn on_document_swapped(&mut self, window: Option<&mut Window>, cx: &mut App) {
        self.parse_task = None;
        self.parse_paused = false;
        self.preview_selection = None;
        self.is_preview_selecting = false;
        self.search_focused = false;
        if let Some(exclusive_path) = self.lightbox.close() {
            if let Some(window) = window {
                crate::views::lightbox::release_lightbox_image(&exclusive_path, window, cx);
            } else if let Some(handle) = self.window_handle {
                let p_clone = exclusive_path.clone();
                if handle.update(cx, |_view, window, cx| {
                    crate::views::lightbox::release_lightbox_image(&p_clone, window, cx);
                }).is_err() {
                    let resource = gpui::Resource::Path(std::sync::Arc::from(exclusive_path.as_path()));
                    cx.remove_asset::<gpui::ImgResourceLoader>(&resource);
                }
            } else {
                let resource = gpui::Resource::Path(std::sync::Arc::from(exclusive_path.as_path()));
                cx.remove_asset::<gpui::ImgResourceLoader>(&resource);
            }
        }
        if self.state.view_mode != ViewMode::Source && self.state.doc().is_parse_stale() {
            self.parse_now_and_refresh();
        }
        self.reset_preview_list();
        self.syntax_warm_pending = true;
    }

    pub fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        self.cancel_pending_preview_seek();
        self.end_preview_scroll_drag();
        if mode != ViewMode::Source && self.state.parse_is_stale() {
            self.parse_now_and_refresh();
        }
        self.preview_selection = None;
        self.is_preview_selecting = false;
        self.preview_focused = false;
        self.preview_autoscroll_task = None;
        self.preview_mouse_window_pos = None;
        self.state.set_view_mode(mode);
        self.syntax_warm_pending = false;
        self.start_warm_coordination_task(cx);
        cx.notify();
    }

    pub fn on_editor_click_line(&mut self, vis_idx: usize, vis_col: usize, window: &mut Window) {
        window.focus(&self.focus_handle);
        self.search_focused = false;
        self.preview_focused = false;
        let vl = self.compute_visual_lines(window);
        let (phys_line, phys_col) = crate::editor_engine::visual_to_physical(vis_idx, vis_col, &vl);
        self.state.editor.set_cursor(phys_line, phys_col);
        self.preview_selection = None;
        self.is_preview_selecting = false;
        self.preview_autoscroll_task = None;
        self.preview_mouse_window_pos = None;
    }

    pub fn step_preview_autoscroll(
        &mut self,
        velocity: f32,
        mouse_pos: Point<Pixels>,
        _bounds: Bounds<Pixels>,
        _cx: &mut Context<Self>,
    ) {
        self.cancel_pending_preview_seek();
        let num_blocks = self.state.doc().parsed.blocks.len();
        if num_blocks == 0 {
            self.preview_autoscroll_task = None;
            return;
        }

        if velocity > 0.0 {
            let max_rendered = self.rendered_regions
                .lock()
                .unwrap()
                .iter()
                .map(|r| r.block_ix)
                .max();
            if let Some(max_bix) = max_rendered {
                if max_bix + 2 >= num_blocks {
                    self.preview_autoscroll_task = None;
                    return;
                }
            }
        }

        let old_top = self.preview_list.logical_scroll_top();
        let mut top = old_top;
        let new_offset = f32::from(top.offset_in_item) + velocity;

        if velocity > 0.0 {
            if top.item_ix + 1 >= num_blocks {
                self.preview_autoscroll_task = None;
                return;
            }
            let item_h = 24.0;
            if new_offset >= item_h {
                let advance = (new_offset / item_h).floor() as usize;
                let target_ix = top.item_ix + advance;
                if target_ix >= num_blocks.saturating_sub(1) {
                    top.item_ix = num_blocks.saturating_sub(1);
                    top.offset_in_item = px(0.0);
                } else {
                    top.item_ix = target_ix;
                    top.offset_in_item = px(new_offset % item_h);
                }
            } else {
                top.offset_in_item = px(new_offset);
            }
        } else {
            if new_offset < 0.0 {
                let item_h = 24.0;
                let retreat = ((-new_offset) / item_h).ceil() as usize;
                if retreat >= top.item_ix {
                    top.item_ix = 0;
                    top.offset_in_item = px(0.0);
                } else {
                    top.item_ix -= retreat;
                    top.offset_in_item = px(item_h - ((-new_offset) % item_h));
                }
            } else {
                top.offset_in_item = px(new_offset);
            }
        }
        self.preview_list.scroll_to(top);

        if let Some(sel) = self.preview_selection {
            let reg = self.rendered_regions.lock().unwrap();
            if let Some(head) = crate::views::preview::hit_test_point_across_all_regions(&reg, None, mouse_pos, &self.state.doc().parsed.blocks) {
                self.preview_selection = Some(crate::views::preview::PreviewSelection::new(sel.anchor, head));
            }
        }
    }

    pub fn trigger_preview_autoscroll_if_needed(&mut self, cx: &mut Context<Self>) {
        if !self.is_preview_selecting {
            self.preview_autoscroll_task = None;
            return;
        }
        let Some(mouse_pos) = self.preview_mouse_window_pos else {
            return;
        };
        let Some(bounds) = self.preview_content_bounds else {
            return;
        };
        let mouse_y = f32::from(mouse_pos.y);
        let bounds_top = f32::from(bounds.top());
        let bounds_height = f32::from(bounds.size.height);
        if crate::views::preview::detect_edge_autoscroll(mouse_y, bounds_top, bounds_height).is_some() {
            if self.preview_autoscroll_task.is_none() {
                let view = cx.entity().downgrade();
                self.preview_autoscroll_task = Some(cx.spawn(async move |_this, app| {
                    loop {
                        app.background_executor().timer(std::time::Duration::from_millis(16)).await;
                        let keep_going = view.update(app, |this, cx| {
                            if !this.is_preview_selecting || this.preview_autoscroll_task.is_none() {
                                this.preview_autoscroll_task = None;
                                return false;
                            }
                            let Some(mouse_pos) = this.preview_mouse_window_pos else {
                                this.preview_autoscroll_task = None;
                                return false;
                            };
                            let Some(bounds) = this.preview_content_bounds else {
                                this.preview_autoscroll_task = None;
                                return false;
                            };
                            let mouse_y = f32::from(mouse_pos.y);
                            let bounds_top = f32::from(bounds.top());
                            let bounds_height = f32::from(bounds.size.height);
                            let Some(vel) = crate::views::preview::detect_edge_autoscroll(mouse_y, bounds_top, bounds_height) else {
                                this.preview_autoscroll_task = None;
                                return false;
                            };
                            this.step_preview_autoscroll(vel, mouse_pos, bounds, cx);
                            if this.preview_autoscroll_task.is_none() {
                                return false;
                            }
                            cx.notify();
                            true
                        }).unwrap_or(false);
                        if !keep_going {
                            break;
                        }
                    }
                }));
            }
        }
    }

    pub fn theme(&self, _cx: &App) -> &crate::theme::ThemeColors {
        &self.state.theme
    }

    pub fn close_lightbox(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(exclusive_path) = self.lightbox.close() {
            crate::views::lightbox::release_lightbox_image(&exclusive_path, window, cx);
        }
    }

    pub fn open_lightbox(
        &mut self,
        svg_path: std::path::PathBuf,
        code_hash: u64,
        window: &mut Window,
        cx: &mut App,
    ) {
        let scale_factor = window.scale_factor();
        self.open_lightbox_with_scale(svg_path, code_hash, scale_factor, window, cx);
    }

    pub fn open_lightbox_with_scale(
        &mut self,
        svg_path: std::path::PathBuf,
        code_hash: u64,
        scale_factor: f32,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(old_exclusive) = self.lightbox.close() {
            crate::views::lightbox::release_lightbox_image(&old_exclusive, window, cx);
        }

        let is_sys_dark = matches!(
            window.appearance(),
            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
        );
        self.state.update_appearance(is_sys_dark);

        let scale_bucket = glidemark_core::mermaid::scale_factor_to_bucket(scale_factor);
        let is_dark = self.theme(cx).is_dark;
        self.lightbox.is_dark = Some(is_dark);

        let actual_svg_path = resolve_lightbox_svg_path(&svg_path, code_hash, is_dark);

        let cached_path = {
            let mut cache = crate::views::lightbox::LIGHTBOX_DIAGRAM_CACHE.lock().unwrap();
            cache.get(&(code_hash, is_dark, scale_bucket))
        };

        let hidpi_path = if let Some(path) = cached_path.filter(|p| p.exists()) {
            Some(path)
        } else {
            if let Ok(orig_svg) = std::fs::read_to_string(&actual_svg_path) {
                let (orig_w, orig_h) = glidemark_core::mermaid::extract_svg_dimensions(&orig_svg);
                let (target_w, target_h) =
                    glidemark_core::mermaid::calculate_hidpi_dimensions(orig_w, orig_h, scale_factor);
                let hidpi_svg =
                    glidemark_core::mermaid::apply_texture_pixel_budget(&orig_svg, target_w, target_h);
                let cache_dir = std::env::temp_dir().join("glidemark").join("mermaid_hidpi");
                let filename = format!(
                    "{:016x}_{}_{}_v2.svg",
                    code_hash,
                    if is_dark { "dark" } else { "light" },
                    scale_bucket
                );
                let out_path = cache_dir.join(filename);

                if std::fs::create_dir_all(&cache_dir).is_ok()
                    && std::fs::write(&out_path, &hidpi_svg).is_ok()
                    && out_path.is_file()
                {
                    {
                        let mut cache = crate::views::lightbox::LIGHTBOX_DIAGRAM_CACHE.lock().unwrap();
                        cache.insert((code_hash, is_dark, scale_bucket), out_path.clone());
                    }
                    Some(out_path)
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(hidpi_path) = hidpi_path {
            if let Some(evicted) = self.lightbox.open_hidpi(actual_svg_path, hidpi_path, code_hash) {
                crate::views::lightbox::release_lightbox_image(&evicted, window, cx);
            }
        } else {
            if let Some(evicted) = self.lightbox.open(actual_svg_path, code_hash) {
                crate::views::lightbox::release_lightbox_image(&evicted, window, cx);
            }
        }
    }

    pub fn toggle_drawer(&mut self, cx: &mut Context<Self>) {
        if self.drawer_closing {
            self.drawer_closing = false;
            self.state.drawer_open = true;
            cx.notify();
            return;
        }

        if self.state.drawer_open {
            self.drawer_closing = true;
            cx.notify();
            cx.spawn(async move |this, app| {
                app.background_executor()
                    .timer(std::time::Duration::from_millis(180))
                    .await;
                let _ = this.update(app, |this, cx| {
                    if this.drawer_closing {
                        this.state.drawer_open = false;
                        this.drawer_closing = false;
                        cx.notify();
                    }
                });
            })
            .detach();
        } else {
            self.state.drawer_open = true;
            self.drawer_closing = false;
            cx.notify();
        }
    }

    pub fn open_settings_window(&mut self, cx: &mut Context<Self>) {
        if let Some(existing) = &self.settings_window {
            if existing
                .update(cx, |_, window, _| {
                    window.activate_window();
                })
                .is_ok()
            {
                return;
            }
        }
        let workspace = cx.entity().clone();
        let ws = cx.entity().downgrade();
        let preferences = self.state.preferences.clone();
        let theme = self.state.theme.clone();
        let strings = self.state.strings;
        let current_tab = self.state.settings_tab;

        cx.defer(move |cx| {
            let bounds = Bounds::centered(None, size(px(640.), px(460.)), cx);
            let handle = cx.open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some(format!("GlideMark - {}", strings.settings).into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(14.), px(14.))),
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_window, cx| {
                    cx.new(|cx| SettingsWindowView::new(ws, current_tab, preferences, theme, strings, cx))
                },
            );
            if let Ok(h) = handle {
                let _ = workspace.update(cx, |this, _cx| {
                    this.settings_window = Some(h);
                });
            }
        });
    }

    pub fn trigger_auto_save(&mut self, cx: &mut Context<Self>) {
        if !self.state.preferences.auto_save {
            return;
        }
        let Some(key) = self.state.active.clone() else { return; };
        self.auto_save_tasks.remove(&key);
        self.next_auto_save_id = self.next_auto_save_id.wrapping_add(1);
        let task_id = self.next_auto_save_id;
        let delay_ms = self.state.preferences.auto_save_delay_ms;
        let save_key = key.clone();
        let cleanup_entity = cx.entity().downgrade();
        let task = cx.spawn(async move |this, app| {
            app.background_executor().timer(std::time::Duration::from_millis(delay_ms)).await;
            let _ = this.update(app, |this, cx| {
                let dirty = this.state.open_docs.iter().any(|doc| {
                    doc.key == save_key && doc.doc.status == glidemark_core::document::DocumentStatus::Modified
                });
                if dirty {
                    let _ = this.state.save_doc(&save_key);
                    cx.notify();
                }
                let cleanup_key = save_key.clone();
                let cleanup_entity = cleanup_entity.clone();
                cx.defer(move |cx| {
                    let _ = cleanup_entity.update(cx, |this, _| {
                        if this.auto_save_tasks.get(&cleanup_key).is_some_and(|(id, _)| *id == task_id) {
                            this.auto_save_tasks.remove(&cleanup_key);
                        }
                    });
                });
            });
        });
        self.auto_save_tasks.insert(key, (task_id, task));
    }

    fn rename_active_file(&mut self, new_name: &str, cx: &mut Context<Self>) {
        let old_key = self.state.active.clone();
        if self.state.rename_file(new_name).is_ok() && self.state.active != old_key {
            if let Some(old_key) = old_key {
                self.auto_save_tasks.remove(&old_key);
            }
            self.trigger_auto_save(cx);
        }
    }

    pub fn trigger_save(&mut self, cx: &mut Context<Self>) {
        if self.state.active.is_none() {
            return;
        }
        self.parse_now_and_refresh();
        if self.state.doc().path.is_some() {
            let _ = self.state.save();
            cx.notify();
        } else {
            let suggested_dir = self
                .state
                .workspace_roots
                .first()
                .map(|r| r.path.clone())
                .unwrap_or_else(|| {
                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                });
            let rx = cx.prompt_for_new_path(&suggested_dir, Some("Untitled.md"));
            cx.spawn(async move |this, app| {
                if let Ok(Ok(Some(path))) = rx.await {
                    let _ = this.update(app, |this, cx| {
                        if let Some(key) = this.state.active.clone() {
                            let _ = this.state.save_doc_as(&key, path);
                            this.on_document_swapped(None, cx);
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
        }
    }

    pub fn trigger_close_doc(
        &mut self,
        key: &crate::state::DocKey,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let decision = self.state.request_close(key);
        match decision {
            CloseDecision::CloseImmediately => {
                self.auto_save_tasks.remove(key);
                self.on_document_swapped(window, cx);
                cx.notify();
            }
            CloseDecision::PromptDirty { .. } => {
                self.on_document_swapped(window, cx);
                cx.notify();
            }
        }
    }

    pub fn trigger_close_active(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        if let Some(key) = self.state.active.clone() {
            self.trigger_close_doc(&key, window, cx);
        }
    }

    pub fn handle_modal_cancel(&mut self, cx: &mut Context<Self>) {
        self.state.cancel_close();
        cx.notify();
    }

    pub fn handle_cancel_remove_root(&mut self, cx: &mut Context<Self>) {
        self.state.cancel_remove_workspace_root();
        cx.notify();
    }

    pub fn handle_confirm_remove_root(&mut self, cx: &mut Context<Self>) {
        match self.state.confirm_remove_workspace_root() {
            Ok(_) => {
                self.error_banner = None;
                self.state.clear_error();
            }
            Err(e) => {
                let strings = self.state.strings;
                let msg = format!("{}: {}", strings.error_remove_failed, e);
                self.error_banner = Some(msg.clone());
                self.state.set_error(msg);
            }
        }
        cx.notify();
    }

    pub fn handle_modal_discard(&mut self, key: &crate::state::DocKey, window: &mut Window, cx: &mut Context<Self>) {
        self.auto_save_tasks.remove(key);
        self.state.confirm_close_discard(key);
        self.on_document_swapped(Some(window), cx);
        self.after_dirty_doc_resolved(Some(window), cx);
    }

    pub fn handle_modal_save(&mut self, key: &crate::state::DocKey, window: &mut Window, cx: &mut Context<Self>) {
        match self.state.confirm_close_save(key) {
            Ok(true) => {
                self.auto_save_tasks.remove(key);
                self.on_document_swapped(Some(window), cx);
                self.after_dirty_doc_resolved(Some(window), cx);
            }
            Ok(false) => {
                self.prompt_save_as_and_close(key.clone(), cx);
            }
            Err(e) => {
                eprintln!("Save error during close: {}", e);
                cx.notify();
            }
        }
    }

    pub fn prompt_save_as_and_close(&mut self, close_key: crate::state::DocKey, cx: &mut Context<Self>) {
        let was_quitting = self.state.is_quitting;
        self.state.pending_close = None;
        cx.notify();

        let suggested_dir = self
            .state
            .workspace_roots
            .first()
            .map(|r| r.path.clone())
            .unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            });
        let rx = cx.prompt_for_new_path(&suggested_dir, Some("Untitled.md"));

        cx.spawn(async move |this, app| {
            if let Ok(Ok(Some(path))) = rx.await {
                let _ = this.update(app, |this, cx| {
                    match this.state.save_doc_as(&close_key, path) {
                        Ok(()) => {
                            this.state.close_document_forced(&close_key);
                            this.on_document_swapped(None, cx);
                            this.state.is_quitting = was_quitting;
                            this.after_dirty_doc_resolved(None, cx);
                        }
                        Err(e) => {
                            eprintln!("Save As failed: {}", e);
                            this.state.is_quitting = was_quitting;
                            let title = this.state.doc().display_name_with_fallback(this.state.strings.untitled_file);
                            this.state.pending_close = Some(crate::state::PendingClose {
                                key: close_key,
                                title,
                                has_path: false,
                            });
                            cx.notify();
                        }
                    }
                });
            } else {
                let _ = this.update(app, |this, cx| {
                    this.state.is_quitting = false;
                    this.state.pending_close = None;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn after_dirty_doc_resolved(&mut self, mut window_opt: Option<&mut Window>, cx: &mut Context<Self>) {
        if self.state.is_quitting {
            if let Some(next_dirty) = self.state.next_dirty_doc_key() {
                self.state.request_close(&next_dirty);
                self.on_document_swapped(window_opt.as_deref_mut(), cx);
                cx.notify();
            } else {
                // All dirty documents resolved! Close window per ADR 0031.
                self.state.is_quitting = false;
                if let Some(window) = window_opt.as_deref_mut() {
                    window.remove_window();
                } else if let Some(h) = self.window_handle {
                    let _ = h.update(cx, |_view, window, _cx| {
                        window.remove_window();
                    });
                }
            }
        } else {
            cx.notify();
        }
    }

    pub fn on_window_should_close(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) -> bool {
        let allow = self.state.handle_window_should_close();
        self.on_document_swapped(window, cx);
        cx.notify();
        allow
    }

    pub fn trigger_draft(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        self.state.focus_or_create_draft();
        self.on_document_swapped(window, cx);
        cx.notify();
    }

    fn render_canvas_header(
        &self,
        window: &Window,
        theme: &crate::theme::ThemeColors,
        is_fullscreen: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_macos = cfg!(target_os = "macos");
        let drawer_open = self.state.drawer_open;
        let view_mode = self.state.view_mode;
        let strings = self.state.strings;
        let is_renaming = self.is_renaming;
        let rename_buffer = self.rename_buffer.clone();

        let viewport_w = f32::from(window.viewport_size().width);
        let col_w = self.state.preferences.max_column_width;
        let header_pl = px(calculate_title_header_padding(is_macos, is_fullscreen, drawer_open, view_mode, viewport_w, col_w, self.editor_text_start_offset(window)));

        // Right padding: on Windows/Linux, reserve 138px for system window controls (min/max/close).
        let header_pr = px(crate::platform::TRAILING_WINDOW_CONTROL_INSET);

        let active_key = self.state.active.clone();
        let (title_text, tooltip_text) = if active_key.is_some() {
            let title = self.state.doc().display_name_with_fallback(strings.untitled_file);
            let tooltip = if let Some(path) = &self.state.doc().path {
                path.to_string_lossy().to_string()
            } else {
                strings.draft_unsaved_hint.to_string()
            };
            (title, tooltip)
        } else {
            (strings.no_document_open.to_string(), strings.no_document_open.to_string())
        };

        div()
            .id("canvas_integrated_header")
            // 38px matches the drawer header so both icon rows share one vertical
            // center (19px), which is where the native traffic lights sit.
            .h(px(38.))
            .py(px(4.))
            .w_full()
            .pl(header_pl)
            .pr(header_pr)
            .bg(theme.surface)
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            // Left Group: Expand sidebar button (when collapsed) + Document Title
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .min_w(px(0.))
                    .when(!drawer_open, |el| {
                        el.child(render_icon_button(
                            "btn_expand_drawer",
                            format!("{} (⌘B)", strings.directory_drawer),
                            false,
                            theme,
                            render_sidebar_icon(theme.text_muted),
                            cx.listener(|this, _, _window, cx| {
                                this.toggle_drawer(cx);
                            }),
                        ))
                    })
                    .child(if is_renaming && active_key.is_some() {
                        div()
                            .id("tab_rename_box")
                            .px_2()
                            .py_0p5()
                            .bg(theme.bg)
                            .border_1()
                            .border_color(theme.accent)
                            .rounded_sm()
                            .flex()
                            .items_center()
                            .cursor_text()
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::BOLD)
                                    .line_height(px(24.))
                                    .text_color(theme.text_primary)
                                    .child(if rename_buffer.is_empty() {
                                        "Untitled.md".to_string()
                                    } else {
                                        rename_buffer.clone()
                                    }),
                            )
                            .child(
                                div()
                                    .w(px(1.5))
                                    .h(px(14.))
                                    .ml_0p5()
                                    .bg(theme.accent)
                                    .with_animation(
                                        "rename_cursor_blink",
                                        Animation::new(std::time::Duration::from_millis(600)).repeat(),
                                        |el, delta| el.opacity(if delta < 0.5 { 1.0 } else { 0.0 }),
                                    ),
                            )
                            .into_any_element()
                    } else {
                        let tooltip = tooltip_text.clone();
                        div()
                            .id("canvas_document_title")
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .min_w(px(0.))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .tooltip(move |_window, cx| {
                                let t = tooltip.clone();
                                cx.new(|_| TooltipView { text: t }).into()
                            })
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                if event.click_count == 2 && this.state.active.is_some() {
                                    if this.state.doc().path.is_some() {
                                        let name = this
                                            .state
                                            .doc()
                                            .path
                                            .as_ref()
                                            .and_then(|p| p.file_name())
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("Untitled.md");
                                        this.rename_buffer = name.to_string();
                                        this.is_renaming = true;
                                        window.focus(&this.focus_handle);
                                        cx.notify();
                                    } else {
                                        this.trigger_save(cx);
                                    }
                                }
                            }))
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::BOLD)
                                    .line_height(px(24.))
                                    .debug_selector(|| "editor_document_title".into())
                                    .text_color(if active_key.is_some() { theme.text_primary } else { theme.text_muted })
                                    .truncate()
                                    .child(title_text)
                            )
                            .into_any_element()
                    })
            )
            // Draggable Window Control Area in center between title and action tools
            .child(
                div()
                    .id("canvas_header_drag_area")
                    .flex_1()
                    .h_full()
                    .min_w(px(20.))
                    .window_control_area(WindowControlArea::Drag)
                    .on_mouse_down(MouseButton::Left, |_e, window, _| {
                        window.start_window_move();
                    })
            )
            // Right Group: Flat View Mode Switcher + Action Buttons (Save, Export, Settings)
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(render_mode_icon_tab(
                                "btn_mode_source",
                                format!("{} (⌘1)", strings.source_mode),
                                view_mode == ViewMode::Source,
                                theme,
                                render_source_mode_icon(mode_tab_colors(view_mode == ViewMode::Source, theme).1),
                                cx.listener(|this, _, _window, cx| {
                                    this.set_view_mode(ViewMode::Source, cx);
                                }),
                            ))
                            .child(render_mode_icon_tab(
                                "btn_mode_split",
                                format!("{} (⌘2)", strings.split_mode),
                                view_mode == ViewMode::Split,
                                theme,
                                render_split_mode_icon(mode_tab_colors(view_mode == ViewMode::Split, theme).1),
                                cx.listener(|this, _, _window, cx| {
                                    this.set_view_mode(ViewMode::Split, cx);
                                }),
                            ))
                            .child(render_mode_icon_tab(
                                "btn_mode_preview",
                                format!("{} (⌘3)", strings.preview_mode),
                                view_mode == ViewMode::Preview,
                                theme,
                                render_preview_mode_icon(mode_tab_colors(view_mode == ViewMode::Preview, theme).1),
                                cx.listener(|this, _, _window, cx| {
                                    this.set_view_mode(ViewMode::Preview, cx);
                                }),
                            ))
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(render_icon_button(
                                "btn_export",
                                format!("{} (⇧⌘E)", strings.export_html),
                                false,
                                theme,
                                render_export_icon(theme.text_muted),
                                cx.listener(|this, _, _window, _cx| {
                                    let html = this.state.export_standalone_html();
                                    if let Some(path) = &this.state.doc().path {
                                        let out_path = path.with_extension("html");
                                        let _ = std::fs::write(&out_path, html);
                                    }
                                }),
                            ))
                            .child(render_icon_button(
                                "btn_settings",
                                format!("{} (⌘,)", strings.settings),
                                false,
                                theme,
                                render_settings_icon(theme.text_muted),
                                cx.listener(|this, _, _window, cx| {
                                    this.open_settings_window(cx);
                                }),
                            ))
                    )
            )
    }

    fn render_remove_root_modal(&self, theme: &crate::theme::ThemeColors, cx: &mut Context<Self>) -> impl IntoElement {
        let strings = self.state.strings;
        let root_name = self
            .state
            .pending_remove_root
            .map(|idx| crate::state::root_label(&self.state.workspace_roots, idx))
            .unwrap_or_default();
        let prompt_text = strings.remove_root_confirm_prompt.replace("{}", &root_name);

        div()
            .id("remove_root_modal_backdrop")
            .absolute()
            .inset_0()
            .bg(rgba(0x00000066))
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("remove_root_modal_card")
                    .w(px(380.))
                    .p_5()
                    .rounded_xl()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_primary)
                                    .child(prompt_text),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child(strings.remove_root_confirm_detail),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .items_center()
                            .gap_2()
                            // Cancel button
                            .child(
                                div()
                                    .id("btn_remove_root_cancel")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_xs()
                                    .text_color(theme.text_primary)
                                    .hover(|s| s.bg(theme.bg))
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                        this.handle_cancel_remove_root(cx);
                                    }))
                                    .child(format!("{} (Esc)", strings.modal_cancel)),
                            )
                            // Confirm button
                            .child(
                                div()
                                    .id("btn_remove_root_confirm")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .bg(rgba(0xef44441a))
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0xef4444))
                                    .hover(|s| s.bg(rgba(0xef444433)))
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                        this.handle_confirm_remove_root(cx);
                                    }))
                                    .child(strings.remove_root_confirm),
                            ),
                    ),
            )
    }

    fn render_close_modal(&self, theme: &crate::theme::ThemeColors, cx: &mut Context<Self>) -> impl IntoElement {
        let strings = self.state.strings;
        let pending = self.state.pending_close.clone().unwrap_or(crate::state::PendingClose {
            key: crate::state::DocKey::Scratch(0),
            title: String::new(),
            has_path: false,
        });
        let title = pending.title.clone();
        let key_discard = pending.key.clone();
        let key_save = pending.key.clone();
        let prompt_text = strings.modal_save_prompt.replace("{}", &title);

        div()
            .id("dirty_close_modal_backdrop")
            .absolute()
            .inset_0()
            .bg(rgba(0x00000066))
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("dirty_close_modal_card")
                    .w(px(380.))
                    .p_5()
                    .rounded_xl()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_primary)
                                    .child(prompt_text),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child(strings.modal_save_detail),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .items_center()
                            .gap_2()
                            // Cancel button
                            .child(
                                div()
                                    .id("btn_modal_cancel")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_xs()
                                    .text_color(theme.text_primary)
                                    .hover(|s| s.bg(theme.bg))
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                        this.handle_modal_cancel(cx);
                                    }))
                                    .child(format!("{} (Esc)", strings.modal_cancel)),
                            )
                            // Discard button
                            .child(
                                div()
                                    .id("btn_modal_discard")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .bg(rgba(0xef44441a))
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0xef4444))
                                    .hover(|s| s.bg(rgba(0xef444433)))
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                        this.handle_modal_discard(&key_discard, window, cx);
                                    }))
                                    .child(format!("{} (D)", strings.modal_discard)),
                            )
                            // Save button
                            .child(
                                div()
                                    .id("btn_modal_save")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .bg(theme.accent)
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0xffffff))
                                    .hover(|s| s.opacity(0.9))
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                        this.handle_modal_save(&key_save, window, cx);
                                    }))
                                    .child(format!("{} (S)", strings.modal_save)),
                            ),
                    ),
            )
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.syntax_warm_pending {
            self.syntax_warm_pending = false;
            self.start_warm_coordination_task(cx);
        }
        if !self.preview_scroll_activation_observed {
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() { this.end_preview_scroll_drag(); cx.notify(); }
            }).detach();
            self.preview_scroll_activation_observed = true;
        }
        {
            let mut images = self.local_images.lock().unwrap();
            images.begin_frame(self.state.active.clone(), self.state.doc().path.clone());
            for image in images.retired.drain(..) {
                crate::views::lightbox::release_lightbox_image(image.snapshot.path(), window, cx);
            }
        }
        let image_view = cx.entity().clone();
        cx.defer(move |cx| image_view.update(cx, |this, cx| this.process_local_images(cx)));
        let is_sys_dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        self.state.update_appearance(is_sys_dark);
        self.workspace_width = f32::from(window.viewport_size().width);
        self.window_handle = Some(window.window_handle());
        let is_fullscreen = window.is_fullscreen();

        let current_is_dark = self.state.theme.is_dark;
        if self.lightbox.is_open
            && self.lightbox.is_dark.is_some()
            && self.lightbox.is_dark != Some(current_is_dark)
        {
            if let (Some(code_hash), Some(svg_path)) = (self.lightbox.code_hash, self.lightbox.svg_path.clone()) {
                let scale_factor = window.scale_factor();
                let saved_zoom = self.lightbox.zoom;
                let saved_pan_x = self.lightbox.pan_x;
                let saved_pan_y = self.lightbox.pan_y;
                self.open_lightbox_with_scale(svg_path, code_hash, scale_factor, window, cx);
                self.lightbox.zoom = saved_zoom;
                self.lightbox.pan_x = saved_pan_x;
                self.lightbox.pan_y = saved_pan_y;
            }
        }

        let is_focused = self.focus_handle.is_focused(window);
        if is_focused {
            let pane = if self.preview_focused { PaneKind::Preview } else { PaneKind::Source };
            self.current_reading_pane = Some(pane);
            self.last_reading_pane = Some(pane);
        } else {
            self.current_reading_pane = None;
        }

        let editor_failures = self.collect_visible_editor_failures();
        let preview_failures = self.collect_visible_preview_failures();

        if self.is_preview_selecting && !self.focus_handle.is_focused(window) {
            self.is_preview_selecting = false;
            self.preview_autoscroll_task = None;
            self.preview_mouse_window_pos = None;
        }

        let visual_lines = self.compute_visual_lines(window);
        if !self.state.doc().is_parse_stale() {
            if self.source_code_blocks_rev != self.state.editor.revision || self.source_code_blocks.is_empty() {
                self.source_code_blocks = Arc::new(self.state.doc().parsed.code_blocks.clone());
                self.source_code_blocks_rev = self.state.editor.revision;
            }
        }

        if !self.syntax_defer_armed {
            self.syntax_defer_armed = true;
            let view_defer = cx.entity().clone();
            cx.defer(move |cx| {
                view_defer.update(cx, |this, cx| {
                    this.process_visible_syntax_defer(cx);
                });
            });
        }

        let theme = &self.state.theme;
        let strings = self.state.strings;
        let view_mode = self.state.view_mode;
        let is_dirty = self.state.is_dirty();
        let has_conflict = self.state.has_conflict();
        let parse_paused = self.parse_paused;

        let view_click = cx.entity().clone();
        let on_click_line = Arc::new(move |vis_idx: usize, vis_col: usize, window: &mut Window, cx: &mut App| {
            view_click.update(cx, |this, cx| {
                this.on_editor_click_line(vis_idx, vis_col, window);
                cx.notify();
            });
        });

        let view_drawer = cx.entity().clone();
        let on_select_file = Arc::new(move |path: &std::path::PathBuf, window: &mut Window, cx: &mut App| {
            view_drawer.update(cx, |this, cx| {
                let _ = this.state.load_file(path.clone());
                this.on_document_swapped(Some(window), cx);
                cx.notify();
            });
        });

        let view_doc_switch = cx.entity().clone();
        let on_select_doc = Arc::new(move |key: &crate::state::DocKey, window: &mut Window, cx: &mut App| {
            view_doc_switch.update(cx, |this, cx| {
                this.state.switch_document(key);
                this.on_document_swapped(Some(window), cx);
                cx.notify();
            });
        });

        let view_doc_close = cx.entity().clone();
        let on_close_doc = Arc::new(move |key: &crate::state::DocKey, window: &mut Window, cx: &mut App| {
            view_doc_close.update(cx, |this, cx| {
                this.trigger_close_doc(key, Some(window), cx);
            });
        });

        let view_draft = cx.entity().clone();
        let on_new_draft = Arc::new(move |window: &mut Window, cx: &mut App| {
            view_draft.update(cx, |this, cx| {
                this.trigger_draft(Some(window), cx);
            });
        });

        let view_tab = cx.entity().clone();
        let on_select_tab = Arc::new(move |tab: crate::state::DrawerTab, _window: &mut Window, cx: &mut App| {
            view_tab.update(cx, |this, cx| {
                this.state.set_drawer_tab(tab);
                cx.notify();
            });
        });

        let view_heading = cx.entity().clone();
        let on_select_heading = Arc::new(move |block_idx: usize, _window: &mut Window, cx: &mut App| {
            view_heading.update(cx, |this, cx| {
                this.cancel_pending_preview_seek();
                this.end_preview_scroll_drag();
                this.preview_list.scroll_to(ListOffset {
                    item_ix: block_idx,
                    offset_in_item: px(0.),
                });
                cx.notify();
            });
        });

        let view_folder = cx.entity().clone();
        let on_open_folder = Arc::new(move |_window: &mut Window, cx: &mut App| {
            let rx = cx.prompt_for_paths(PathPromptOptions {
                files: false,
                directories: true,
                multiple: false,
                prompt: None,
            });
            view_folder.update(cx, |_this, cx| {
                cx.spawn(async move |this, app| {
                    if let Ok(Ok(Some(mut paths))) = rx.await {
                        if let Some(path) = paths.pop() {
                            let _ = this.update(app, |this, cx| {
                                match this.state.add_workspace_root(path) {
                                    Ok(_) => {
                                        this.error_banner = None;
                                        this.state.clear_error();
                                    }
                                    Err(e) => {
                                        let strings = this.state.strings;
                                        let msg = match e {
                                            crate::state::RootAddError::NotFound => {
                                                strings.error_root_not_found.to_string()
                                            }
                                            crate::state::RootAddError::NotADirectory => {
                                                strings.error_root_not_dir.to_string()
                                            }
                                            crate::state::RootAddError::AlreadyExists => {
                                                strings.error_root_already_exists.to_string()
                                            }
                                            crate::state::RootAddError::NestedInExisting(p) => {
                                                format!("{}: {}", strings.error_root_nested, p.display())
                                            }
                                            crate::state::RootAddError::ContainsExisting(p) => {
                                                format!("{}: {}", strings.error_root_parent, p.display())
                                            }
                                            crate::state::RootAddError::SaveFailed(err) => {
                                                format!("{}: {}", strings.error_save_failed, err)
                                            }
                                            crate::state::RootAddError::CanonicalizeFailed(p, err) => {
                                                format!("{}: {}: {}", strings.error_canonicalize_failed, p.display(), err)
                                            }
                                        };
                                        this.error_banner = Some(msg.clone());
                                        this.state.set_error(msg);
                                    }
                                }
                                cx.notify();
                            });
                        }
                    }
                })
                .detach();
            });
        });

        let view_toggle = cx.entity().clone();
        let on_toggle_drawer = Arc::new(move |_window: &mut Window, cx: &mut App| {
            view_toggle.update(cx, |this, cx| {
                this.toggle_drawer(cx);
            });
        });

        let view_toggle_root = cx.entity().clone();
        let on_toggle_root = Arc::new(move |root_idx: usize, _window: &mut Window, cx: &mut App| {
            view_toggle_root.update(cx, |this, cx| {
                if root_idx < this.state.workspace_roots.len() {
                    let path = this.state.workspace_roots[root_idx].path.clone();
                    this.state.toggle_root_folder(root_idx, &path);
                    cx.notify();
                }
            });
        });

        let view_remove_root = cx.entity().clone();
        let on_remove_root = Arc::new(move |root_idx: usize, _window: &mut Window, cx: &mut App| {
            view_remove_root.update(cx, |this, cx| {
                this.state.request_remove_workspace_root(root_idx);
                cx.notify();
            });
        });

        let view_refresh_root = cx.entity().clone();
        let on_refresh_root = Arc::new(move |root_idx: usize, _window: &mut Window, cx: &mut App| {
            view_refresh_root.update(cx, |this, cx| {
                this.state.refresh_root(root_idx);
                cx.notify();
            });
        });

        let view_tree = cx.entity().clone();
        let on_toggle_folder = Arc::new(
            move |root_idx: usize, path: &std::path::PathBuf, _window: &mut Window, cx: &mut App| {
                view_tree.update(cx, |this, cx| {
                    this.state.toggle_root_folder(root_idx, path);
                    cx.notify();
                });
            },
        );

        let view_new_file = cx.entity().clone();
        let on_new_file_in_dir = Arc::new(move |dir: &std::path::PathBuf, window: &mut Window, cx: &mut App| {
            view_new_file.update(cx, |this, cx| {
                match this.state.create_file_in_dir(dir) {
                    Ok(_) => {
                        this.on_document_swapped(Some(window), cx);
                    }
                    Err(e) => {
                        this.state.set_error(format!("Failed to create file: {}", e));
                    }
                }
                cx.notify();
            });
        });

        let loose_docs = self.state.loose_docs();

        let view_lb = cx.entity().clone();
        let on_open_lightbox = Arc::new(move |svg_path: std::path::PathBuf, hash: u64, window: &mut Window, cx: &mut App| {
            view_lb.update(cx, |this, cx| {
                this.open_lightbox(svg_path, hash, window, cx);
                cx.notify();
            });
        });

        let show_drawer = self.state.drawer_open || self.drawer_closing;
        let is_drawer_closing = self.drawer_closing;

        div()
            .key_context("Workspace")
            .track_focus(&self.focus_handle)
            .can_drop(|item, _window, _cx| {
                if let Some(paths) = item.downcast_ref::<ExternalPaths>() {
                    paths.paths().iter().any(|p| {
                        p.extension().map_or(false, |ext| ext == "md" || ext == "markdown" || ext == "txt")
                    })
                } else {
                    false
                }
            })
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                if let Some(path) = paths.paths().iter().find(|p| {
                    p.extension().map_or(false, |ext| ext == "md" || ext == "markdown" || ext == "txt")
                }) {
                    let _ = this.state.load_file(path.clone());
                    this.on_document_swapped(Some(window), cx);
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                if this.is_resizing_split {
                    let w = this.workspace_width;
                    let x = f32::from(event.position.x);
                    let left_offset = if this.state.drawer_open { crate::views::editor::DRAWER_WIDTH } else { 0.0 };
                    let avail_w = (w - left_offset).max(100.0);
                    let offset_x = (x - left_offset).max(0.0);
                    this.split_ratio = (offset_x / avail_w).clamp(0.20, 0.80);
                    cx.notify();
                }
                if this.is_preview_selecting {
                    this.preview_mouse_window_pos = Some(event.position);
                    this.trigger_preview_autoscroll_if_needed(cx);
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                this.release_preview_scroll_drag();
                if this.is_resizing_split {
                    this.is_resizing_split = false;
                }
                if this.is_preview_selecting {
                    this.is_preview_selecting = false;
                    this.preview_autoscroll_task = None;
                    this.preview_mouse_window_pos = None;
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleSourceMode, _window, cx| {
                this.set_view_mode(ViewMode::Source, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSplitMode, _window, cx| {
                this.set_view_mode(ViewMode::Split, cx);
            }))
            .on_action(cx.listener(|this, _: &TogglePreviewMode, _window, cx| {
                this.set_view_mode(ViewMode::Preview, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleDrawer, _window, cx| {
                this.toggle_drawer(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSettings, _window, cx| {
                this.open_settings_window(cx);
            }))
            .on_action(cx.listener(|_this, _: &OpenFileDialog, _window, cx| {
                let rx = cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: None,
                });
                cx.spawn(async move |this, app| {
                    if let Ok(Ok(Some(mut paths))) = rx.await {
                        if let Some(path) = paths.pop() {
                            let _ = this.update(app, |this, cx| {
                                let _ = this.state.load_file(path);
                                this.on_document_swapped(None, cx);
                                cx.notify();
                            });
                        }
                    }
                })
                .detach();
            }))
            .on_action(cx.listener(|this, _: &SaveFile, _window, cx| {
                this.trigger_save(cx);
            }))
            .on_action(cx.listener(|this, _: &ExportHtml, _window, _cx| {
                let html = this.state.export_standalone_html();
                if let Some(path) = &this.state.doc().path {
                    let out_path = path.with_extension("html");
                    let _ = std::fs::write(&out_path, html);
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.lightbox.is_open {
                    if event.keystroke.key == "escape" {
                        this.close_lightbox(window, cx);
                        cx.notify();
                        return;
                    }
                }

                if this.state.pending_remove_root.is_some() {
                    match event.keystroke.key.as_str() {
                        "escape" => {
                            this.handle_cancel_remove_root(cx);
                            return;
                        }
                        "enter" => {
                            this.handle_confirm_remove_root(cx);
                            return;
                        }
                        _ => {}
                    }
                }

                if let Some(pending) = this.state.pending_close.clone() {
                    match event.keystroke.key.as_str() {
                        "escape" => {
                            this.handle_modal_cancel(cx);
                            return;
                        }
                        "d" | "D" => {
                            this.handle_modal_discard(&pending.key, window, cx);
                            return;
                        }
                        "enter" | "s" | "S" => {
                            this.handle_modal_save(&pending.key, window, cx);
                            return;
                        }
                        _ => {}
                    }
                }

                let search_is_focused = this
                    .search_input
                    .as_ref()
                    .map(|si| si.read(cx).focus_handle.is_focused(window))
                    .unwrap_or(false);
                this.search_focused = search_is_focused;

                if search_is_focused {
                    let key = event.keystroke.key.as_str();
                    let mods = event.keystroke.modifiers;
                    if mods.platform && (key == "c" || key == "C") {
                        if !this.state.search_query.is_empty() {
                            cx.write_to_clipboard(ClipboardItem::new_string(this.state.search_query.clone()));
                        }
                        return;
                    }
                    if key == "enter" {
                        this.search_focused = false;
                        window.focus(&this.focus_handle);
                        cx.notify();
                        return;
                    }
                    return;
                }

                if this.is_renaming {
                    match event.keystroke.key.as_str() {
                        "enter" => {
                            let new_name = this.rename_buffer.trim().to_owned();
                            if !new_name.is_empty() {
                                this.rename_active_file(&new_name, cx);
                            }
                            this.is_renaming = false;
                            cx.notify();
                        }
                        "escape" => {
                            this.is_renaming = false;
                            cx.notify();
                        }
                        "backspace" => {
                            this.rename_buffer.pop();
                            cx.notify();
                        }
                        "space" => {
                            this.rename_buffer.push(' ');
                            cx.notify();
                        }
                        "period" => {
                            this.rename_buffer.push('.');
                            cx.notify();
                        }
                        key => {
                            if let Some(ch) = &event.keystroke.key_char {
                                this.rename_buffer.push_str(ch);
                                cx.notify();
                            } else if key.chars().count() == 1
                                && !event.keystroke.modifiers.control
                                && !event.keystroke.modifiers.platform
                            {
                                this.rename_buffer.push_str(key);
                                cx.notify();
                            }
                        }
                    }
                    return;
                }

                let key = event.keystroke.key.as_str();
                let mods = event.keystroke.modifiers;
                if key.eq_ignore_ascii_case("tab") && this.preview_focused {
                    if this.preview_scroll_focus.is_focused(window) { window.focus(&this.focus_handle); }
                    else { window.focus(&this.preview_scroll_focus); }
                    return;
                }
                if this.preview_scroll_focus.is_focused(window) && !mods.platform { return; }

                if mods.platform {
                    match key {
                        "z" => {
                            if mods.shift {
                                this.state.editor.redo();
                            } else {
                                this.state.editor.undo();
                            }
                            if this.state.sync_from_editor() {
                                this.schedule_parse(cx);
                            }
                            this.trigger_auto_save(cx);
                            cx.notify();
                        }
                        "s" => {
                            this.trigger_save(cx);
                        }
                        "w" => {
                            this.trigger_close_active(Some(window), cx);
                        }
                        "n" => {
                            this.trigger_draft(Some(window), cx);
                        }
                        "o" => {
                            let rx = cx.prompt_for_paths(PathPromptOptions {
                                files: true,
                                directories: false,
                                multiple: false,
                                prompt: None,
                            });
                            cx.spawn(async move |this, app| {
                                if let Ok(Ok(Some(mut paths))) = rx.await {
                                    if let Some(path) = paths.pop() {
                                        let _ = this.update(app, |this, cx| {
                                            let _ = this.state.load_file(path);
                                            this.on_document_swapped(None, cx);
                                            cx.notify();
                                        });
                                    }
                                }
                            })
                            .detach();
                        }
                        "," => {
                            this.open_settings_window(cx);
                        }
                        "b" => {
                            this.toggle_drawer(cx);
                        }
                        "1" => {
                            this.set_view_mode(ViewMode::Source, cx);
                        }
                        "2" => {
                            this.set_view_mode(ViewMode::Split, cx);
                        }
                        "f" | "F" => {
                            if !this.state.drawer_open {
                                this.state.toggle_drawer();
                            }
                            this.search_focused = true;
                            if let Some(ref input) = this.search_input {
                                let handle = input.read(cx).focus_handle.clone();
                                window.focus(&handle);
                            }
                            cx.notify();
                        }
                        "c" | "C" => {
                            let should_copy_preview = match this.state.view_mode {
                                ViewMode::Preview => true,
                                ViewMode::Split => this.preview_focused,
                                ViewMode::Source => false,
                            };
                            if should_copy_preview {
                                if let Some(sel) = this.preview_selection {
                                    let text = crate::views::preview::serialize_preview_selection(&this.state.doc().parsed, sel);
                                    if !text.is_empty() {
                                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                                    }
                                }
                            }
                        }
                        "3" => {
                            this.set_view_mode(ViewMode::Preview, cx);
                        }
                        _ => {}
                    }
                    return;
                }

                let mut handled = true;
                match key {
                    "backspace" => this.state.editor.delete_backward(),
                    "delete" => this.state.editor.delete_forward(),
                    "enter" => this.state.editor.insert_newline(),
                    "tab" => {
                        let spaces = " ".repeat(this.state.preferences.tab_size);
                        this.state.editor.insert_text(&spaces);
                    }
                    "left" => this.state.editor.move_left(),
                    "right" => this.state.editor.move_right(),
                    "up" => {
                        let visual_lines = this.compute_visual_lines(window);
                        this.state.editor.move_up_visual_lines(&visual_lines);
                    }
                    "down" => {
                        let visual_lines = this.compute_visual_lines(window);
                        this.state.editor.move_down_visual_lines(&visual_lines);
                    }
                    "home" => this.state.editor.move_to_line_start(),
                    "end" => this.state.editor.move_to_line_end(),
                    _ => {
                        if let Some(key_char) = &event.keystroke.key_char {
                            if !mods.control {
                                this.state.editor.insert_text(key_char);
                            } else {
                                handled = false;
                            }
                        } else if key.chars().count() == 1 && !mods.control {
                            this.state.editor.insert_text(key);
                        } else {
                            handled = false;
                        }
                    }
                }

                if handled {
                    this.preview_focused = false;
                    if this.state.sync_from_editor() {
                        this.schedule_parse(cx);
                    }
                    this.trigger_auto_save(cx);
                    cx.notify();
                }
            }))
            .size_full()
            .flex()
            .flex_row()
            .bg(theme.bg)
            .text_color(theme.text_primary)
            // 1. Left Drawer (Files & TOC) with Bidirectional 180ms Animation
            .when(show_drawer, |body| {
                body.child(
                    div()
                        .id(if is_drawer_closing { "animated_drawer_closing" } else { "animated_drawer_opening" })
                        .h_full()
                        .overflow_hidden()
                        .child(
                            div()
                                .w(px(crate::views::editor::DRAWER_WIDTH))
                                .h_full()
                                .border_r_1()
                                .border_color(theme.border)
                                .bg(theme.bg)
                                .overflow_hidden()
                                .child({
                                    let view_search = cx.entity().clone();
                                    let si_clear = self.search_input.clone();
                                    let on_clear_search = {
                                        let view = view_search.clone();
                                        Arc::new(move |_window: &mut Window, cx: &mut App| {
                                            if let Some(ref input) = si_clear {
                                                input.update(cx, |inp, cx| {
                                                    inp.clear(cx);
                                                });
                                            }
                                            view.update(cx, |this, cx| {
                                                this.state.clear_search();
                                                cx.notify();
                                            });
                                        })
                                    };
                                    let si_focus = self.search_input.clone();
                                    let on_focus_search = {
                                        let view = view_search;
                                        Arc::new(move |window: &mut Window, cx: &mut App| {
                                            if let Some(ref input) = si_focus {
                                                let handle = input.read(cx).focus_handle.clone();
                                                window.focus(&handle);
                                            }
                                            view.update(cx, |this, cx| {
                                                this.search_focused = true;
                                                cx.notify();
                                            });
                                        })
                                    };
                                    let search_params = crate::views::drawer::DrawerSearchParams {
                                        search_query: self.state.search_query.clone(),
                                        has_search_matches: self.state.has_search_matches(),
                                        search_in_progress: self.state.search_in_progress,
                                        search_truncated: self.state.search_truncated,
                                        on_clear_search: Some(on_clear_search),
                                        on_focus_search: Some(on_focus_search),
                                        search_input: self.search_input.clone(),
                                    };
                                    crate::views::drawer::render_drawer_with_search(
                                        is_fullscreen,
                                        self.state.drawer_tab,
                                        &self.state.workspace_roots,
                                        &loose_docs,
                                        &self.state.open_docs,
                                        self.state.active.as_ref(),
                                        &self.state.doc().parsed.headings(),
                                        theme,
                                        strings,
                                        &self.drawer_scroll_handle,
                                        &search_params,
                                        on_select_tab,
                                        on_toggle_root,
                                        on_remove_root,
                                        on_refresh_root,
                                        on_toggle_folder,
                                        on_select_file,
                                        on_select_doc,
                                        on_close_doc,
                                        on_new_draft,
                                        on_select_heading,
                                        on_open_folder,
                                        on_toggle_drawer,
                                        on_new_file_in_dir,
                                    )
                                }),
                        )
                        .with_animation(
                            if is_drawer_closing { "drawer_slide_out" } else { "drawer_slide_in" },
                            Animation::new(std::time::Duration::from_millis(180)).with_easing(ease_in_out),
                            move |el, delta| {
                                let factor = if is_drawer_closing { 1.0 - delta } else { delta };
                                el.w(px(crate::views::editor::DRAWER_WIDTH * factor)).opacity(factor.clamp(0.0, 1.0))
                            },
                        ),
                )
            })
            // 2. Center Content Canvas (Seamless edge-to-edge surface with integrated 38px header)
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .min_h(px(0.))
                    .h_full()
                    .bg(theme.surface)
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(self.render_canvas_header(window, theme, is_fullscreen, cx))
                    // Error Feedback Banner
                    .when_some(self.error_banner.clone(), |el, err_msg| {
                        el.child(
                            div()
                                .id("workspace_error_banner")
                                .h(px(36.))
                                .px_4()
                                .bg(rgba(0xef44441a))
                                .border_b_1()
                                .border_color(rgba(0xef444433))
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            svg()
                                                .path("icons/info.svg")
                                                .size(px(14.))
                                                .text_color(rgb(0xef4444)),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(rgb(0xef4444))
                                                .child(err_msg),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("btn_close_error_banner")
                                        .px_2()
                                        .py_0p5()
                                        .rounded_sm()
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(rgb(0xef4444))
                                        .hover(|s| s.bg(rgba(0xef444422)))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _window, cx| {
                                                this.error_banner = None;
                                                this.state.clear_error();
                                                cx.notify();
                                            }),
                                        )
                                        .child("×"),
                                ),
                        )
                    })
                    // Conflict Warning Banner
                    .when(has_conflict, |el| {
                        el.child(
                            div()
                                .h(px(36.))
                                .px_4()
                                .bg(theme.conflict_bg)
                                .border_b_1()
                                .border_color(theme.conflict_border)
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(0xfef08a))
                                        .child(strings.conflict_warning),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap_2()
                                        .child(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(theme.surface)
                                                .text_xs()
                                                .cursor_pointer()
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(|this, _, _window, cx| {
                                                        this.state.doc_mut().resolve_conflict_keep_local();
                                                        cx.notify();
                                                    }),
                                                )
                                                .child(strings.keep_local),
                                        )
                                        .child(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(theme.surface)
                                                .text_xs()
                                                .cursor_pointer()
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(|this, _, window, cx| {
                                                        if let Some(path) = &this.state.doc().path.clone() {
                                                            let _ = this.state.load_file(path.clone());
                                                            this.on_document_swapped(Some(window), cx);
                                                        }
                                                        cx.notify();
                                                    }),
                                                )
                                                .child(strings.reload_disk),
                                        ),
                                ),
                        )
                    })
                    // Large Document Parse Pause Banner
                    .when(parse_paused, |el| {
                        el.child(
                            div()
                                .h(px(36.))
                                .px_4()
                                .bg(theme.surface)
                                .border_b_1()
                                .border_color(theme.border)
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child(strings.parse_paused),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(theme.bg)
                                        .border_1()
                                        .border_color(theme.border)
                                        .text_xs()
                                        .cursor_pointer()
                                        .hover(|s| s.text_color(theme.accent))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _window, cx| {
                                                this.reset_syntax_attempts_and_refresh(cx);
                                                cx.notify();
                                            }),
                                        )
                                        .child(strings.refresh_preview),
                                ),
                        )
                    })
                            .child({
                                let copied_code_hash = self.copied_code_hash;
                                let on_copy = {
                                    let view_entity = cx.entity().clone();
                                    Arc::new(move |code_text: String, hash: u64, _window: &mut Window, app: &mut App| {
                                        app.write_to_clipboard(ClipboardItem::new_string(code_text));
                                        let view_entity = view_entity.clone();
                                        view_entity.update(app, |view, cx| {
                                            view.copied_code_hash = Some((hash, std::time::Instant::now()));
                                            cx.notify();
                                            cx.spawn(async move |this, app| {
                                                app.background_executor().timer(std::time::Duration::from_millis(1200)).await;
                                                let _ = this.update(app, |view, cx| {
                                                    if let Some((curr_h, _)) = view.copied_code_hash {
                                                        if curr_h == hash {
                                                            view.copied_code_hash = None;
                                                            cx.notify();
                                                        }
                                                    }
                                                });
                                            }).detach();
                                        });
                                    })
                                };

                                let view_for_sel = cx.entity().clone();
                                let on_selection_change: crate::views::preview::OnPreviewSelectionCallback = Arc::new(move |sel, is_selecting, window, app| {
                                    let view = view_for_sel.clone();
                                    view.update(app, |this, cx| {
                                        this.preview_selection = sel;
                                        this.is_preview_selecting = is_selecting;
                                        if is_selecting {
                                            this.preview_focused = true;
                                            window.focus(&this.focus_handle);
                                            this.preview_mouse_window_pos = Some(window.mouse_position());
                                            this.trigger_preview_autoscroll_if_needed(cx);
                                        } else {
                                            this.preview_autoscroll_task = None;
                                            this.preview_mouse_window_pos = None;
                                        }
                                        cx.notify();
                                    });
                                });

                                let view_for_scroll = cx.entity().clone();
                                let on_autoscroll: crate::views::preview::OnPreviewAutoscrollCallback = Arc::new(move |velocity, window, app| {
                                    let view = view_for_scroll.clone();
                                    view.update(app, |this, cx| {
                                        this.preview_mouse_window_pos = Some(window.mouse_position());
                                        if let (Some(mouse_pos), Some(bounds)) = (this.preview_mouse_window_pos, this.preview_content_bounds) {
                                            this.step_preview_autoscroll(velocity, mouse_pos, bounds, cx);
                                        }
                                        this.trigger_preview_autoscroll_if_needed(cx);
                                    });
                                });

                                let recovery_view = cx.entity().clone();
                                let recovery_doc = self.state.active.clone();
                                let recovery_revision = self.state.editor.revision;
                                let on_image_recovery: crate::views::preview::OnImageRecoveryCallback = Arc::new(move |range, url, candidate, _window, app| {
                                    let doc = recovery_doc.clone();
                                    let view = recovery_view.clone();
                                    view.update(app, |this, cx| {
                                        if this.state.active != doc || this.state.editor.revision != recovery_revision || this.state.doc().is_parse_stale() {
                                            this.error_banner = Some("文档已变化，请刷新预览后重试图片修复".into());
                                            cx.notify();
                                            return;
                                        }
                                        let document = this.state.doc().path.clone();
                                        cx.spawn(async move |view, app| {
                                            let check_url = url.clone();
                                            let check_candidate = candidate.clone();
                                            let valid = app.background_executor().spawn(async move {
                                                crate::local_images::validate_image_recovery(&check_url, document.as_deref(), &check_candidate)
                                            }).await;
                                            let _ = view.update(app, |this, cx| {
                                                if !valid || this.state.active != doc || this.state.editor.revision != recovery_revision || this.state.doc().is_parse_stale() {
                                                    this.error_banner = Some("图片候选或文档已变化，请刷新预览后重试".into());
                                                    cx.notify();
                                                    return;
                                                }
                                                let source = this.state.editor.to_text();
                                                let Some(target) = glidemark_core::ast::image_source_target_range(&source, range, &url) else {
                                                    this.error_banner = Some("无法安全定位这处图片路径，请手动修复".into());
                                                    cx.notify();
                                                    return;
                                                };
                                                let Some(replacement) = crate::local_images::recovered_image_reference(&candidate.relative, &url) else {
                                                    this.error_banner = Some("图片路径无法安全转换，请手动修复".into());
                                                    cx.notify();
                                                    return;
                                                };
                                                let expected = &source[target.clone()];
                                                if !this.state.editor.replace_source_range(target, expected, &replacement) {
                                                    this.error_banner = Some("图片路径无法撤销地修改（文档可能超过 8 MiB），请手动修复".into());
                                                    cx.notify();
                                                    return;
                                                }
                                                this.preview_selection = None;
                                                this.is_preview_selecting = false;
                                                this.preview_autoscroll_task = None;
                                                this.preview_mouse_window_pos = None;
                                                this.error_banner = None;
                                                this.state.sync_from_editor();
                                                this.schedule_parse(cx);
                                                this.trigger_auto_save(cx);
                                                cx.notify();
                                            });
                                        }).detach();
                                    });
                                });

                                let preview_callbacks = crate::views::preview::PreviewCallbacks {
                                    on_copy: Some(on_copy.clone()),
                                    on_open_lightbox: Some(on_open_lightbox.clone()),
                                    on_selection_change: Some(on_selection_change),
                                    on_autoscroll: Some(on_autoscroll),
                                    on_image_recovery: Some(on_image_recovery),
                                };

                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .min_h(px(0.))
                                    .size_full()
                                    .overflow_hidden()
                                    .child(if self.state.active.is_none() {
                                        div()
                                            .size_full()
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .justify_center()
                                            .gap_3()
                                            .text_color(theme.text_muted)
                                            .child(
                                                svg()
                                                    .path("icons/files.svg")
                                                    .size(px(40.))
                                                    .text_color(theme.border),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(14.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(strings.no_document_open),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .child(strings.no_document_open_desc),
                                            )
                                            .into_any_element()
                                    } else {
                                        let text_start = self.editor_text_start_offset(window);
                                        let gutter_width = text_start - crate::views::editor::EDITOR_CONTAINER_PADDING_X - crate::views::editor::EDITOR_ROW_PADDING_X - crate::views::editor::EDITOR_GUTTER_MARGIN_RIGHT;
                                        let editor_left_offset = crate::views::editor::calculate_editor_left_offset(self.state.drawer_open, text_start);
                                        match view_mode {
                                            ViewMode::Source => {
                                                let view_bounds = cx.entity().clone();
                                                div()
                                                    .size_full()
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _event: &MouseDownEvent, window, cx| {
                                                        window.focus(&this.focus_handle);
                                                        this.search_focused = false;
                                                        cx.notify();
                                                    }))
                                                    .child(
                                                        canvas(
                                                            move |bounds, _window, cx| {
                                                                view_bounds.update(cx, |this, _cx| {
                                                                    this.editor_content_bounds = Some(bounds);
                                                                    this.preview_content_bounds = None;
                                                                });
                                                            },
                                                            |_bounds, _, _window, _cx| {},
                                                        )
                                                        .absolute()
                                                        .size_full(),
                                                    )
                                                    .child(render_editor_pane(
                                                        self.state.editor.lines.clone(),
                                                        visual_lines.clone(),
                                                        self.syntax_cache.clone(),
                                                        self.source_code_blocks.clone(),
                                                        !self.state.doc().is_parse_stale(),
                                                        self.state.editor.cursor(),
                                                        theme,
                                                        strings,
                                                        self.state.preferences.font_size,
                                                        self.state.preferences.line_height,
                                                        is_dirty,
                                                        &self.editor_scroll_handle,
                                                        editor_left_offset,
                                                        gutter_width,
                                                        on_click_line.clone(),
                                                        &editor_failures,
                                                    ))
                                                    .into_any_element()
                                            }
                                            ViewMode::Preview => {
                                                let view_bounds = cx.entity().clone();
                                                div().size_full().flex()
                                                    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| this.handle_preview_scroll_wheel(event, window, cx)))
                                                    .child(div()
                                                    .flex_1().h_full().relative()
                                                    .min_w(px(0.))
                                                    .child(
                                                        canvas(
                                                            move |bounds, _window, cx| {
                                                                view_bounds.update(cx, |this, _cx| {
                                                                    this.preview_content_bounds = Some(bounds);
                                                                    this.editor_content_bounds = None;
                                                                });
                                                            },
                                                            |_bounds, _, _window, _cx| {},
                                                        )
                                                        .absolute()
                                                        .size_full(),
                                                    )
                                                    .child(crate::views::preview::render_preview_pane_with_callbacks(
                                                        self.state.doc().parsed.clone(),
                                                        theme,
                                                        strings,
                                                        self.state.preferences.max_column_width,
                                                        self.state.preferences.font_size,
                                                        &self.preview_list,
                                                        copied_code_hash,
                                                        preview_callbacks.clone(),
                                                        self.preview_selection,
                                                        self.is_preview_selecting,
                                                        self.rendered_regions.clone(),
                                                        self.syntax_cache.clone(),
                                                        &preview_failures,
                                                    self.local_images.clone(),
                                                    ))).child(self.render_preview_scrollbar(theme, cx))
                                                    .into_any_element()
                                            }
                                            ViewMode::Split => div()
                                                .size_full()
                                                .flex()
                                                .overflow_hidden()
                                                // Left Editor Pane
                                                .child({
                                                    let view_bounds = cx.entity().clone();
                                                    div()
                                                        .w(relative(self.split_ratio))
                                                        .min_w(px(0.))
                                                        .min_h(px(0.))
                                                        .h_full()
                                                        .flex()
                                                        .flex_col()
                                                        .overflow_hidden()
                                                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _event: &MouseDownEvent, window, cx| {
                                                            window.focus(&this.focus_handle);
                                                            this.search_focused = false;
                                                            cx.notify();
                                                        }))
                                                        .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                                                            this.handle_editor_scroll_wheel(event, window, cx);
                                                        }))
                                                        .child(
                                                            canvas(
                                                                move |bounds, _window, cx| {
                                                                    view_bounds.update(cx, |this, _cx| {
                                                                        this.editor_content_bounds = Some(bounds);
                                                                    });
                                                                },
                                                                |_bounds, _, _window, _cx| {},
                                                            )
                                                            .absolute()
                                                            .size_full(),
                                                        )
                                                        .child(render_editor_pane(
                                                            self.state.editor.lines.clone(),
                                                            visual_lines.clone(),
                                                            self.syntax_cache.clone(),
                                                            self.source_code_blocks.clone(),
                                                            !self.state.doc().is_parse_stale(),
                                                            self.state.editor.cursor(),
                                                            theme,
                                                            strings,
                                                            self.state.preferences.font_size,
                                                            self.state.preferences.line_height,
                                                            is_dirty,
                                                            &self.editor_scroll_handle,
                                                            editor_left_offset,
                                                            gutter_width,
                                                            on_click_line,
                                                            &editor_failures,
                                                        ))
                                                })
                                                // Draggable Resizable Divider: 5px hit zone with 1px hairline
                                                .child(
                                                    div()
                                                        .w(px(5.))
                                                        .h_full()
                                                        .flex()
                                                        .justify_center()
                                                        .items_center()
                                                        .cursor_col_resize()
                                                        .bg(theme.surface)
                                                        .hover(|s| s.bg(theme.accent))
                                                        .on_mouse_down(
                                                            MouseButton::Left,
                                                            cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                                                                if event.click_count == 2 {
                                                                    this.split_ratio = 0.5;
                                                                    cx.notify();
                                                                } else {
                                                                    this.is_resizing_split = true;
                                                                }
                                                            }),
                                                        )
                                                        .child(
                                                            div()
                                                                .w(px(1.))
                                                                .h_full()
                                                                .bg(theme.border),
                                                        ),
                                                )
                                                // Right Preview Pane
                                                .child(
                                                    div()
                                                        .w(relative(1.0 - self.split_ratio))
                                                        .min_w(px(0.))
                                                        .min_h(px(0.))
                                                        .h_full()
                                                        .flex()
                                                        .flex_col()
                                                        .overflow_hidden()
                                                        .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                                                            this.handle_preview_scroll_wheel(event, window, cx);
                                                        }))
                                                        .child({
                                                            let view_bounds = cx.entity().clone();
                                                            div().size_full().flex().child(div()
                                                                .flex_1().h_full().relative()
                                                                .min_w(px(0.))
                                                                .child(
                                                                    canvas(
                                                                        move |bounds, _window, cx| {
                                                                            view_bounds.update(cx, |this, _cx| {
                                                                                this.preview_content_bounds = Some(bounds);
                                                                            });
                                                                        },
                                                                        |_bounds, _, _window, _cx| {},
                                                                    )
                                                                    .absolute()
                                                                    .size_full(),
                                                                )
                                                                .child(crate::views::preview::render_preview_pane_with_callbacks(
                                                                    self.state.doc().parsed.clone(),
                                                                    theme,
                                                                    strings,
                                                                    self.state.preferences.max_column_width,
                                                                    self.state.preferences.font_size,
                                                                    &self.preview_list,
                                                                    copied_code_hash,
                                                                    preview_callbacks,
                                                                    self.preview_selection,
                                                                    self.is_preview_selecting,
                                                                    self.rendered_regions.clone(),
                                                                    self.syntax_cache.clone(),
                                                                    &preview_failures,
                                                                    self.local_images.clone(),
                                                                ))).child(self.render_preview_scrollbar(theme, cx))
                                                        }),
                                                )
                                                .into_any_element(),
                                        }
                                    })
                            })
                    )
                    .when(self.lightbox.is_open, |root| {
                        root.child(render_lightbox(&self.lightbox, theme, strings, cx))
                    })
                    .when(self.state.pending_remove_root.is_some(), |root| {
                        root.child(self.render_remove_root_modal(theme, cx))
                    })
                    .when(self.state.pending_close.is_some(), |root| {
                        root.child(self.render_close_modal(theme, cx))
                    })
    }
}

// Floating tooltip micro-capsule with dark frosted styling
#[derive(Clone)]
pub(crate) struct TooltipView {
    pub(crate) text: String,
}

impl Render for TooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgba(0x18181bf2))
            .border_1()
            .border_color(rgba(0x3f3f4640))
            .text_size(px(11.))
            .text_color(rgb(0xf4f4f5))
            .font_family(crate::platform::UI_FONT)
            .shadow_md()
            .child(self.text.clone())
    }
}

// Minimalist vector icon button helper with tooltips and hover capsule
fn render_icon_button<F>(
    id: impl Into<ElementId>,
    tooltip_text: impl Into<String>,
    active: bool,
    theme: &crate::theme::ThemeColors,
    icon: impl IntoElement + 'static,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let text = tooltip_text.into();
    let base = div()
        .id(id)
        .tooltip(move |_window, cx| {
            let t = text.clone();
            cx.new(|_| TooltipView { text: t }).into()
        })
        .w(px(28.))
        .h(px(28.))
        .rounded(px(6.))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, handler);

    if active {
        base.bg(theme.selection_bg())
            .child(icon)
    } else {
        base.hover(|s| s.bg(theme.hover_bg()))
            .child(icon)
    }
}

/// Resolves the SVG path for the lightbox, ensuring that the `is_dark` variant
/// matches the current theme active in the preview pane (ADR 0036).
pub fn resolve_lightbox_svg_path(
    svg_path: &std::path::Path,
    code_hash: u64,
    is_dark: bool,
) -> std::path::PathBuf {
    let expected_variant = if is_dark { "dark" } else { "light" };
    let filename = format!("{:016x}_{}.svg", code_hash, expected_variant);

    let preview_cache_dir = std::env::temp_dir().join("glidemark").join("mermaid");
    let preview_candidate = preview_cache_dir.join(&filename);
    if preview_candidate.exists() {
        return preview_candidate;
    }

    if let Some(parent) = svg_path.parent() {
        let parent_candidate = parent.join(&filename);
        if parent_candidate.exists() {
            return parent_candidate;
        }
    }

    svg_path.to_path_buf()
}

/// Adaptive header padding calculation for window chrome clearance (ADR 0032).
///
/// Under ADR 0032:
/// - On macOS:
///   - When the drawer is collapsed (`drawer_open == false`), the canvas header touches the left edge
///     and must clear traffic lights: 76px in normal window mode, 12px in fullscreen mode.
///   - When the drawer is open (`drawer_open == true`), the drawer header occupies the left edge,
///     so the canvas header needs only standard 8px padding.
/// - On non-macOS platforms:
///   - Always 8px regardless of fullscreen or drawer state.
pub fn adaptive_header_padding(is_macos: bool, is_fullscreen: bool, drawer_open: bool) -> f32 {
    if !is_macos {
        8.0
    } else if drawer_open {
        CANVAS_TITLE_LEFT_MARGIN - 8.0
    } else if is_fullscreen {
        12.0
    } else {
        crate::platform::LEADING_WINDOW_CONTROL_INSET
    }
}

/// Computes the exact title left padding according to Phase H specification:
/// - Collapsed drawer: maintains traffic light clearance (76px normal on macOS)
/// - Source / Split mode: aligns title with the measured editor text start
/// - Preview mode: aligns title with centered reading column
pub fn calculate_title_header_padding(
    is_macos: bool,
    is_fullscreen: bool,
    drawer_open: bool,
    view_mode: ViewMode,
    viewport_width: f32,
    max_column_width: f32,
    editor_text_start: f32,
) -> f32 {
    if !drawer_open {
        adaptive_header_padding(is_macos, is_fullscreen, false)
    } else {
        match view_mode {
            ViewMode::Source | ViewMode::Split => {
                editor_text_start - 8.0
            }
            ViewMode::Preview => {
                let drawer_w = crate::views::editor::DRAWER_WIDTH;
                let available_w = (viewport_width - drawer_w).max(0.0);
                let col_w = max_column_width;
                let left_margin = ((available_w - col_w) * 0.5).max(32.0);
                (left_margin - 8.0).max(8.0)
            }
        }
    }
}

/// Canvas document title left margin (in pixels, ADR 0036).
/// Consistently aligns with the 16px content margin across Source, Split, and Preview.
pub const CANVAS_TITLE_LEFT_MARGIN: f32 = 16.0;

/// Fraction (0..1) of the target preview block to scroll to, combining the progress
/// inside the source line with the position of a wrapped sub-line.
pub fn calculate_subline_scroll_fraction(inner_progress: f32, subline_idx: usize, total_sublines: usize) -> f32 {
    let total = total_sublines.max(1);
    let subline_fraction = (subline_idx as f32) / (total as f32);
    (inner_progress + subline_fraction).clamp(0.0, 1.0)
}

/// Resolves background and text colors for view mode switcher tabs.
///
/// Active tabs receive `theme.selection_bg()` and `theme.selection_text()`
/// strictly distinct from `theme.surface`.
/// Inactive tabs receive transparent background and `theme.text_muted`.
pub fn mode_tab_colors(active: bool, theme: &crate::theme::ThemeColors) -> (gpui::Hsla, gpui::Hsla) {
    if active {
        (theme.selection_bg(), theme.selection_text())
    } else {
        (gpui::transparent_black(), theme.text_muted)
    }
}

fn render_mode_icon_tab<F>(
    id: impl Into<ElementId>,
    tooltip_text: impl Into<String>,
    active: bool,
    theme: &crate::theme::ThemeColors,
    icon: impl IntoElement + 'static,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let text = tooltip_text.into();
    let (bg_color, text_color) = mode_tab_colors(active, theme);
    let base = div()
        .id(id)
        .tooltip(move |_window, cx| {
            let t = text.clone();
            cx.new(|_| TooltipView { text: t }).into()
        })
        .w(px(28.))
        .h(px(28.))
        .rounded(px(6.))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, handler);

    if active {
        base.bg(bg_color)
            .text_color(text_color)
            .child(icon)
    } else {
        base.text_color(text_color)
            .hover(|s| s.bg(theme.hover_bg()).text_color(theme.text_primary))
            .child(icon)
    }
}

// Standard Lucide SVG icons rendered natively by GPUI
fn render_sidebar_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/panel-left.svg").size(px(16.)).text_color(color)
}

fn render_source_mode_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/code.svg").size(px(16.)).text_color(color)
}

fn render_split_mode_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/columns-2.svg").size(px(16.)).text_color(color)
}

fn render_preview_mode_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/book-open.svg").size(px(16.)).text_color(color)
}

fn render_export_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/download.svg").size(px(16.)).text_color(color)
}

fn render_settings_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/settings.svg").size(px(16.)).text_color(color)
}
