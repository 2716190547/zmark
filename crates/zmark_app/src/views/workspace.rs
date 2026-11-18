use gpui::prelude::*;
use gpui::*;
use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::actions::{
    CancelRename, ExportHtml, OpenFileDialog, SaveFile, ToggleDrawer, TogglePreviewMode, ToggleSettings,
    ToggleSourceMode, ToggleSplitMode,
};
use crate::state::{parse_debounce, AppState, CloseDecision, ViewMode};
use crate::views::editor::render_editor_pane;
use crate::views::lightbox::{render_lightbox, LightboxState};
use crate::updater::{PreparedUpdate, UpdateConfig, UpdateStatus};
use crate::views::preview::PreviewScrollState;
use crate::views::settings::SettingsWindowView;

mod source;
mod find;
pub use source::{SourceCommand, SourceComposition, SourceDrag, SourceRow};

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

#[derive(Clone)]
struct EditorScrollbarHandle {
    inner: UniformListScrollHandle,
    dirty: Rc<Cell<bool>>,
    dragging: Rc<Cell<bool>>,
    shared_content_height: Rc<Cell<Option<f32>>>,
}

impl EditorScrollbarHandle {
    fn virtual_scroll_bounds(&self) -> (f32, f32, f32) {
        let base = &self.inner.0.borrow().base_handle;
        let viewport_h = f32::from(base.bounds().size.height).max(1.0);
        let real_max = f32::from(base.max_offset().y).max(0.0);
        let real_total = viewport_h + real_max;
        let virtual_total = match self.shared_content_height.get() {
            Some(shared) if real_max > 0.5 => shared.max(real_total),
            _ => real_total,
        };
        let virtual_max = (virtual_total - viewport_h).max(0.0);
        (real_max, virtual_total, virtual_max)
    }
}

impl gpui_component::scroll::ScrollbarHandle for EditorScrollbarHandle {
    fn viewport_bounds(&self) -> Bounds<Pixels> {
        self.inner.0.borrow().base_handle.bounds()
    }

    fn offset(&self) -> Point<Pixels> {
        let (real_max, _, virtual_max) = self.virtual_scroll_bounds();
        let base = &self.inner.0.borrow().base_handle;
        let real = base.offset();
        if real_max <= 0.5 || virtual_max <= 0.5 {
            return real;
        }
        let ratio = (-f32::from(real.y) / real_max).clamp(0.0, 1.0);
        point(real.x, px(-(ratio * virtual_max)))
    }

    fn set_offset(&self, offset: Point<Pixels>) {
        let (real_max, _, virtual_max) = self.virtual_scroll_bounds();
        let ratio = if virtual_max > 0.5 {
            (-f32::from(offset.y) / virtual_max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let real_y = (ratio * real_max).clamp(0.0, real_max);
        let current_x = self.inner.0.borrow().base_handle.offset().x;
        self.inner
            .0
            .borrow_mut()
            .base_handle
            .set_offset(point(current_x, px(-real_y)));
        self.dirty.set(true);
    }

    fn content_size(&self) -> Size<Pixels> {
        let base = &self.inner.0.borrow().base_handle;
        let bounds = base.bounds();
        let (_, virtual_total, _) = self.virtual_scroll_bounds();
        size(
            bounds.size.width + base.max_offset().x,
            px(virtual_total.max(f32::from(bounds.size.height))),
        )
    }

    fn start_drag(&self) {
        self.dragging.set(true);
    }

    fn end_drag(&self) {
        self.dragging.set(false);
    }
}

pub struct WorkspaceView {
    pub state: AppState,
    sidebar_selection: crate::views::sidebar_selection::SharedSelection,
    sidebar_busy: bool,
    find: find::FindState,
    sidebar_undo: Option<Vec<PathBuf>>,
    sidebar_undo_task: Option<Task<()>>,
    pub focus_handle: FocusHandle,
    pub editor_scroll_handle: UniformListScrollHandle,
    editor_scrollbar_dirty: Rc<Cell<bool>>,
    editor_scrollbar_dragging: Rc<Cell<bool>>,
    split_scrollbar_content_height: Rc<Cell<Option<f32>>>,
    sync_scroll_target: Option<(ScrollOrigin, f32)>,
    sync_scroll_task: Option<Task<()>>,
    pub preview_list: PreviewScrollState,
    preview_viewport_size: Option<Size<Pixels>>,
    pub preview_scroll_activation_observed: bool,
    pub preview_scroll_end_pending: bool,
    preview_scroll_pending_seek: Option<(usize, f32)>,
    preview_scroll_seek_generation: u64,
    last_preview_sync_offset: f32,
    pub drawer_scroll_handle: ScrollHandle,
    pub split_ratio: f32,
    pub is_resizing_split: bool,
    pub is_renaming: bool,
    pub rename_target: Option<crate::state::DocKey>,
    pub rename_in_sidebar: bool,
    pub selected_directory: Option<PathBuf>,
    pub folder_transitions: HashMap<PathBuf, gpui_base::motion::TransitionState<f32>>,
    /// Native input states are created when this view is first mounted in a window.
    pub rename_input: Option<Entity<crate::views::drawer::SearchInput>>,
    pub workspace_width: f32,
    auto_save_tasks: HashMap<crate::state::DocKey, (u64, Task<()>)>,
    next_auto_save_id: u64,
    pub parse_task: Option<Task<()>>,
    pub parse_paused: bool,
    pub drawer_closing: bool,
    drawer_close_task: Option<Task<()>>,
    pub settings_window: Option<WindowHandle<gpui_component::Root>>,
    pub copied_code_hash: Option<(u64, std::time::Instant)>,
    pub last_scroll_origin: Option<(ScrollOrigin, std::time::Instant)>,
    pub lightbox: LightboxState,
    pub window_handle: Option<AnyWindowHandle>,
    window_activation_subscription: Option<Subscription>,
    window_was_active: bool,
    pub error_banner: Option<String>,
    /// (display file name, absolute path) shown after a successful HTML export.
    pub export_notice: Option<(String, PathBuf)>,
    export_notice_generation: u64,
    export_notice_closing: bool,
    was_dirty: bool,
    dirty_pulse_generation: u64,
    export_press_generation: u64,
    update_press_generation: u64,
    settings_press_generation: u64,
    pub update_config: UpdateConfig,
    pub update_status: UpdateStatus,
    pub prepared_update: Option<PreparedUpdate>,
    update_checking: bool,
    pending_restart: bool,
    pub search_focused: bool,
    pub preview_focused: bool,
    pub search_input: Option<Entity<crate::views::drawer::SearchInput>>,
    pub preview_selection: Option<crate::views::preview::PreviewSelectionRange>,
    pub is_preview_selecting: bool,
    pub preview_autoscroll_task: Option<gpui::Task<()>>,
    /// Eased scroll started by an outline click; dropped on any manual scroll.
    outline_scroll_task: Option<gpui::Task<()>>,
    preview_autoscroll_accum: f32,
    pub preview_mouse_window_pos: Option<Point<Pixels>>,
    pub preview_content_bounds: Option<Bounds<Pixels>>,
    pub editor_content_bounds: Option<Bounds<Pixels>>,
    pub source_rows: Arc<std::sync::Mutex<Vec<SourceRow>>>,
    pub source_composition: Option<SourceComposition>,
    pub source_drag: Option<SourceDrag>,
    source_click_after_composition: Option<(MouseDownEvent, bool)>,
    pub source_autoscroll_task: Option<Task<()>>,
    source_focus_subscription: Option<Subscription>,
    pub document_layout_cache: crate::views::editor::DocumentLayoutCache,
    pub cached_visual_rev: usize,
    pub cached_visual_width_bits: u64,
    pub cached_visual_font_bits: u32,
    pub cached_visual_lines: Arc<Vec<crate::editor_engine::VisualLine>>,
    /// Sorted anchor map for the current AST. Rebuilding it per wheel event
    /// copied and sorted the whole anchor list on every scroll tick.
    sync_map_cache: Option<(
        std::sync::Weak<zmark_core::ast::ParsedDocument>,
        zmark_core::anchor::SyncScrollMap,
    )>,
    pub rendered_regions: Arc<crate::views::preview::RegionsRegistry>,
    pub local_images: crate::local_images::SharedImages,
    release_hidden_diagrams: bool,
    pub local_image_task: Option<gpui::Task<()>>,
    /// Crisp re-rasterization of the open SVG lightbox at its current zoom, with its pixel size.
    pub lightbox_sharp: Option<(Arc<gpui::RenderImage>, (u32, u32))>,
    pub lightbox_sharp_want: (u32, u32),
    pub lightbox_local_image: Option<Arc<crate::local_images::LocalImage>>,
    pub lightbox_sharp_task: Option<gpui::Task<()>>,
    pub syntax_active_task: Option<gpui::Task<()>>,
    pub syntax_pending_request: Option<SyntaxBatchRequest>,
    pub syntax_cancel_flag: Arc<AtomicBool>,
    pub syntax_request_counter: u64,
    pub syntax_cache: Arc<std::sync::Mutex<zmark_core::syntax::SyntaxCache>>,
    #[cfg(target_os = "macos")]
    source_syntax_session: Arc<crate::syntax::SourceSyntaxWorker>,
    #[cfg(not(target_os = "macos"))]
    source_syntax_session: Arc<std::sync::Mutex<crate::syntax::SourceSyntaxSession>>,
    pub syntax_attempts: std::collections::HashMap<zmark_core::syntax::CodeKey, (u8, u8)>,
    pub syntax_block_states:
        std::collections::HashMap<zmark_core::syntax::CodeKey, SyntaxBlockState>,
    pub syntax_defer_armed: bool,
    pub last_visible_syntax_keys: std::collections::HashSet<zmark_core::syntax::CodeKey>,
    pub source_code_blocks: Arc<Vec<zmark_core::ast::ParsedCodeBlock>>,
    pub source_code_blocks_rev: usize,
    pub syntax_warm_states: std::collections::HashMap<&'static str, WarmState>,
    pub syntax_warm_task_id: u64,
    pub syntax_warm_task: Option<gpui::Task<()>>,
    pub syntax_warm_pending: bool,
    pub syntax_invalid_positions:
        std::collections::HashSet<(zmark_core::syntax::CodeKey, usize)>,
    pub current_reading_pane: Option<PaneKind>,
    pub last_reading_pane: Option<PaneKind>,
    pub current_ordered_candidates: Vec<zmark_core::syntax::CodeKey>,
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
    pub fn localized_reason<'a>(
        &self,
        strings: &'a zmark_core::i18n::I18nStrings,
    ) -> Option<&'a str> {
        match self {
            SyntaxBlockState::Ready
            | SyntaxBlockState::UnsupportedLanguage
            | SyntaxBlockState::WaitingWarm => None,
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
    pub blocks: Vec<zmark_core::syntax::CodeKey>,
    pub ordered_candidates: Vec<zmark_core::syntax::CodeKey>,
    pub reading_pane: Option<PaneKind>,
}

impl WorkspaceView {
    pub fn new(state: AppState, cx: &mut Context<Self>) -> Self {
        Self::new_with_update_config(state, UpdateConfig::disabled(), cx)
    }

    pub fn new_with_update_config(
        state: AppState,
        update_config: UpdateConfig,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::theme::init_components(cx);
        let mut preview_list = PreviewScrollState::new();
        preview_list.reset(state.doc().parsed.blocks.len());
        let mut view = Self {
            state,
            focus_handle: cx.focus_handle(),
            preview_viewport_size: None,
            preview_scroll_activation_observed: false,
            preview_scroll_end_pending: false,
            preview_scroll_pending_seek: None,
            preview_scroll_seek_generation: 0,
            last_preview_sync_offset: 0.0,
            editor_scroll_handle: UniformListScrollHandle::new(),
            editor_scrollbar_dirty: Rc::new(Cell::new(false)),
            editor_scrollbar_dragging: Rc::new(Cell::new(false)),
            split_scrollbar_content_height: Rc::new(Cell::new(None)),
            sync_scroll_target: None,
            sync_scroll_task: None,
            preview_list,
            drawer_scroll_handle: ScrollHandle::new(),
            split_ratio: 0.5,
            is_resizing_split: false,
            is_renaming: false,
            rename_target: None,
            rename_in_sidebar: false,
            selected_directory: None,
            sidebar_selection: Default::default(),
            sidebar_busy: false,
            find: find::FindState::default(),
            sidebar_undo: None,
            sidebar_undo_task: None,
            folder_transitions: HashMap::new(),
            rename_input: None,
            workspace_width: 1200.0,
            auto_save_tasks: HashMap::new(),
            next_auto_save_id: 0,
            parse_task: None,
            parse_paused: false,
            drawer_closing: false,
            drawer_close_task: None,
            settings_window: None,
            copied_code_hash: None,
            last_scroll_origin: None,
            lightbox: LightboxState::new(),
            window_handle: None,
            window_activation_subscription: None,
            window_was_active: false,
            error_banner: None,
            export_notice: None,
            export_notice_generation: 0,
            export_notice_closing: false,
            was_dirty: false,
            dirty_pulse_generation: 0,
            export_press_generation: 0,
            update_press_generation: 0,
            settings_press_generation: 0,
            update_config,
            update_status: UpdateStatus::Idle,
            prepared_update: None,
            update_checking: false,
            pending_restart: false,
            search_focused: false,
            preview_focused: false,
            search_input: None,
            preview_selection: None,
            is_preview_selecting: false,
            preview_autoscroll_task: None,
            outline_scroll_task: None,
            preview_autoscroll_accum: 0.0,
            preview_mouse_window_pos: None,
            preview_content_bounds: None,
            editor_content_bounds: None,
            source_rows: Arc::new(std::sync::Mutex::new(Vec::new())),
            source_composition: None,
            source_drag: None,
            source_click_after_composition: None,
            source_autoscroll_task: None,
            source_focus_subscription: None,
            document_layout_cache: crate::views::editor::DocumentLayoutCache::new(),
            cached_visual_rev: usize::MAX,
            cached_visual_width_bits: 0,
            cached_visual_font_bits: 0,
            cached_visual_lines: Arc::new(Vec::new()),
            sync_map_cache: None,
            rendered_regions: Arc::new(std::sync::Mutex::new(Vec::new())),
            release_hidden_diagrams: false,
            local_images: Arc::new(std::sync::Mutex::new(
                crate::local_images::LocalImages::default(),
            )),
            local_image_task: None,
            lightbox_sharp: None,
            lightbox_sharp_want: (0, 0),
            lightbox_local_image: None,
            lightbox_sharp_task: None,
            syntax_active_task: None,
            syntax_pending_request: None,
            syntax_cancel_flag: Arc::new(AtomicBool::new(false)),
            syntax_request_counter: 0,
            syntax_cache: Arc::new(std::sync::Mutex::new(
                zmark_core::syntax::SyntaxCache::new(),
            )),
            source_syntax_session: Default::default(),
            syntax_attempts: std::collections::HashMap::new(),
            syntax_block_states: std::collections::HashMap::new(),
            syntax_defer_armed: false,
            last_visible_syntax_keys: std::collections::HashSet::new(),
            source_code_blocks: Arc::new(Vec::new()),
            source_code_blocks_rev: usize::MAX,
            syntax_warm_states: {
                let mut map = std::collections::HashMap::new();
                for &lang in &zmark_core::syntax::SUPPORTED_CANONICAL_LANGUAGES {
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
                if let crate::local_images::ImageState::Ready(image) = state {
                    images.retired.push(image);
                }
            }
            if let Some((_, _, image)) = images.loading.take() {
                images.retired.push(image);
            }
            if let Some(image) = view.lightbox_local_image.take() { images.retired.push(image); }
            let mut released = std::collections::HashSet::new();
            for image in images.retired.drain(..) {
                if !released.insert(Arc::as_ptr(&image)) { continue; }
                let path = image.path().to_path_buf();
                if let Some(handle) = view.window_handle {
                    let _ = handle.update(cx, |_, window, cx| image.release(window, cx));
                } else if let Some(render) = &image.render { cx.drop_image(render.clone(), None); }
                cx.remove_asset::<gpui::ImgResourceLoader>(&gpui::Resource::Path(Arc::from(path.as_path())));
            }
        })
        .detach();
        view.start_warm_coordination_task(cx);
        view.init_update_service(cx);

        view
    }

    fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_component::input::{InputEvent, InputState};
        if self.search_input.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx)
                .placeholder(self.state.strings.search_placeholder)
                .clean_on_escape()
                .default_value(self.state.search_query.clone()));
            cx.subscribe_in(&input, window, |this, input, event, window, cx| {
                match event {
                    InputEvent::Change => {
                        let query = input.read(cx).value();
                        this.update_search_query(&query, cx);
                    }
                    InputEvent::PressEnter { .. } => {
                        this.search_focused = false;
                        window.focus(&this.focus_handle, cx);
                        cx.notify();
                    }
                    InputEvent::Focus => { this.search_focused = true; cx.notify(); }
                    InputEvent::Blur => { this.search_focused = false; cx.notify(); }
                }
            }).detach();
            self.search_input = Some(input);
        }
        if self.rename_input.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx));
            cx.subscribe_in(&input, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) && this.is_renaming {
                    window.focus(&this.focus_handle, cx);
                    this.finish_rename(true, cx);
                }
            }).detach();
            self.rename_input = Some(input);
        }
    }

    /// Drops syntax bookkeeping for code blocks that no longer exist in the
    /// current AST. These maps are keyed by block content, so without pruning
    /// every edit inside a code block keeps the old payload alive until the
    /// document is switched.
    fn prune_syntax_state(&mut self) {
        let keys: std::collections::HashSet<_> = self
            .state
            .doc()
            .parsed
            .code_blocks
            .iter()
            .map(|block| block.key.clone())
            .collect();
        self.syntax_attempts.retain(|key, _| keys.contains(key));
        self.syntax_block_states.retain(|key, _| keys.contains(key));
        self.last_visible_syntax_keys.retain(|key| keys.contains(key));
        self.syntax_invalid_positions.retain(|(key, _)| keys.contains(key));
        self.current_ordered_candidates.retain(|key| keys.contains(key));
    }

    /// Swaps in a new document: the preview list drops cached heights and starts at the top.
    pub fn reset_preview_list(&mut self) {
        self.cancel_source_composition();
        self.end_source_drag();
        self.source_rows.lock().unwrap().clear();
        self.cancel_pending_preview_seek();
        self.end_preview_scroll_drag();
        self.cancel_synced_scroll();
        self.editor_scrollbar_dragging.set(false);
        self.split_scrollbar_content_height.set(None);
        self.preview_selection = None;
        self.is_preview_selecting = false;
        self.preview_focused = false;
        self.preview_autoscroll_task = None;
        self.preview_mouse_window_pos = None;
        self.preview_list
            .reset(self.state.doc().parsed.blocks.len());
        self.last_preview_sync_offset = 0.0;
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
                let result = app
                    .background_executor()
                    .spawn(async move { job.execute() })
                    .await;
                let _ = this.update(app, |view, cx| {
                    if view.state.apply_search_result(result) {
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        cx.notify();
    }

    pub fn process_local_images(&mut self, cx: &mut Context<Self>) {
        let next = {
            let mut images = self.local_images.lock().unwrap();
            images.evict_hidden();
            if !images.retired.is_empty() {
                cx.notify();
                return;
            }
            if self.local_image_task.is_some() || images.loading.is_some() {
                return;
            }
            images
                .wanted
                .iter()
                .find(|url| {
                    matches!(
                        images.entries.get(*url),
                        Some(crate::local_images::ImageState::Waiting)
                    )
                })
                .map(|url| (images.generation, url.clone(), images.document.clone()))
        };
        let Some((generation, url, document)) = next else {
            return;
        };
        self.local_image_task = Some(cx.spawn(async move |this, app| {
            let load_url = url.clone();
            let (mut result, candidates) = app.background_executor().spawn(async move {
                let result = crate::local_images::load_image(&load_url, document.as_deref());
                let candidates = if result.is_err() && crate::local_images::is_windows_absolute_image(&load_url) {
                    Some(crate::local_images::image_recovery_candidates(&load_url, document.as_deref()))
                } else { None };
                (result, candidates)
            }).await;
            let _ = this.update(app, |view, cx| {
                view.local_image_task = None;
                let changed = {
                    let mut images = view.local_images.lock().unwrap();
                    if images.generation != generation || !images.wanted.contains(&url) {
                        false
                    } else {
                        if result.as_ref().is_ok_and(|image| images.decoded_bytes() + image.decoded_bytes > crate::local_images::MAX_CACHE_BYTES) {
                            result = Err(format!("可见图片缓存超过 {}MiB，请减少同时显示的图片", crate::local_images::MAX_CACHE_BYTES / (1024 * 1024)));
                        }
                        match result {
                            Ok(image) if image.render.is_some() => images.complete(generation, url.clone(), Ok(image)),
                            Ok(image) => {
                                images.loading = Some((generation, url.clone(), image));
                                true
                            }
                            Err(error) => {
                                let changed = images.complete(generation, url.clone(), Err(error));
                                if changed {
                                    if let Some(candidates) = candidates {
                                        images.entries.insert(url.clone(), crate::local_images::ImageState::RecoveryCandidates(candidates));
                                    }
                                }
                                changed
                            }
                        }
                    }
                };
                if changed { view.preview_list.invalidate_layout(); cx.notify(); }
                view.process_local_images(cx);
            });
        }));
    }

    fn complete_local_image_decode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (generation, url, image) = {
            let mut images = self.local_images.lock().unwrap();
            let Some((generation, url, image)) = images.loading.clone() else { return; };
            if generation != images.generation || !images.wanted.contains(&url) {
                images.loading = None;
                images.retired.push(image);
                cx.notify();
                return;
            }
            (generation, url, image)
        };
        let resource = gpui::Resource::Path(Arc::from(image.path()));
        // Pending is not failure: GPUI caches the decode and notifies this view when ready.
        let Some(render) = window.use_asset::<gpui::ImgResourceLoader>(&resource, cx) else { return; };
        let result = match render {
            Ok(render) => {
                let size = render.size(0);
                if size.width.0 < 1 || size.height.0 < 1 || render.frame_count() != 1
                    || crate::local_images::check_dimensions(size.width.0 as u32, size.height.0 as u32).is_err()
                    || size.width.0 as u32 != image.width || size.height.0 as u32 != image.height
                    || render.as_bytes(0).is_none_or(|bytes| bytes.len() as u64 > crate::local_images::MAX_DECODED_BYTES)
                {
                    Err("图片实际解码尺寸或帧数超限".into())
                } else {
                    Ok(image.clone())
                }
            }
            Err(error) => Err(format!("图片解码失败：{error}")),
        };
        let mut images = self.local_images.lock().unwrap();
        images.loading = None;
        let changed = images.complete(generation, url.clone(), result);
        if !matches!(images.entries.get(&url), Some(crate::local_images::ImageState::Ready(i)) if Arc::ptr_eq(i, &image)) {
            images.retired.push(image);
        }
        if changed || !images.retired.is_empty() {
            self.preview_list.invalidate_layout();
            cx.notify();
        }
    }

    /// Re-measures the preview after an edit, keeping the current scroll position.
    pub fn invalidate_preview_list(&mut self) {
        // The size table is rebuilt from the new AST on the next frame; the
        // current pixel offset stays valid because rebuild keeps its anchor.
        self.preview_list.invalidate_layout();
        self.last_preview_sync_offset = self.preview_list.offset();
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
                    this.state.sync_from_editor();
                    if this.state.parse_now() {
                        this.prune_syntax_state();
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
        self.cancel_source_composition();
        self.end_source_drag();
        self.parse_task = None;
        self.parse_paused = false;
        // A rename box belongs to the document it was opened on.
        self.is_renaming = false;
        self.preview_selection = None;
        self.is_preview_selecting = false;
        self.search_focused = false;
        self.state.sync_from_editor();
        if self.state.parse_now() {
            self.prune_syntax_state();
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
        self.syntax_block_states
            .retain(|_, s| matches!(s, SyntaxBlockState::Ready));
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
            let languages = this
                .read_with(app, |this, _| {
                    this.state
                        .doc()
                        .parsed
                        .code_blocks
                        .iter()
                        .filter_map(|block| block.key.language)
                        .collect::<std::collections::HashSet<_>>()
                })
                .unwrap_or_default();
            for lang in languages {
                let should_run = this
                    .read_with(app, |this, _cx| {
                        if task_id != this.syntax_warm_task_id {
                            return false;
                        }
                        this.syntax_warm_states.get(lang).copied() != Some(WarmState::Ready)
                    })
                    .unwrap_or(false);

                if !should_run {
                    continue;
                }

                let ok = app
                    .background_executor()
                    .spawn(async move { zmark_core::syntax::validate_language_warm(lang) })
                    .await;

                let success = if ok {
                    true
                } else {
                    app.background_executor()
                        .spawn(async move { zmark_core::syntax::validate_language_warm(lang) })
                        .await
                };

                let new_state = if success {
                    WarmState::Ready
                } else {
                    WarmState::InitFailed
                };

                let cont = this
                    .update(app, |this, cx| {
                        if task_id == this.syntax_warm_task_id {
                            this.syntax_warm_states.insert(lang, new_state);
                            this.last_visible_syntax_keys.clear();
                            this.process_visible_syntax_defer(cx);
                            cx.notify();
                            true
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false);

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

    pub fn reevaluate_admission(&mut self, order: &[zmark_core::syntax::CodeKey]) {
        let mut cache = self.syntax_cache.lock().unwrap();
        let mut retained = std::collections::HashSet::new();
        for key in order {
            if let Some(spans) = cache.get(key) {
                if cache.insert(key.clone(), spans, &retained) {
                    retained.insert(key.clone());
                    self.syntax_block_states
                        .insert(key.clone(), SyntaxBlockState::Ready);
                } else {
                    self.syntax_block_states
                        .insert(key.clone(), SyntaxBlockState::ResourceConstrained);
                }
            }
        }
    }

    fn failure_for_occurrence(
        &self,
        key: &zmark_core::syntax::CodeKey,
        open_line: usize,
        is_valid: bool,
    ) -> Option<VisibleSyntaxFailure> {
        let state = if !is_valid
            || self
                .syntax_invalid_positions
                .contains(&(key.clone(), open_line))
        {
            SyntaxBlockState::MappingInvalid
        } else if key
            .language
            .is_some_and(|lang| self.syntax_warm_states.get(lang) == Some(&WarmState::InitFailed))
        {
            SyntaxBlockState::InitFailed
        } else {
            *self.syntax_block_states.get(key)?
        };
        state
            .localized_reason(self.state.strings)
            .map(|reason| VisibleSyntaxFailure {
                source_line: open_line + 1,
                reason,
            })
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
        let offset_y =
            (-f32::from(self.editor_scroll_handle.0.borrow().base_handle.offset().y)).max(0.0);
        let row_h = crate::views::editor::calculate_line_row_height(
            self.state.preferences.font_size,
            zmark_core::config::Preferences::sanitize_line_height(
                self.state.preferences.line_height,
            ),
        );
        let start_vis = (offset_y / row_h).floor().max(0.0) as usize;
        let visible_h = f32::from(editor_bounds.size.height);
        let visible_line_count = (visible_h / row_h).ceil().max(1.0) as usize;
        let end_vis = start_vis + visible_line_count + 1;

        let start_phys = self
            .cached_visual_lines
            .get(start_vis)
            .map(|vl| vl.physical_line_idx)
            .unwrap_or(0);
        let end_phys = self
            .cached_visual_lines
            .get(end_vis.min(self.cached_visual_lines.len().saturating_sub(1)))
            .map(|vl| vl.physical_line_idx)
            .unwrap_or(usize::MAX);

        for b in self.source_code_blocks.iter() {
            let b_start = b.fence_open_line;
            let b_end = b
                .fence_close_line
                .unwrap_or_else(|| b.lines.last().map(|l| l.source_line_idx).unwrap_or(b_start));
            if b_start <= end_phys && b_end >= start_phys && !b.key.payload.is_empty() {
                if let Some(failure) =
                    self.failure_for_occurrence(&b.key, b.fence_open_line, b.is_valid)
                {
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
                if !key.payload.is_empty()
                    && preview_bounds.intersects(&r.bounds)
                    && seen.insert((key.clone(), open_line))
                {
                    if let Some(block) = self
                        .state
                        .doc()
                        .parsed
                        .code_blocks
                        .iter()
                        .find(|b| b.key == *key && b.fence_open_line == open_line)
                    {
                        if let Some(failure) =
                            self.failure_for_occurrence(key, open_line, block.is_valid)
                        {
                            failures.push(failure);
                        }
                    }
                }
            }
        }
        failures
    }

    fn sync_scroll_map(&mut self) -> Option<&zmark_core::anchor::SyncScrollMap> {
        if self.state.doc().is_parse_stale() || self.state.doc().parsed.anchors.is_empty() {
            return None;
        }
        let parsed = self.state.doc().parsed.clone();
        let current = Arc::downgrade(&parsed);
        let cached = self.sync_map_cache.as_ref().is_some_and(|(weak, _)| {
            weak.strong_count() > 0 && std::sync::Weak::ptr_eq(weak, &current)
        });
        if !cached {
            self.sync_map_cache = Some((
                current,
                zmark_core::anchor::SyncScrollMap::new(parsed.anchors.clone()),
            ));
        }
        self.sync_map_cache.as_ref().map(|(_, map)| map)
    }

    /// Aligns preview virtual list position to current editor cursor block.
    pub fn align_preview_to_cursor(&mut self) {
        self.cancel_pending_preview_seek();
        self.end_preview_scroll_drag();
        self.cancel_synced_scroll();
        let cursor_line = self.state.editor.cursor().0 + 1;
        let (block_index, inner_progress) = match self.sync_scroll_map() {
            Some(map) => map.source_line_to_preview_progress(cursor_line),
            None => return,
        };
        self.preview_list.set_scroll_offset(
            self.preview_list
                .offset_for_block(block_index, inner_progress),
        );
        self.last_preview_sync_offset = self.preview_list.offset();
    }

    fn editor_text_start_offset(&self, window: &Window) -> f32 {
        let gutter = crate::views::editor::editor_gutter_width(
            window.text_system(),
            self.state.editor.lines.len(),
            self.state.preferences.font_size,
        );
        crate::views::editor::editor_text_start_offset(gutter)
    }

    pub fn compute_editor_max_width_px(&self, window: &Window) -> f32 {
        let drawer_w = if self.state.drawer_open {
            px(crate::views::editor::DRAWER_WIDTH)
        } else {
            px(0.)
        };
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
                block
                    .lines
                    .last()
                    .map(|l| l.source_line_idx)
                    .unwrap_or(start)
            });
            for i in start..=end.min(line_count.saturating_sub(1)) {
                mask[i] = true;
            }
        }
        mask
    }

    pub fn compute_visual_lines(
        &mut self,
        window: &Window,
    ) -> Arc<Vec<crate::editor_engine::VisualLine>> {
        let max_width_px = self.compute_editor_max_width_px(window);
        let font_size = self.state.preferences.font_size;
        let rev = self.state.editor.revision;
        let width_bits = max_width_px.round() as u64;
        let font_bits = font_size.to_bits();

        let ast_blocks_changed = !self.state.doc().is_parse_stale()
            && (self.source_code_blocks_rev != rev
                || !Arc::ptr_eq(&self.source_code_blocks, &self.state.doc().parsed.code_blocks));

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
                self.source_code_blocks = self.state.doc().parsed.code_blocks.clone();
                self.source_code_blocks_rev = rev;
                self.syntax_invalid_positions.clear();
                for b in self.state.doc().parsed.code_blocks.iter() {
                    if !b.is_valid {
                        self.syntax_invalid_positions
                            .insert((b.key.clone(), b.fence_open_line));
                    }
                }
            }
        }

        let display = self.source_display_lines();
        let fenced_lines = self.compute_fenced_lines(display.len());
        let lines = self.document_layout_cache.update(
            window.text_system(),
            &display,
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
            key: zmark_core::syntax::CodeKey,
            pane: PaneKind,
            dist_to_center: f32,
            doc_line: usize,
        }

        let mut candidates: Vec<CandidateItem> = Vec::new();

        // 1. Preview candidates from rendered preview regions that intersect preview content bounds
        if matches!(self.state.view_mode, ViewMode::Preview | ViewMode::Split) {
            if let Some(preview_bounds) = self.preview_content_bounds {
                let preview_center_y =
                    f32::from(preview_bounds.origin.y + preview_bounds.size.height * 0.5);
                let regions = self.rendered_regions.lock().unwrap();
                for r in regions.iter() {
                    if let Some(ref key) = r.content_key {
                        if !key.payload.is_empty() && preview_bounds.intersects(&r.bounds) {
                            let Some(block) =
                                self.state.doc().parsed.code_blocks.iter().find(|b| {
                                    Some(b.fence_open_line) == r.fence_open_line && b.key == *key
                                })
                            else {
                                continue;
                            };
                            let doc_line = block.fence_open_line;
                            if !block.is_valid
                                || self
                                    .syntax_invalid_positions
                                    .contains(&(key.clone(), doc_line))
                            {
                                continue;
                            }

                            let region_mid_y =
                                f32::from(r.bounds.origin.y + r.bounds.size.height * 0.5);
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
                    let offset_y =
                        (-f32::from(self.editor_scroll_handle.0.borrow().base_handle.offset().y))
                            .max(0.0);
                    let row_h = crate::views::editor::calculate_line_row_height(
                        self.state.preferences.font_size,
                        zmark_core::config::Preferences::sanitize_line_height(
                            self.state.preferences.line_height,
                        ),
                    );
                    let visible_h = f32::from(editor_bounds.size.height);
                    let viewport_center_y = offset_y + visible_h * 0.5;
                    let start_vis = (offset_y / row_h).floor().max(0.0) as usize;
                    let visible_line_count = (visible_h / row_h).ceil().max(1.0) as usize;
                    let end_vis = start_vis + visible_line_count + 1;

                    let start_phys = self
                        .cached_visual_lines
                        .get(start_vis)
                        .map(|vl| vl.physical_line_idx)
                        .unwrap_or(0);
                    let end_phys = self
                        .cached_visual_lines
                        .get(end_vis.min(self.cached_visual_lines.len().saturating_sub(1)))
                        .map(|vl| vl.physical_line_idx)
                        .unwrap_or(usize::MAX);

                    for b in self.source_code_blocks.iter() {
                        let b_start = b.fence_open_line;
                        let b_end = b.fence_close_line.unwrap_or_else(|| {
                            b.lines.last().map(|l| l.source_line_idx).unwrap_or(b_start)
                        });
                        if b_start <= end_phys && b_end >= start_phys && !b.key.payload.is_empty() {
                            if self
                                .syntax_invalid_positions
                                .contains(&(b.key.clone(), b.fence_open_line))
                            {
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
        let mut ordered_visible_keys: Vec<zmark_core::syntax::CodeKey> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for c in candidates {
            if seen.insert(c.key.clone()) {
                ordered_visible_keys.push(c.key);
            }
        }

        let order_changed = ordered_visible_keys != self.current_ordered_candidates
            || self.current_reading_pane != self.last_evaluated_reading_pane;
        let current_keys_set: std::collections::HashSet<_> =
            ordered_visible_keys.iter().cloned().collect();

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
                            self.syntax_block_states
                                .insert(key.clone(), SyntaxBlockState::WaitingWarm);
                            continue;
                        }
                        WarmState::InitFailed => {
                            self.syntax_block_states
                                .insert(key.clone(), SyntaxBlockState::InitFailed);
                            continue;
                        }
                        WarmState::Ready => {}
                    }
                }
            }

            if cache.get(key).is_none() {
                if key.language.is_none() || key.payload.is_empty() {
                    let spans = Arc::from([zmark_core::syntax::SyntaxSpan {
                        range: 0..key.payload.len(),
                        role: zmark_core::syntax::SyntaxRole::Plain,
                    }]);
                    let empty_retained = std::collections::HashSet::new();
                    cache.insert(key.clone(), spans, &empty_retained);
                    continue;
                }

                let (gen, count) = self.syntax_attempts.entry(key.clone()).or_insert((0, 0));
                if *count < 2 {
                    *count += 1;
                    missing_blocks.push(key.clone());
                } else if *gen == 0 && zmark_core::syntax::is_syntax_set_ready() {
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
        let source_blocks = if self.state.doc().path.as_ref().is_some_and(|path| !zmark_core::file_ops::is_markdown(path)) {
            Some(self.source_code_blocks.clone())
        } else { None };
        let source_session = self.source_syntax_session.clone();
        let blocks = request.blocks.clone();

        self.syntax_active_task = Some(cx.spawn(async move |this, app| {
            let parsed_results = app
                .background_executor()
                .spawn(async move {
                    let mut results: Vec<(
                        zmark_core::syntax::CodeKey,
                        zmark_core::syntax::ParseResult,
                    )> = Vec::with_capacity(blocks.len());
                    if let Some(source_blocks) = source_blocks {
                        #[cfg(target_os = "macos")]
                        return source_session.parse_visible(source_blocks, blocks, cancel);
                        #[cfg(not(target_os = "macos"))]
                        return source_session.lock().unwrap().parse_visible(&source_blocks, &blocks, &cancel);
                    }
                    for key in blocks {
                        if cancel.load(Ordering::Relaxed) {
                            break;
                        }
                        let res = zmark_core::syntax::parse_code_spans_bounded(&key.payload, key.language, Some(&cancel));
                        results.push((key, res));
                    }
                    results
                })
                .await;

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
                            zmark_core::syntax::ParseResult::Ready(spans) => {
                                // Admission below uses only the current ordered candidates;
                                // results outside that snapshot are dropped with this map.
                                new_ready.insert(key, spans);
                            }
                            zmark_core::syntax::ParseResult::BudgetExceeded => {
                                let (_gen, count) =
                                    this.syntax_attempts.entry(key.clone()).or_insert((0, 1));
                                if *count < 2 {
                                    *count += 1;
                                    retry_blocks.push(key);
                                } else {
                                    this.syntax_block_states
                                        .insert(key, SyntaxBlockState::BudgetExceeded);
                                }
                            }
                            zmark_core::syntax::ParseResult::Cancelled => {}
                            zmark_core::syntax::ParseResult::HardLimit => {
                                this.syntax_block_states
                                    .insert(key, SyntaxBlockState::HardLimit);
                            }
                            zmark_core::syntax::ParseResult::UnsupportedLanguage => {
                                this.syntax_block_states
                                    .insert(key, SyntaxBlockState::UnsupportedLanguage);
                            }
                            zmark_core::syntax::ParseResult::InitFailed => {
                                this.syntax_block_states
                                    .insert(key, SyntaxBlockState::InitFailed);
                            }
                        }
                    }

                    let snapshot_matches = request.ordered_candidates
                        == this.current_ordered_candidates
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
                                this.syntax_block_states
                                    .insert(candidate.clone(), SyntaxBlockState::Ready);
                            } else {
                                this.syntax_block_states.insert(
                                    candidate.clone(),
                                    SyntaxBlockState::ResourceConstrained,
                                );
                            }
                        } else if let Some(spans) = cache.get(candidate) {
                            if cache.insert(candidate.clone(), spans, &retained) {
                                retained.insert(candidate.clone());
                                this.syntax_block_states
                                    .insert(candidate.clone(), SyntaxBlockState::Ready);
                            } else {
                                this.syntax_block_states.insert(
                                    candidate.clone(),
                                    SyntaxBlockState::ResourceConstrained,
                                );
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
    ///
    /// The wheel event is projected to where GPUI's native scroll is about to put
    /// the editor, instead of reading the pre-event offset. Events that cannot
    /// move the editor (already at the top/bottom) no longer drag the preview.
    pub fn handle_editor_scroll_wheel(
        &mut self,
        event: &ScrollEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_pending_preview_seek();
        self.end_preview_scroll_drag();
        if self.state.view_mode != ViewMode::Split
            || self.state.doc().is_parse_stale()
            || self.state.doc().parsed.anchors.is_empty()
        {
            return;
        }
        let now = std::time::Instant::now();
        if let Some((origin, time)) = self.last_scroll_origin {
            if origin == ScrollOrigin::Preview
                && now.duration_since(time) < std::time::Duration::from_millis(100)
            {
                return;
            }
        }
        let wheel_dy = f32::from(event.delta.pixel_delta(window.line_height()).y);
        if wheel_dy == 0.0 {
            return;
        }
        let (current_y, max_y, _) = self.editor_scroll_metrics();
        // GPUI applies `scroll_offset.y += delta_y`; content offset is its negation.
        let projected_y = projected_scroll_offset(current_y, wheel_dy, max_y);
        if (projected_y - current_y).abs() <= 0.5 {
            // The editor cannot move in this direction; leave the preview alone.
            return;
        }
        self.last_scroll_origin = Some((ScrollOrigin::Editor, now));

        let target = self.preview_offset_for_editor_y(projected_y, max_y, window);
        // A down-gesture must never move the preview up (and vice versa), even
        // when estimated block heights make the mapped target overshoot.
        let current_preview = self.preview_list.offset();
        let target = if wheel_dy < 0.0 {
            target.max(current_preview)
        } else {
            target.min(current_preview)
        };
        self.drive_synced_scroll(ScrollOrigin::Preview, target, false, window, cx);
    }

    fn preview_offset_for_vis_index(
        &mut self,
        visual_lines: &[crate::editor_engine::VisualLine],
        vis_idx: usize,
    ) -> f32 {
        let vl_opt = visual_lines.get(vis_idx);
        let phys_line_idx = vl_opt.map(|vl| vl.physical_line_idx).unwrap_or(0);
        let phys_line = phys_line_idx + 1;
        let subline_idx = vl_opt.map(|vl| vl.subline_idx).unwrap_or(0);
        let total_sublines = visual_lines
            .iter()
            .filter(|vl| vl.physical_line_idx == phys_line_idx)
            .count()
            .max(1);
        let (block_index, inner_progress) = match self.sync_scroll_map() {
            Some(map) => map.source_line_to_preview_progress(phys_line),
            None => (0, 0.0),
        };
        let fraction =
            calculate_subline_scroll_fraction(inner_progress, subline_idx, total_sublines);
        self.preview_list.offset_for_block(block_index, fraction)
    }

    fn preview_offset_for_editor_y(&mut self, y: f32, max_y: f32, window: &mut Window) -> f32 {
        if y <= 0.5 {
            return 0.0;
        }
        if y >= max_y - 0.5 {
            return self.preview_list.max_scroll();
        }
        let visual_lines = self.compute_visual_lines(window);
        if visual_lines.is_empty() {
            return 0.0;
        }
        let row_h = crate::views::editor::calculate_line_row_height(
            self.state.preferences.font_size,
            self.state.preferences.line_height,
        )
        .max(1.0);
        let exact_vis = (y / row_h).max(0.0);
        let v0 = (exact_vis.floor() as usize).min(visual_lines.len() - 1);
        let v1 = (v0 + 1).min(visual_lines.len() - 1);
        let frac = (exact_vis - v0 as f32).clamp(0.0, 1.0);
        let p0 = self.preview_offset_for_vis_index(&visual_lines, v0);
        if v1 == v0 || frac <= f32::EPSILON {
            return p0;
        }
        let p1 = self.preview_offset_for_vis_index(&visual_lines, v1);
        p0 + (p1 - p0) * frac
    }

    /// Synchronizes preview wheel scrolling to the editor pane with 100ms echo suppression.
    pub fn handle_preview_scroll_wheel(
        &mut self,
        event: &ScrollEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_pending_preview_seek();
        self.end_preview_scroll_drag();
        if self.state.view_mode != ViewMode::Split
            || self.state.doc().is_parse_stale()
            || self.state.doc().parsed.anchors.is_empty()
        {
            return;
        }
        let now = std::time::Instant::now();
        if let Some((origin, time)) = self.last_scroll_origin {
            if origin == ScrollOrigin::Editor
                && now.duration_since(time) < std::time::Duration::from_millis(100)
            {
                return;
            }
        }
        let wheel_dy = f32::from(event.delta.pixel_delta(window.line_height()).y);
        if wheel_dy == 0.0 {
            return;
        }
        let current = self.preview_list.offset();
        let max = self.preview_list.max_scroll();
        let projected = projected_scroll_offset(current, wheel_dy, max);
        if (projected - current).abs() <= 0.5 {
            // Preview already at its limit: do not scroll the editor either.
            return;
        }
        self.last_scroll_origin = Some((ScrollOrigin::Preview, now));
        self.scroll_editor_to_preview_offset(projected, Some(wheel_dy < 0.0), false, window, cx);
        self.last_preview_sync_offset = projected;
        cx.notify();
    }

    fn sync_preview_scroll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_preview_scroll_with_mode(false, window, cx);
    }

    fn sync_preview_scroll_with_mode(
        &mut self,
        immediate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.view_mode != ViewMode::Split
            || self.state.doc().is_parse_stale()
            || self.state.doc().parsed.anchors.is_empty()
        {
            return;
        }
        let now = std::time::Instant::now();
        if let Some((origin, time)) = self.last_scroll_origin {
            if origin == ScrollOrigin::Editor
                && now.duration_since(time) < std::time::Duration::from_millis(100)
            {
                return;
            }
        }
        self.last_scroll_origin = Some((ScrollOrigin::Preview, now));
        let offset = self.preview_list.offset();
        self.scroll_editor_to_preview_offset(offset, None, immediate, window, cx);
        cx.notify();
    }

    /// Aligns the editor top to a preview content offset. `direction` is
    /// `Some(true)` for a down-gesture, `Some(false)` for up, `None` for a
    /// programmatic alignment that may move either way.
    fn scroll_editor_to_preview_offset(
        &mut self,
        offset: f32,
        direction: Option<bool>,
        immediate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let max = self.preview_list.max_scroll();
        let visual_lines = self.compute_visual_lines(window);
        if visual_lines.is_empty() {
            return;
        }
        let max_vis = visual_lines.len() - 1;
        let (target_vis_idx, exact_vis) = if offset <= 0.5 || max <= 0.5 {
            (0, 0.0)
        } else if offset >= max - 0.5 {
            (max_vis, max_vis as f32)
        } else {
            let (block_ix, progress) = self.preview_list.block_progress_at_offset(offset);
            let exact_line = match self.sync_scroll_map() {
                Some(map) => map.preview_to_source_line_fraction(block_ix, progress),
                None => 1.0,
            };
            let phys_pos = (exact_line - 1.0).max(0.0);
            let phys0 = phys_pos.floor() as usize;
            let phys_frac = (phys_pos - phys0 as f32).clamp(0.0, 1.0);
            let first_vis = visual_lines
                .iter()
                .position(|vl| vl.physical_line_idx == phys0)
                .unwrap_or(max_vis);
            let next_vis = visual_lines
                .iter()
                .position(|vl| vl.physical_line_idx > phys0)
                .unwrap_or(visual_lines.len());
            let exact_vis =
                first_vis as f32 + (next_vis.saturating_sub(first_vis)) as f32 * phys_frac;
            (first_vis, exact_vis)
        };
        let (current_y, editor_max, _) = self.editor_scroll_metrics();
        let row_h = crate::views::editor::calculate_line_row_height(
            self.state.preferences.font_size,
            self.state.preferences.line_height,
        )
        .max(1.0);
        if editor_max <= 0.5 {
            let current_vis = (current_y / row_h).floor() as usize;
            let target_vis_idx = match direction {
                Some(true) => target_vis_idx.max(current_vis),
                Some(false) => target_vis_idx.min(current_vis),
                None => target_vis_idx,
            };
            self.editor_scroll_handle
                .scroll_to_item_strict(target_vis_idx, ScrollStrategy::Top);
            return;
        }
        let mut target_y = if offset >= max - 0.5 && max > 0.5 {
            editor_max
        } else {
            (exact_vis * row_h).clamp(0.0, editor_max)
        };
        if let Some(forward) = direction {
            target_y = if forward {
                target_y.max(current_y)
            } else {
                target_y.min(current_y)
            };
        }
        self.drive_synced_scroll(ScrollOrigin::Editor, target_y, immediate, window, cx);
    }

    fn cancel_synced_scroll(&mut self) {
        self.sync_scroll_target = None;
        self.sync_scroll_task = None;
    }

    /// Returns the active smooth-sync scroll target pane and offset, if any.
    pub fn sync_scroll_target(&self) -> Option<(ScrollOrigin, f32)> {
        self.sync_scroll_target
    }

    fn apply_synced_target(&mut self, target_pane: ScrollOrigin, offset: f32) {
        match target_pane {
            ScrollOrigin::Preview => {
                self.last_scroll_origin = Some((ScrollOrigin::Editor, std::time::Instant::now()));
                self.preview_list.set_scroll_offset(offset);
                self.last_preview_sync_offset = self.preview_list.offset();
            }
            ScrollOrigin::Editor => {
                self.last_scroll_origin = Some((ScrollOrigin::Preview, std::time::Instant::now()));
                let handle = self.editor_scroll_handle.0.borrow().base_handle.clone();
                let max_y = f32::from(handle.max_offset().y).max(0.0);
                let clamped = offset.clamp(0.0, max_y);
                handle.set_offset(point(handle.offset().x, px(-clamped)));
            }
        }
    }

    fn drive_synced_scroll(
        &mut self,
        target_pane: ScrollOrigin,
        target: f32,
        immediate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.preferences.reduce_motion || immediate {
            self.cancel_synced_scroll();
            self.apply_synced_target(target_pane, target);
            cx.notify();
            return;
        }
        let (current, max_scroll) = match target_pane {
            ScrollOrigin::Preview => (self.preview_list.offset(), self.preview_list.max_scroll()),
            ScrollOrigin::Editor => {
                let (cur, max, _) = self.editor_scroll_metrics();
                (cur, max)
            }
        };
        let target = target.clamp(0.0, max_scroll);
        if (target - current).abs() <= 1.0 {
            self.cancel_synced_scroll();
            self.apply_synced_target(target_pane, target);
            cx.notify();
            return;
        }
        let first_step = current + (target - current) * 0.35;
        self.apply_synced_target(target_pane, first_step);

        let keep_task = self
            .sync_scroll_target
            .is_some_and(|(pane, _)| pane == target_pane)
            && self.sync_scroll_task.is_some();
        self.sync_scroll_target = Some((target_pane, target));
        if !keep_task {
            let view = cx.entity().downgrade();
            let handle = window.window_handle();
            self.sync_scroll_task = Some(cx.spawn(async move |_this, app| loop {
                app.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                let keep_going = handle
                    .update(app, |_, _window, app| {
                        view.update(app, |this, cx| this.step_synced_scroll(cx))
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if !keep_going {
                    break;
                }
            }));
        }
        cx.notify();
    }

    fn step_synced_scroll(&mut self, cx: &mut Context<Self>) -> bool {
        let Some((target_pane, raw_target)) = self.sync_scroll_target else {
            self.sync_scroll_task = None;
            return false;
        };
        if self.state.view_mode != ViewMode::Split {
            self.cancel_synced_scroll();
            return false;
        }
        let (current, max_scroll) = match target_pane {
            ScrollOrigin::Preview => (self.preview_list.offset(), self.preview_list.max_scroll()),
            ScrollOrigin::Editor => {
                let (cur, max, _) = self.editor_scroll_metrics();
                (cur, max)
            }
        };
        let target = raw_target.clamp(0.0, max_scroll);
        if (target - current).abs() <= 0.75 {
            self.apply_synced_target(target_pane, target);
            self.cancel_synced_scroll();
            cx.notify();
            return false;
        }
        let next = current + (target - current) * 0.28;
        self.apply_synced_target(target_pane, next);
        cx.notify();
        true
    }

    fn end_preview_scroll_drag(&mut self) {
        self.preview_list.cancel_scrollbar_drag();
    }

    fn cancel_pending_preview_seek(&mut self) {
        self.outline_scroll_task = None;
        if !self.preview_list.is_scrollbar_dragging() {
            self.preview_list.cancel_scrollbar_drag();
        }
        if matches!(self.sync_scroll_target, Some((ScrollOrigin::Preview, _))) {
            self.cancel_synced_scroll();
        }
        self.preview_scroll_seek_generation = self.preview_scroll_seek_generation.wrapping_add(1);
        self.preview_scroll_end_pending = false;
        self.preview_scroll_pending_seek = None;
    }

    /// Eases the preview to `target` (content px). The split-mode editor follows
    /// each frame through `sync_preview_scroll`. Any manual scroll cancels it via
    /// `cancel_pending_preview_seek`; "Reduce motion" jumps straight there.
    pub fn animate_preview_scroll_to(&mut self, target: f32, window: &mut Window, cx: &mut Context<Self>) {
        let start = self.preview_list.offset();
        let ms = outline_scroll_duration_ms(target - start);
        if self.state.preferences.reduce_motion || ms == 0 {
            self.apply_outline_scroll(target, window, cx);
            return;
        }
        let began = std::time::Instant::now();
        let view = cx.entity().downgrade();
        let handle = window.window_handle();
        self.outline_scroll_task = Some(cx.spawn(async move |_this, app| loop {
            app.background_executor().timer(std::time::Duration::from_millis(16)).await;
            let t = (began.elapsed().as_millis() as f32 / ms as f32).min(1.0);
            let offset = start + (target - start) * outline_scroll_ease(t);
            let alive = handle
                .update(app, |_, window, app| {
                    view.update(app, |this, cx| {
                        this.apply_outline_scroll(offset, window, cx);
                        if t >= 1.0 {
                            this.outline_scroll_task = None;
                        }
                    })
                    .is_ok()
                })
                .unwrap_or(false);
            if !alive || t >= 1.0 {
                break;
            }
        }));
    }

    fn apply_outline_scroll(&mut self, offset: f32, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_list.set_scroll_offset(offset);
        self.last_scroll_origin = None;
        self.sync_preview_scroll_with_mode(true, window, cx);
        self.last_preview_sync_offset = self.preview_list.offset();
        cx.notify();
    }

    fn preview_scroll_to_ratio(&mut self, ratio: f32, window: &mut Window, cx: &mut Context<Self>) {
        let ratio = ratio.clamp(0.0, 1.0);
        self.cancel_pending_preview_seek();
        self.preview_list
            .set_scroll_offset(ratio * self.preview_list.max_scroll());
        self.last_scroll_origin = None;
        self.sync_preview_scroll(window, cx);
        self.last_preview_sync_offset = self.preview_list.offset();
        cx.notify();
    }

    /// Returns `(current_y, max_scroll, ratio)` from the editor uniform list handle.
    fn editor_scroll_metrics(&self) -> (f32, f32, f32) {
        let handle = self.editor_scroll_handle.0.borrow();
        let max_scroll = f32::from(handle.base_handle.max_offset().y).max(0.0);
        let current_y = (-f32::from(handle.base_handle.offset().y)).clamp(0.0, max_scroll);
        let ratio = if max_scroll > 0.5 {
            (current_y / max_scroll).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (current_y, max_scroll, ratio)
    }

    fn editor_scrollbar_handle(&self) -> EditorScrollbarHandle {
        EditorScrollbarHandle {
            inner: self.editor_scroll_handle.clone(),
            dirty: self.editor_scrollbar_dirty.clone(),
            dragging: self.editor_scrollbar_dragging.clone(),
            shared_content_height: self.split_scrollbar_content_height.clone(),
        }
    }

    fn render_overlay_scrollbar<H: gpui_component::scroll::ScrollbarHandle + Clone>(
        id: &'static str,
        selector: &'static str,
        handle: &H,
    ) -> AnyElement {
        div()
            .absolute()
            .inset_0()
            .debug_selector(move || selector.to_string())
            .child(
                gpui_component::scroll::Scrollbar::vertical(handle)
                    .id(id)
                    .mode(gpui_component::scroll::ScrollbarMode::Hover)
                    .viewport_from_layout()
                    .styles(|styles| {
                        styles
                            .thumb(|t| t.radius(px(999.)))
                            .thumb_hover(|t| t.radius(px(999.)))
                            .thumb_active(|t| t.radius(px(999.)))
                    }),
            )
            .into_any_element()
    }

    fn render_editor_scrollbar(&self) -> AnyElement {
        Self::render_overlay_scrollbar(
            "editor_scrollbar_native",
            "editor_scrollbar",
            &self.editor_scrollbar_handle(),
        )
    }

    fn render_preview_scrollbar(&self) -> AnyElement {
        Self::render_overlay_scrollbar(
            "preview_scrollbar_native",
            "preview_scrollbar",
            &self.preview_list,
        )
    }

    /// A new document starts from a preview scrolled to the top, parsing only if needed.
    /// Reclaims GPU texture memory by dropping any active HiDPI lightbox image.
    pub fn on_document_swapped(&mut self, mut window: Option<&mut Window>, cx: &mut App) {
        self.release_hidden_diagrams = true;
        if let Some(image) = self.lightbox_local_image.take() {
            if let Some(render) = &image.render { cx.drop_image(render.clone(), window.as_deref_mut()); }
        }
        self.find.invalidate();
        if self.state.doc().path.as_ref().is_some_and(|p| !zmark_core::file_ops::is_markdown(p)) {
            self.state.view_mode = ViewMode::Source;
        }
        self.is_renaming = false;
        self.rename_target = None;
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
                if handle
                    .update(cx, |_view, window, cx| {
                        crate::views::lightbox::release_lightbox_image(&p_clone, window, cx);
                    })
                    .is_err()
                {
                    let resource =
                        gpui::Resource::Path(std::sync::Arc::from(exclusive_path.as_path()));
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
        let mode = if self.state.doc().path.as_ref().is_some_and(|p| !zmark_core::file_ops::is_markdown(p)) {
            ViewMode::Source
        } else { mode };
        self.cancel_source_composition();
        self.end_source_drag();
        self.cancel_pending_preview_seek();
        self.end_preview_scroll_drag();
        self.cancel_synced_scroll();
        self.editor_scrollbar_dragging.set(false);
        self.split_scrollbar_content_height.set(None);
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

    pub fn on_editor_click_line(&mut self, vis_idx: usize, vis_col: usize, window: &mut Window, cx: &mut App) {
        self.cancel_source_composition();
        self.end_source_drag();
        window.focus(&self.focus_handle, cx);
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
        text_system: &gpui::WindowTextSystem,
        _cx: &mut Context<Self>,
    ) {
        self.cancel_pending_preview_seek();
        let num_blocks = self.state.doc().parsed.blocks.len();
        if num_blocks == 0 {
            self.preview_autoscroll_task = None;
            return;
        }

        if velocity > 0.0 {
            let max_rendered = self
                .rendered_regions
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

        // Advance one block per ~24px of edge velocity, matching the old
        // ListState cadence while using real size-table offsets.
        if velocity < 0.0 {
            self.preview_autoscroll_accum = 0.0;
            self.preview_list
                .set_scroll_offset(self.preview_list.offset() + velocity);
        } else {
            self.preview_autoscroll_accum += velocity;
            let steps = (self.preview_autoscroll_accum / 24.0).floor() as usize;
            if steps > 0 {
                self.preview_autoscroll_accum -= steps as f32 * 24.0;
                let offset = self.preview_list.offset();
                let (ix, _) = self.preview_list.block_progress_at_offset(offset);
                let target = (ix + steps).min(num_blocks.saturating_sub(1));
                self.preview_list
                    .set_scroll_offset(self.preview_list.offset_for_block(target, 0.0));
                if target + 2 >= num_blocks {
                    self.preview_autoscroll_task = None;
                }
            } else {
                self.preview_list
                    .set_scroll_offset(self.preview_list.offset() + velocity);
            }
        }
        self.last_preview_sync_offset = self.preview_list.offset();

        if let Some(sel) = self.preview_selection {
            let reg = self.rendered_regions.lock().unwrap();
            if let Some(head) = crate::views::preview::hit_test_point_across_all_regions(
                &reg,
                text_system,
                mouse_pos,
                &self.state.doc().parsed.blocks,
            ) {
                self.preview_selection = Some(crate::views::preview::PreviewSelection::new(
                    sel.anchor, head,
                ));
            }
        }
    }

    fn end_preview_selection(&mut self) {
        self.is_preview_selecting = false;
        self.preview_autoscroll_task = None;
        self.preview_autoscroll_accum = 0.0;
        self.preview_mouse_window_pos = None;
    }

    pub fn trigger_preview_autoscroll_if_needed(&mut self, cx: &mut Context<Self>) {
        if !self.is_preview_selecting {
            self.preview_autoscroll_task = None;
            self.preview_autoscroll_accum = 0.0;
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
        if crate::views::preview::detect_edge_autoscroll(mouse_y, bounds_top, bounds_height)
            .is_some()
        {
            if self.preview_autoscroll_task.is_none() {
                let Some(window_handle) = self.window_handle else { return; };
                let view = cx.entity().downgrade();
                self.preview_autoscroll_task = Some(cx.spawn(async move |_this, app| loop {
                    app.background_executor()
                        .timer(std::time::Duration::from_millis(16))
                        .await;
                    let keep_going = window_handle
                        .update(app, |_, window, cx| view.update(cx, |this, cx| {
                            if !this.is_preview_selecting || this.preview_autoscroll_task.is_none()
                            {
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
                            let Some(vel) = crate::views::preview::detect_edge_autoscroll(
                                mouse_y,
                                bounds_top,
                                bounds_height,
                            ) else {
                                this.preview_autoscroll_task = None;
                                return false;
                            };
                            this.step_preview_autoscroll(vel, mouse_pos, bounds, window.text_system(), cx);
                            if this.preview_autoscroll_task.is_none() {
                                return false;
                            }
                            cx.notify();
                            true
                        }))
                        .and_then(|result| result)
                        .unwrap_or(false);
                    if !keep_going {
                        break;
                    }
                }));
            }
        }
    }

    pub fn theme(&self, _cx: &App) -> &crate::theme::ThemeColors {
        &self.state.theme
    }

    /// Drops the crisp re-render and cancels any pending one.
    fn clear_lightbox_sharp(&mut self, window: &mut Window, cx: &mut App) {
        self.lightbox_sharp_task = None;
        self.lightbox_sharp_want = (0, 0);
        if let Some((image, _)) = self.lightbox_sharp.take() {
            crate::views::lightbox::drop_image_from_atlas(cx, image, window);
        }
    }

    /// After zoom settles, re-rasterizes the SVG at zoom x screen scale so enlarged
    /// diagrams stay sharp. The old image stays on screen until the new one is ready.
    fn schedule_lightbox_sharp(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let want = crate::views::lightbox::sharp_target_px(&self.lightbox, window.scale_factor());
        if want == (0, 0) {
            if self.lightbox_sharp.is_some() || self.lightbox_sharp_task.is_some() {
                self.clear_lightbox_sharp(window, cx);
            }
            return;
        }
        let current = self.lightbox_sharp.as_ref().map(|(_, px)| *px);
        if want == self.lightbox_sharp_want || Some(want) == current {
            return;
        }
        let Some(path) = self.lightbox.svg_path.clone() else { return };
        self.lightbox_sharp_want = want;
        let renderer = cx.svg_renderer();
        self.lightbox_sharp_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(120)).await;
            let image = cx
                .background_executor()
                .spawn(async move {
                    let bytes = std::fs::read(&path).ok()?;
                    let svg = renderer.parse_svg(&bytes).ok()?;
                    renderer
                        .render_parsed(
                            &svg,
                            gpui::SvgSize::ExactSize(gpui::size(
                                gpui::DevicePixels(want.0 as i32),
                                gpui::DevicePixels(want.1 as i32),
                            )),
                        )
                        .ok()
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                // A newer request or a closed lightbox makes this result stale.
                if this.lightbox_sharp_want != want || !this.lightbox.is_open {
                    return;
                }
                if let Some(image) = image {
                    if let Some((old, _)) = this.lightbox_sharp.replace((image, want)) {
                        crate::views::lightbox::drop_image_from_atlas(cx, old, window);
                    }
                    if let Some(fallback) = this.lightbox.hidpi_svg_path.take() {
                        crate::views::lightbox::release_lightbox_image(&fallback, window, cx);
                    }
                    cx.notify();
                }
            });
        }));
    }

    pub fn close_lightbox(&mut self, window: &mut Window, cx: &mut App) {
        // Opening a modal on mouse-down can hide the matching mouse-up from the panes.
        self.end_source_drag();
        self.end_preview_selection();
        self.end_preview_scroll_drag();
        self.is_resizing_split = false;
        self.clear_lightbox_sharp(window, cx);
        if let Some(image) = self.lightbox_local_image.take() {
            let shared = self.local_images.lock().unwrap().entries.values().any(|state| matches!(state,
                crate::local_images::ImageState::Ready(other) if Arc::ptr_eq(other, &image)));
            if !shared { image.release(window, cx); }
        }
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

    /// Opens a local image snapshot as it is. Unlike diagrams, snapshots are already
    /// display-sized and are not re-rendered per theme or DPI.
    pub fn open_image_lightbox(
        &mut self,
        image_path: std::path::PathBuf,
        window: &mut Window,
        cx: &mut App,
    ) {
        let target = self
            .local_images
            .lock()
            .unwrap()
            .entries
            .values()
            .find_map(|state| match state {
                crate::local_images::ImageState::Ready(image) if image.path() == image_path => Some(image.clone()),
                _ => None,
            });
        self.close_lightbox(window, cx);
        self.lightbox_local_image = target;
        let evicted = if let Some(image) = &self.lightbox_local_image {
            self.lightbox.open_with_size(image_path, 0, image.natural_size)
        } else { self.lightbox.open(image_path, 0) };
        if let Some(evicted) = evicted {
            crate::views::lightbox::release_lightbox_image(&evicted, window, cx);
        }
    }

    pub fn open_lightbox_with_scale(
        &mut self,
        svg_path: std::path::PathBuf,
        code_hash: u64,
        scale_factor: f32,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.close_lightbox(window, cx);

        let is_sys_dark = matches!(
            window.appearance(),
            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
        );
        self.state.update_appearance(is_sys_dark);

        let scale_bucket = zmark_core::mermaid::scale_factor_to_bucket(scale_factor);
        let is_dark = self.theme(cx).is_dark;
        self.lightbox.is_dark = Some(is_dark);

        let actual_svg_path = resolve_lightbox_svg_path(&svg_path, code_hash, is_dark);

        let orig_svg = std::fs::read_to_string(&actual_svg_path).ok();
        let targets = orig_svg.as_ref().map(|svg| {
            let (w, h) = zmark_core::mermaid::extract_svg_dimensions(svg);
            let mut fitted = LightboxState::new();
            fitted.open_with_size(actual_svg_path.clone(), code_hash, (w as f32, h as f32));
            fitted.update_viewport(window.viewport_size());
            zmark_core::mermaid::calculate_hidpi_dimensions(w * fitted.zoom as f64, h * fitted.zoom as f64, scale_factor)
        });
        let cached_path = crate::views::lightbox::LIGHTBOX_DIAGRAM_CACHE.lock().unwrap()
            .get(&(code_hash, is_dark, scale_bucket));
        let cached_path = cached_path.filter(|path| {
            std::fs::read_to_string(path).ok().is_some_and(|svg|
                Some(zmark_core::mermaid::extract_svg_dimensions(&svg)) == targets)
        });

        let hidpi_path = if let Some(path) = cached_path {
            Some(path)
        } else {
            if let (Some(orig_svg), Some((target_w, target_h))) = (orig_svg, targets) {
                let hidpi_svg = zmark_core::mermaid::apply_texture_pixel_budget(
                    &orig_svg, target_w, target_h,
                );
                let cache_dir = std::env::temp_dir().join("zmark").join("mermaid_hidpi");
                let filename = format!(
                    "{:016x}_{}_{}_v3.svg",
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
                        let mut cache = crate::views::lightbox::LIGHTBOX_DIAGRAM_CACHE
                            .lock()
                            .unwrap();
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
            if let Some(evicted) = self
                .lightbox
                .open_hidpi(actual_svg_path, hidpi_path, code_hash)
            {
                crate::views::lightbox::release_lightbox_image(&evicted, window, cx);
            }
        } else {
            if let Some(evicted) = self.lightbox.open(actual_svg_path, code_hash) {
                crate::views::lightbox::release_lightbox_image(&evicted, window, cx);
            }
        }
    }

    pub fn toggle_drawer(&mut self, cx: &mut Context<Self>) {
        self.drawer_close_task = None;
        if self.state.preferences.reduce_motion {
            self.state.drawer_open = !self.state.drawer_open;
            self.drawer_closing = false;
            cx.notify();
            return;
        }
        if self.drawer_closing {
            self.drawer_closing = false;
            self.state.drawer_open = true;
            cx.notify();
            return;
        }

        if self.state.drawer_open {
            self.drawer_closing = true;
            cx.notify();
            self.drawer_close_task = Some(cx.spawn(async move |this, app| {
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
            }));
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
            let bounds = Bounds::centered(None, size(px(680.), px(540.)), cx);
            let handle = cx.open_window(
                WindowOptions {
                    titlebar: crate::platform::auxiliary_titlebar_options(
                        &format!("ZMark - {}", strings.settings),
                        14.0,
                    ),
                    window_decorations: crate::platform::window_decorations(),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| {
                        SettingsWindowView::new(ws, current_tab, preferences, theme, strings, cx)
                    });
                    cx.new(|cx| gpui_component::Root::new(view, window, cx))
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
        if let Some(key) = self.state.active.clone() { self.trigger_auto_save_for(key, cx); }
    }

    fn trigger_auto_save_for(&mut self, key: crate::state::DocKey, cx: &mut Context<Self>) {
        if !self.state.preferences.auto_save || self.sidebar_busy { return; }
        self.auto_save_tasks.remove(&key);
        self.next_auto_save_id = self.next_auto_save_id.wrapping_add(1);
        let task_id = self.next_auto_save_id;
        let delay_ms = self.state.preferences.auto_save_delay_ms;
        let save_key = key.clone();
        let cleanup_entity = cx.entity().downgrade();
        let task = cx.spawn(async move |this, app| {
            app.background_executor()
                .timer(std::time::Duration::from_millis(delay_ms))
                .await;
            let _ = this.update(app, |this, cx| {
                if this.state.active.as_ref() == Some(&save_key) {
                    // The editor buffer is materialized on the parse/save path,
                    // not on every keystroke; refresh it before the dirty check.
                    this.state.sync_from_editor();
                }
                let dirty = this.state.open_docs.iter().any(|doc| {
                    doc.key == save_key
                        && doc.doc.status == zmark_core::document::DocumentStatus::Modified
                });
                if dirty {
                    if let Err(error) = this.state.save_doc(&save_key) {
                        this.error_banner = Some(format!("保存失败：{error}"));
                    }
                    cx.notify();
                }
                let cleanup_key = save_key.clone();
                let cleanup_entity = cleanup_entity.clone();
                cx.defer(move |cx| {
                    let _ = cleanup_entity.update(cx, |this, _| {
                        if this
                            .auto_save_tasks
                            .get(&cleanup_key)
                            .is_some_and(|(id, _)| *id == task_id)
                        {
                            this.auto_save_tasks.remove(&cleanup_key);
                        }
                    });
                });
            });
        });
        self.auto_save_tasks.insert(key, (task_id, task));
    }

    pub fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(key) = self.state.active.clone() { self.begin_rename_target(key, false, window, cx); }
    }

    pub fn begin_rename_target(&mut self, key: crate::state::DocKey, sidebar: bool, window: &mut Window, cx: &mut Context<Self>) {
        let is_dir = key.as_path().is_some_and(|p| self.state.folder_expanded(p).is_some());
        let name = if let Some(doc) = self.state.open_documents().iter().find(|d| d.key == key) {
            doc.doc.display_name_with_fallback("Untitled.md")
        } else if let Some(path) = key.as_path().filter(|p| p.exists()) {
            path.file_name().unwrap_or_default().to_string_lossy().into_owned()
        } else { return; };
        let end = if is_dir { name.len() } else {
            Path::new(&name).extension().and_then(|ext| ext.to_str())
                .filter(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
                .map(|ext| name.len() - ext.len() - 1).unwrap_or(name.len())
        };
        self.ensure_inputs(window, cx);
        let input = self.rename_input.as_ref().unwrap().clone();
        input.update(cx, |input, cx| {
            input.set_value(name, window, cx);
            input.set_selected_range(0..end, cx);
        });
        self.rename_target = Some(key);
        self.rename_in_sidebar = sidebar;
        self.is_renaming = true;
        input.update(cx, |input,cx| {
            input.set_selection_focus(None, cx);
            input.focus(window,cx);
        });
        cx.notify();
    }

    pub fn finish_rename(&mut self, commit: bool, cx: &mut Context<Self>) {
        if !self.is_renaming { return; }
        self.is_renaming = false;
        let target = self.rename_target.take().or_else(|| self.state.active.clone());
        let name = self.rename_input.as_ref().unwrap().read(cx).value().trim().to_owned();
        if commit {
            if let Some(target) = target {
                let old = target.as_path().cloned();
                let directory = old.as_ref().is_some_and(|p| self.state.folder_expanded(p).is_some());
                let new = old.as_ref().and_then(|p| p.parent()).map(|parent| {
                    parent.join(if directory { name.clone() } else { zmark_core::file_ops::markdown_name(&name) })
                });
                let original = self.state.open_documents().iter().find(|d| d.key == target)
                    .map(|d| d.doc.display_name_with_fallback("Untitled.md"))
                    .or_else(|| old.as_ref().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().into_owned()));
                if original.as_deref() != Some(&name) {
                    match self.state.rename_document(&target, &name) {
                        Ok(warning) => {
                            if let (Some(old), Some(new)) = (old, new) {
                                if old != new { self.on_paths_renamed(&old, &new, cx); }
                                self.error_banner = Some(warning.unwrap_or_else(|| "已更名；旧名称的链接或图片引用需手动更新".into()));
                            }
                        }
                        Err(error) => self.error_banner = Some(format!("更名失败：{error}")),
                    }
                }
            }
        }
        cx.notify();
    }

    fn on_paths_renamed(&mut self, old: &Path, new: &Path, cx: &mut Context<Self>) {
        self.sidebar_selection.lock().unwrap().remap(old, new);
        self.auto_save_tasks.retain(|key, _| !key.as_path().is_some_and(|p| p.starts_with(old)));
        let affected: Vec<_> = self.state.open_documents().iter()
            .filter(|doc| doc.key.as_path().is_some_and(|p| p.starts_with(new)))
            .map(|doc| doc.key.clone()).collect();
        for key in affected { self.trigger_auto_save_for(key, cx); }
        if let Some(path) = &self.selected_directory {
            if let Some(mapped) = zmark_core::file_ops::remap_path(path, old, new) { self.selected_directory = Some(mapped); }
        }
        self.folder_transitions.retain(|path, _| !path.starts_with(old));
        self.local_image_task = None;
        self.local_images.lock().unwrap().refresh();
        self.syntax_cancel_flag.store(true, Ordering::Relaxed);
        self.syntax_active_task = None;
        self.syntax_pending_request = None;
        self.schedule_parse(cx);
    }

    pub fn toggle_folder(&mut self, root: usize, path: &Path, cx: &mut Context<Self>) {
        let Some(expanded) = self.state.folder_expanded(path) else { return; };
        self.folder_transitions.entry(path.to_path_buf()).or_default();
        if self.state.toggle_root_folder(root, path) {
            if expanded && self.rename_target.as_ref().and_then(|k| k.as_path()).is_some_and(|p| p != path && p.starts_with(path)) {
                self.finish_rename(false, cx);
            }
            cx.notify();
        }
    }

    pub fn trigger_save(&mut self, cx: &mut Context<Self>) {
        if self.sidebar_busy { return; }
        if self.state.active.is_none() {
            return;
        }
        self.parse_now_and_refresh();
        if self.state.doc().path.is_some() {
            if let Err(error) = self.state.save() {
                self.error_banner = Some(format!("保存失败：{error}"));
            }
            cx.notify();
        } else {
            let save_key = self.state.active.clone().unwrap();
            let suggested_dir = self
                .state
                .workspace_roots
                .first()
                .map(|r| r.path.clone())
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let suggested_name = self.state.doc().display_name_with_fallback("Untitled.md");
            let rx = cx.prompt_for_new_path(&suggested_dir, Some(&suggested_name));
            cx.spawn(async move |this, app| {
                if let Ok(Ok(Some(path))) = rx.await {
                    let _ = this.update(app, |this, cx| {
                        match this.state.save_doc_as(&save_key, path) {
                            Ok(()) => this.on_document_swapped(None, cx),
                            Err(error) => this.error_banner = Some(format!("另存为失败：{error}")),
                        }
                        cx.notify();
                    });
                }
            })
            .detach();
        }
    }

    /// Exports the active document to a user-chosen `.html` path.
    ///
    /// The old handler silently wrote next to an already-saved document and did
    /// nothing for drafts; this always opens the native save panel, writes off
    /// the UI thread and shows a dismissible success notice.
    pub fn trigger_export_html(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.state.active.is_none() {
            return;
        }
        self.parse_now_and_refresh();
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let (dir, suggested_name) = export_suggested_path(
            self.state.doc().path.as_deref(),
            self.state.strings.untitled_file,
            &home,
        );
        let html = self.state.export_standalone_html();
        let rx = cx.prompt_for_new_path(&dir, Some(&suggested_name));
        cx.spawn(async move |this, app| {
            let Ok(Ok(Some(path))) = rx.await else {
                return;
            };
            let write_path = path.clone();
            let result = app
                .background_executor()
                .spawn(async move { std::fs::write(&write_path, html) })
                .await;
            let _ = this.update(app, |this, cx| match result {
                Ok(()) => this.show_export_notice(path, cx),
                Err(error) => {
                    this.error_banner = Some(format!("{}: {error}", this.state.strings.export_html));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn show_export_notice(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.export_notice_generation = self.export_notice_generation.wrapping_add(1);
        let generation = self.export_notice_generation;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        self.export_notice = Some((name, path));
        self.export_notice_closing = false;
        cx.notify();
        let view = cx.entity().downgrade();
        cx.spawn(async move |_this, app| {
            app.background_executor()
                .timer(std::time::Duration::from_secs(3))
                .await;
            let _ = view.update(app, |this, cx| {
                if this.export_notice_generation == generation {
                    this.begin_export_notice_exit(cx);
                }
            });
        })
        .detach();
    }

    /// Plays the 120ms exit animation before unmounting the export notice.
    fn begin_export_notice_exit(&mut self, cx: &mut Context<Self>) {
        if self.export_notice.is_none() || self.export_notice_closing {
            return;
        }
        if self.state.preferences.reduce_motion {
            self.export_notice = None;
            self.export_notice_closing = false;
            cx.notify();
            return;
        }
        self.export_notice_closing = true;
        cx.notify();
        let generation = self.export_notice_generation;
        let view = cx.entity().downgrade();
        cx.spawn(async move |_this, app| {
            app.background_executor()
                .timer(std::time::Duration::from_millis(120))
                .await;
            let _ = view.update(app, |this, cx| {
                if this.export_notice_generation == generation {
                    this.export_notice = None;
                    this.export_notice_closing = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn exit_sidebar_multiselect(&mut self, cx: &mut Context<Self>) {
        let mut selection = self.sidebar_selection.lock().unwrap();
        if selection.selected.len() <= 1 { return; }
        selection.clear();
        if let Some(key) = self.state.active.clone() { selection.click(key, false, false); }
        drop(selection);
        cx.notify();
    }

    fn sidebar_row_click(&mut self, key: &crate::state::DocKey, event: &MouseDownEvent, name: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.sidebar_busy { return; }
        if let crate::state::DocKey::Path(path) = key {
            if self.state.folder_expanded(path).is_some() {
                self.exit_sidebar_multiselect(cx);
                self.selected_directory = Some(path.clone());
                if name && event.click_count == 2 {
                    self.begin_rename_target(key.clone(), true, window, cx);
                } else if event.click_count == 1 {
                    if let Some(root) = self.state.workspace_roots.iter().position(|r| path.starts_with(&r.path)) {
                        self.toggle_folder(root, path, cx);
                    }
                }
                cx.notify();
                return;
            }
        }
        let additive = if cfg!(target_os = "macos") { event.modifiers.platform } else { event.modifiers.control };
        self.sidebar_selection.lock().unwrap().click(key.clone(), event.modifiers.shift, additive);
        if matches!(key, crate::state::DocKey::Path(_)) && (event.modifiers.shift || additive) { cx.notify(); return; }
        match key {
            crate::state::DocKey::Path(path) => {
                self.selected_directory = None;
                if name && event.click_count == 2 { self.begin_rename_target(key.clone(), true, window, cx); }
                else {
                    if let Err(error) = self.state.load_file(path.clone()) { self.error_banner = Some(error.to_string()); }
                    self.on_document_swapped(Some(window), cx);
                }
            }
            crate::state::DocKey::Scratch(_) => {
                self.selected_directory = None;
                self.state.switch_document(key);
                self.on_document_swapped(Some(window), cx);
                if name && event.click_count == 2 { self.begin_rename_target(key.clone(), true, window, cx); }
            }
        }
        cx.notify();
    }

    fn open_or_add_workspace_dir(
        &mut self,
        path: &Path,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.restore_sidebar_path(path, cx);
        if self.state.find_root_index_for_path(path).is_some() {
            self.state.expand_to_file(path);
            return Ok(());
        }
        match self.state.add_workspace_root(path.to_path_buf()) {
            Ok(_) => {
                self.state.expand_to_file(path);
                Ok(())
            }
            Err(error) => Err(format_root_add_error(self.state.strings, error)),
        }
    }

    fn restore_sidebar_path(&mut self, path: &Path, cx: &mut Context<Self>) {
        let paths = self.state.preferences.sidebar_hidden_paths.iter().filter(|hidden| path.starts_with(hidden)).cloned().collect::<Vec<_>>();
        if !paths.is_empty() { self.persist_sidebar_hidden(paths, false, cx); }
    }

    fn persist_sidebar_hidden(&mut self, paths: Vec<PathBuf>, hide: bool, cx: &mut Context<Self>) {
        if self.sidebar_busy { return; }
        let old = self.state.preferences.sidebar_hidden_paths.clone();
        if hide {
            for path in paths { if !self.state.preferences.sidebar_hidden_paths.contains(&path) { self.state.preferences.sidebar_hidden_paths.push(path); } }
        } else {
            self.state.preferences.sidebar_hidden_paths.retain(|path| !paths.contains(path));
        }
        match self.state.preferences.save_to_path(&self.state.preferences_path) {
            Ok(()) => {
                self.sidebar_selection.lock().unwrap().clear(); self.error_banner = None;
                self.sidebar_undo_task = None;
                self.sidebar_undo = if hide { Some(self.state.preferences.sidebar_hidden_paths.iter().filter(|path| !old.contains(path)).cloned().collect()) } else { None };
                if self.sidebar_undo.as_ref().is_some_and(|paths| !paths.is_empty()) {
                    self.sidebar_undo_task = Some(cx.spawn(async move |this, app| {
                        app.background_executor().timer(std::time::Duration::from_secs(5)).await;
                        let _ = this.update(app, |this,cx| { this.sidebar_undo = None; cx.notify(); });
                    }));
                }
            }
            Err(error) => { self.state.preferences.sidebar_hidden_paths = old; self.error_banner = Some(error.to_string()); }
        }
        cx.notify();
    }

    fn sidebar_batch(&mut self, trash: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.sidebar_busy { return; }
        let keys: Vec<_> = self.sidebar_selection.lock().unwrap().selected.iter().cloned().collect();
        let Some(paths) = keys.iter().map(|key| key.as_path().cloned()).collect::<Option<Vec<_>>>() else {
            self.error_banner = Some(self.state.strings.save_before_batch.to_string()); cx.notify(); return;
        };
        if paths.is_empty() { return; }
        if !trash { self.persist_sidebar_hidden(paths, true, cx); return; }
        self.trash_sidebar_paths(paths, window, cx);
    }

    fn trash_sidebar_paths(&mut self, mut paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        if self.sidebar_busy { return; }
        paths.sort(); paths.dedup();
        // A selected parent owns its descendants; never trash a child twice.
        let targets: Vec<_> = paths.iter().filter(|path| !paths.iter().any(|parent| parent != *path && path.starts_with(parent))).cloned().collect();
        self.state.sync_from_editor();
        let strings = self.state.strings;
        let modified = self.state.open_docs.iter().filter(|doc| doc.doc.status != zmark_core::document::DocumentStatus::Clean && doc.key.as_path().is_some_and(|p| targets.iter().any(|target| p.starts_with(target)))).map(|doc| format!("* {}", doc.doc.display_name_with_fallback(strings.untitled_file))).collect::<Vec<_>>().join("\n");
        let detail = format!("{}\n\n{}\n{}", strings.trash_file_detail, targets.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n"), modified);
        let answer = window.prompt(PromptLevel::Warning, &format!("{} ({}/{})", strings.trash_file, targets.len(), paths.len()), Some(&detail), &[PromptButton::ok(strings.trash_file), PromptButton::cancel(strings.modal_cancel)], cx);
        self.sidebar_busy = true;
        cx.notify();
        cx.spawn(async move |this, app| {
            if answer.await != Ok(0) { let _ = this.update(app, |this,cx| {this.sidebar_busy=false;cx.notify();}); return; }
            let affected = this.update(app, |this,_| {
                this.state.sync_from_editor();
                let affected: Vec<_> = this.state.open_docs.iter().filter(|doc| doc.key.as_path().is_some_and(|p| targets.iter().any(|target| p.starts_with(target)))).map(|doc| doc.key.clone()).collect();
                for key in &affected { this.auto_save_tasks.remove(key); }
                affected
            });
            let Ok(affected) = affected else { return; };
            let results = app.background_executor().spawn(async move {
                targets.into_iter().map(|path| { let result = crate::platform::move_file_to_trash(&path); (path,result) }).collect::<Vec<_>>()
            }).await;
            let _ = this.update(app, |this,cx| {
                this.sidebar_busy = false;
                let succeeded: Vec<_> = results.iter().filter(|(_,result)| result.is_ok()).map(|(p,_)| p.clone()).collect();
                for key in affected {
                    if key.as_path().is_some_and(|p| succeeded.iter().any(|target| p.starts_with(target))) {
                        this.auto_save_tasks.remove(&key);
                        this.state.close_document_forced(&key);
                    } else { this.trigger_auto_save_for(key,cx); }
                }
                for root in &mut this.state.workspace_roots { if succeeded.iter().any(|p| root.path.starts_with(p)) { root.is_missing = true; } }
                this.state.refresh_all_roots();
                let query = this.state.search_query.clone(); this.update_search_query(&query,cx);
                {
                    let mut selection = this.sidebar_selection.lock().unwrap();
                    selection.selected.retain(|key| !key.as_path().is_some_and(|p| succeeded.iter().any(|target| p.starts_with(target))));
                    selection.focus = selection.selected.iter().next().cloned();
                }
                this.sidebar_busy = false;
                this.on_document_swapped(None,cx);
                let failures: Vec<_> = results.into_iter().filter_map(|(path,result)| result.err().map(|error| format!("{}: {}",path.display(),error))).collect();
                this.error_banner = (!failures.is_empty()).then(|| failures.join("\n"));
                cx.notify();
            });
        }).detach();
    }

    fn trigger_trash_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.trash_sidebar_paths(vec![path], window, cx);
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
        self.pending_restart = false;
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

    pub fn handle_modal_discard(
        &mut self,
        key: &crate::state::DocKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.auto_save_tasks.remove(key);
        self.state.confirm_close_discard(key);
        self.on_document_swapped(Some(window), cx);
        self.after_dirty_doc_resolved(Some(window), cx);
    }

    pub fn handle_modal_save(
        &mut self,
        key: &crate::state::DocKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
                self.error_banner = Some(format!("保存失败：{e}"));
                cx.notify();
            }
        }
    }

    pub fn prompt_save_as_and_close(
        &mut self,
        close_key: crate::state::DocKey,
        cx: &mut Context<Self>,
    ) {
        let was_quitting = self.state.is_quitting;
        self.state.pending_close = None;
        cx.notify();

        let suggested_dir = self
            .state
            .workspace_roots
            .first()
            .map(|r| r.path.clone())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let suggested_name = self.state.open_docs.iter()
            .find(|doc| doc.key == close_key)
            .map(|doc| doc.doc.display_name_with_fallback("Untitled.md"))
            .unwrap_or_else(|| "Untitled.md".to_string());
        let rx = cx.prompt_for_new_path(&suggested_dir, Some(&suggested_name));

        cx.spawn(async move |this, app| {
            if let Ok(Ok(Some(path))) = rx.await {
                let _ = this.update(app, |this, cx| {
                    match this.state.save_doc_as(&close_key, path.clone()) {
                        Ok(()) => {
                            this.state.close_document_forced(&crate::state::DocKey::Path(path));
                            this.on_document_swapped(None, cx);
                            this.state.is_quitting = was_quitting;
                            this.after_dirty_doc_resolved(None, cx);
                        }
                        Err(e) => {
                            this.error_banner = Some(format!("另存为失败：{e}"));
                            this.state.is_quitting = was_quitting;
                            let title = this
                                .state
                                .doc()
                                .display_name_with_fallback(this.state.strings.untitled_file);
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
                    this.pending_restart = false;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn init_update_service(&mut self, cx: &mut Context<Self>) {
        if !self.update_config.enabled {
            return;
        }
        let staged_path = self
            .update_config
            .staged_path
            .clone()
            .unwrap_or_else(crate::updater::staged_state_path);
        if !cfg!(target_os = "windows") {
            if let Some(prepared) = crate::updater::read_staged_state(&staged_path) {
                if crate::updater::is_newer(&self.update_config.current_version, &prepared.version) {
                    self.update_status = UpdateStatus::Downloaded(prepared.version.clone());
                    self.prepared_update = Some(prepared);
                } else {
                    let _ = std::fs::remove_file(&staged_path);
                }
            }
        }
        self.check_for_updates(cx, false);
        cx.spawn(async move |this, app| loop {
            app.background_executor()
                .timer(std::time::Duration::from_secs(24 * 60 * 60))
                .await;
            if this
                .update(app, |this, cx| this.check_for_updates(cx, false))
                .is_err()
            {
                break;
            }
        })
        .detach();
    }

    /// Checks the signed feed. Automatic checks stay quiet on network errors;
    /// manual checks (About tab) report the failure.
    pub fn check_for_updates(&mut self, cx: &mut Context<Self>, manual: bool) {
        if let Some(prepared) = &self.prepared_update {
            self.update_status = UpdateStatus::Downloaded(prepared.version.clone());
            cx.notify();
            return;
        }
        if !self.update_config.enabled {
            if manual {
                let reason = self
                    .update_config
                    .disabled_reason
                    .clone()
                    .unwrap_or_else(|| self.state.strings.update_disabled.to_string());
                self.update_status = UpdateStatus::Error(reason);
                cx.notify();
            }
            return;
        }
        if self.update_checking {
            return;
        }
        self.update_checking = true;
        self.update_status = UpdateStatus::Checking;
        cx.notify();
        let config = self.update_config.clone();
        cx.spawn(async move |this, app| {
            let result = app
                .background_executor()
                .spawn(async move { crate::updater::run_check(&config) })
                .await;
            let _ = this.update(app, |this, cx| this.apply_update_result(result, manual, cx));
        })
        .detach();
    }

    fn apply_update_result(
        &mut self,
        result: Result<Option<PreparedUpdate>, crate::updater::UpdateError>,
        manual: bool,
        cx: &mut Context<Self>,
    ) {
        self.update_checking = false;
        match result {
            Ok(Some(prepared)) => {
                self.update_status = UpdateStatus::Downloaded(prepared.version.clone());
                self.prepared_update = Some(prepared);
            }
            Ok(None) => {
                self.update_status = if manual {
                    UpdateStatus::Latest
                } else {
                    UpdateStatus::Idle
                };
            }
            Err(error) => {
                if manual {
                    self.update_status = UpdateStatus::Error(error.to_string());
                } else {
                    eprintln!("zmark update check failed: {error}");
                    self.update_status = UpdateStatus::Idle;
                }
            }
        }
        cx.notify();
    }

    /// Hides the update banner for the rest of this session.

    /// Starts the restart flow. Dirty documents go through the existing close
    /// prompt; the actual swap happens only after they are all resolved.
    pub fn restart_for_update(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        if self.prepared_update.is_none() {
            return;
        }
        self.pending_restart = true;
        if self.on_window_should_close(window, cx) {
            self.install_prepared_update(cx);
        }
    }

    fn install_prepared_update(&mut self, cx: &mut Context<Self>) {
        let Some(prepared) = self.prepared_update.take() else {
            return;
        };
        let Some(target) = self.update_config.target_bundle.clone() else {
            self.prepared_update = Some(prepared);
            self.pending_restart = false;
            self.update_status = UpdateStatus::Error("no target bundle configured".into());
            cx.notify();
            return;
        };
        match crate::updater::spawn_install_helper(&prepared, &target) {
            Ok(()) => {
                self.update_status = UpdateStatus::Installing;
                self.pending_restart = false;
                cx.quit();
            }
            Err(error) => {
                self.prepared_update = Some(prepared);
                self.pending_restart = false;
                self.update_status = UpdateStatus::Error(error.to_string());
                self.error_banner =
                    Some(format!("{}: {error}", self.state.strings.update_failed));
                cx.notify();
            }
        }
    }

    pub fn after_dirty_doc_resolved(
        &mut self,
        mut window_opt: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if self.state.is_quitting {
            if let Some(next_dirty) = self.state.next_dirty_doc_key() {
                self.state.request_close(&next_dirty);
                self.on_document_swapped(window_opt.as_deref_mut(), cx);
                cx.notify();
            } else {
                // All dirty documents resolved. A pending update still owns
                // the quit path; otherwise closing the window quits ZMark.
                self.state.is_quitting = false;
                if self.pending_restart {
                    self.install_prepared_update(cx);
                    return;
                }
                cx.quit();
            }
        } else {
            cx.notify();
        }
    }

    pub fn on_window_should_close(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) -> bool {
        let allow = self.state.handle_window_should_close();
        self.on_document_swapped(window, cx);
        cx.notify();
        allow
    }

    /// Re-fronts the main window when macOS activates the app (Dock click).
    ///
    /// GPUI's macOS reopen callback only runs when no window is visible, so an
    /// app whose window is behind another app or on another Space gets no
    /// application-level handling. Ordering the existing window front on the
    /// inactive → active transition covers both cases and is idempotent when
    /// the window is already frontmost.
    pub fn install_window_activation_observer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let subscription = cx.observe_window_activation(window, |this, window, cx| {
            let active = window.is_window_active();
            if active && !this.window_was_active {
                this.cancel_source_composition();
                match this.state.check_disk_changes() {
                    Ok(true) => this.on_document_swapped(Some(window), cx),
                    Ok(false) => {}
                    Err(error) => this.error_banner = Some(format!("检查磁盘文件失败：{error}")),
                }
                cx.notify();
            }
            if cfg!(target_os = "macos")
                && crate::platform::should_front_window(this.window_was_active, active)
            {
                window.activate_window();
            }
            this.window_was_active = active;
        });
        self.window_activation_subscription = Some(subscription);
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
        is_dirty: bool,
        dirty_generation: u64,
        reduce_motion: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_macos = cfg!(target_os = "macos");
        let drawer_open = self.state.drawer_open;
        let view_mode = self.state.view_mode;
        let strings = self.state.strings;
        let is_renaming = self.is_renaming && !self.rename_in_sidebar;
        let rename_input = self.rename_input.clone();

        let viewport_w = f32::from(window.viewport_size().width);
        let col_w = self.state.preferences.max_column_width;
        let header_pl = px(calculate_title_header_padding(
            is_macos,
            is_fullscreen,
            drawer_open,
            view_mode,
            viewport_w,
            col_w,
            self.editor_text_start_offset(window),
        ));

        // Right padding: on Windows/Linux, reserve 138px for system window controls (min/max/close).
        let header_pr = px(crate::platform::TRAILING_WINDOW_CONTROL_INSET);

        let active_key = self.state.active.clone();
        let (title_text, tooltip_text) = if active_key.is_some() {
            let title = self
                .state
                .doc()
                .display_name_with_fallback(strings.untitled_file);
            let tooltip = if let Some(path) = &self.state.doc().path {
                path.to_string_lossy().to_string()
            } else {
                strings.draft_unsaved_hint.to_string()
            };
            (title, tooltip)
        } else {
            (
                strings.no_document_open.to_string(),
                strings.no_document_open.to_string(),
            )
        };

        div()
            .id("canvas_integrated_header")
            // 38px matches the drawer header so both icon rows share one vertical
            // center (19px), which is where the native traffic lights sit.
            .h(px(38.))
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
                    .when(is_renaming, |el| el.flex_1())
                    .when(!drawer_open, |el| {
                        el.child(render_icon_button(
                            "btn_expand_drawer",
                            format!("{} ({})", strings.directory_drawer, crate::platform::shortcut_hint("Shift+B")),
                            false,
                            theme,
                            render_sidebar_icon(theme.text_muted),
                            cx.listener(|this, _, _window, cx| {
                                this.toggle_drawer(cx);
                            }),
                        ))
                    })
                    .child(if is_renaming && active_key.is_some() {
                        // Same position and type as the static title: no frame, only a
                        // blinking caret, so entering rename never shifts the layout.
                        div()
                            .id("tab_rename_box")
                            .key_context("InlineRename")
                            .debug_selector(|| "tab_rename_box".into())
                            .min_w(px(0.))
                            .w_full()
                            .flex_1()
                            .text_size(px(18.))
                            .font_weight(FontWeight::BOLD)
                            .line_height(px(24.))
                            .px_2()
                            .py_1()
                            .flex()
                            .items_center()
                            .text_color(theme.text_primary)
                            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _window, cx| {
                                this.finish_rename(false, cx);
                            }))
                            .child(crate::views::drawer::render_input(rename_input.as_ref().unwrap(), true).w_full().flex_1())
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
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                    if event.click_count == 2 {
                                        this.begin_rename(window, cx);
                                    }
                                }),
                            )
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::BOLD)
                                    .line_height(px(24.))
                                    .debug_selector(|| "editor_document_title".into())
                                    .text_color(if active_key.is_some() {
                                        theme.text_primary
                                    } else {
                                        theme.text_muted
                                    })
                                    .truncate()
                                    .child(title_text),
                            )
                            .map(|el| {
                                if is_dirty {
                                    crate::views::motion::animated(
                                        el,
                                        reduce_motion,
                                        format!("dirty_title_{dirty_generation}"),
                                        200,
                                        |el, delta| el.opacity(0.55 + 0.45 * delta),
                                    )
                                    .into_any_element()
                                } else {
                                    el.into_any_element()
                                }
                            })
                            .into_any_element()
                    }),
            )
            // Draggable Window Control Area in center between title and action tools
            .child(
                div()
                    .id("canvas_header_drag_area")
                    .debug_selector(|| "canvas_header_drag_area".into())
                    .flex_1()
                    .when(is_renaming, |el| el.flex_none().w(px(0.)))
                    .h_full()
                    .min_w(px(20.))
                    .occlude()
                    .window_control_area(WindowControlArea::Drag)
                    .when(!cfg!(target_os = "windows"), |el| {
                        el.on_mouse_down(MouseButton::Left, |_e, window, _| {
                            window.start_window_move();
                        })
                    }),
            )
            // Right Group: Flat View Mode Switcher + Action Buttons (Save, Export, Settings)
            .child(
                if self.sidebar_selection.lock().unwrap().selected.len() > 1 {
                    div().relative().flex().items_center().gap_2()
                        .child({
                            let selection = self.sidebar_selection.clone();
                            canvas(move |bounds, _, _| selection.lock().unwrap().batch_bounds = Some(bounds), |_, _, _, _| {})
                                .absolute().top_0().left_0().size_full()
                        })
                        .child(div().text_size(px(11.)).text_color(theme.text_muted)
                            .child(strings.selected_items.replace("{}", &self.sidebar_selection.lock().unwrap().selected.len().to_string())))
                        .child(render_icon_button("sidebar_batch_remove", strings.remove_from_sidebar, false, theme,
                            svg().path("icons/minus.svg").size(px(16.)).text_color(theme.text_muted),
                            cx.listener(|this,_,window,cx| this.sidebar_batch(false,window,cx))))
                        .child(render_icon_button("sidebar_batch_trash", strings.trash_file, false, theme,
                            svg().path("icons/trash.svg").size(px(16.)).text_color(theme.destructive_text()),
                            cx.listener(|this,_,window,cx| this.sidebar_batch(true,window,cx))))
                        .child(render_icon_button("sidebar_batch_cancel", strings.modal_cancel, false, theme,
                            svg().path("icons/x.svg").size(px(16.)).text_color(theme.text_muted),
                            cx.listener(|this,_,_,cx| {this.sidebar_selection.lock().unwrap().clear();cx.notify();})))
                        .into_any_element()
                } else {
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
                                format!("{} ({})", strings.source_mode, crate::platform::shortcut_hint("1")),
                                view_mode == ViewMode::Source,
                                theme,
                                render_source_mode_icon(
                                    mode_tab_colors(view_mode == ViewMode::Source, theme).1,
                                ),
                                cx.listener(|this, _, _window, cx| {
                                    this.set_view_mode(ViewMode::Source, cx);
                                }),
                            ))
                            .when(self.state.doc().path.as_ref().is_none_or(|p| zmark_core::file_ops::is_markdown(p)), |el| { el
                            .child(render_mode_icon_tab(
                                "btn_mode_split",
                                format!("{} ({})", strings.split_mode, crate::platform::shortcut_hint("2")),
                                view_mode == ViewMode::Split,
                                theme,
                                render_split_mode_icon(
                                    mode_tab_colors(view_mode == ViewMode::Split, theme).1,
                                ),
                                cx.listener(|this, _, _window, cx| {
                                    this.set_view_mode(ViewMode::Split, cx);
                                }),
                            ))
                            .child(render_mode_icon_tab(
                                "btn_mode_preview",
                                format!("{} ({})", strings.preview_mode, crate::platform::shortcut_hint("3")),
                                view_mode == ViewMode::Preview,
                                theme,
                                render_preview_mode_icon(
                                    mode_tab_colors(view_mode == ViewMode::Preview, theme).1,
                                ),
                                cx.listener(|this, _, _window, cx| {
                                    this.set_view_mode(ViewMode::Preview, cx);
                                }),
                            ))
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .when_some(
                                match &self.update_status {
                                    UpdateStatus::Downloaded(version) => Some(version.clone()),
                                    _ => None,
                                },
                                |el, version| {
                                    // One-shot pulse keyed by version: it replays when a new
                                    // update is staged, never loops, and is skipped entirely
                                    // under "Reduce motion".
                                    el.child(crate::views::motion::animated(
                                        div()
                                            .debug_selector(|| "btn_update_ready".to_string())
                                            .child(render_animated_icon_button(
                                                "btn_update_ready",
                                                format!("{} v{}", strings.restart_update, version),
                                                theme,
                                                svg()
                                                    .path("icons/refresh.svg")
                                                    .size(px(16.))
                                                    .text_color(theme.accent),
                                                reduce_motion,
                                                self.update_press_generation,
                                                cx.listener(|this, _, window, cx| {
                                                    this.update_press_generation =
                                                        this.update_press_generation.wrapping_add(1);
                                                    this.restart_for_update(Some(window), cx);
                                                }),
                                            )),
                                        reduce_motion,
                                        format!("update_ready_pulse_{version}"),
                                        360,
                                        |el, delta| el.opacity(0.35 + 0.65 * delta),
                                    ))
                                },
                            )
                            .child(render_animated_icon_button(
                                "btn_find", format!("{} · {}", strings.find_in_file, crate::platform::shortcut_hint("F")), theme,
                                svg().path("icons/search.svg").size(px(16.)).text_color(theme.text_muted),
                                reduce_motion, 0,
                                cx.listener(|this, _,window,cx| this.open_find(window,cx)),
                            ))
                            .child(render_animated_icon_button(
                                "btn_export",
                                format!("{} (⇧{})", strings.export_html, crate::platform::shortcut_hint("E")),
                                theme,
                                render_export_icon(theme.text_muted),
                                reduce_motion,
                                self.export_press_generation,
                                cx.listener(|this, _, window, cx| {
                                    this.export_press_generation =
                                        this.export_press_generation.wrapping_add(1);
                                    this.trigger_export_html(window, cx);
                                }),
                            ))
                            .child(render_animated_icon_button(
                                "btn_settings",
                                format!("{} ({})", strings.settings, crate::platform::shortcut_hint(",")),
                                theme,
                                render_settings_icon(theme.text_muted),
                                reduce_motion,
                                self.settings_press_generation,
                                cx.listener(|this, _, _window, cx| {
                                    this.settings_press_generation =
                                        this.settings_press_generation.wrapping_add(1);
                                    this.open_settings_window(cx);
                                }),
                            )),
                    ).into_any_element() }
)
            .when(cfg!(target_os = "windows") && !is_fullscreen, |el| {
                el.child(render_window_controls(theme))
            })
    }

    fn render_remove_root_modal(
        &self,
        theme: &crate::theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
                    .w(px(400.))
                    .p_6()
                    .rounded_xl()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_primary)
                                    .child(prompt_text),
                            )
                            .child(
                                div()
                                    .text_sm()
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
                            .child(render_modal_button(
                                "btn_remove_root_cancel",
                                format!("{} (Esc)", strings.modal_cancel),
                                theme,
                                ModalButtonRole::Secondary,
                                cx.listener(|this, _, _window, cx| {
                                    this.handle_cancel_remove_root(cx);
                                }),
                            ))
                            .child(render_modal_button(
                                "btn_remove_root_confirm",
                                strings.remove_root_confirm.to_string(),
                                theme,
                                ModalButtonRole::Danger,
                                cx.listener(|this, _, _window, cx| {
                                    this.handle_confirm_remove_root(cx);
                                }),
                            )),
                    ),
            )
    }

    fn render_close_modal(
        &self,
        theme: &crate::theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let strings = self.state.strings;
        let pending = self
            .state
            .pending_close
            .clone()
            .unwrap_or(crate::state::PendingClose {
                key: crate::state::DocKey::Scratch(0),
                title: String::new(),
                has_path: false,
            });
        let title = pending.title.clone();
        let key_discard = pending.key.clone();
        let key_save = pending.key.clone();
        let prompt_text = strings.modal_save_prompt.replace("{}", &title);
        let cancel_label = format!("{} ({})", strings.modal_cancel, "Esc");
        let discard_label = format!("{} ({})", strings.modal_discard, "D");
        let save_label = format!("{} ({})", strings.modal_save, "S");

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
                    .w(px(400.))
                    .p_6()
                    .rounded_xl()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
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
                            .child(render_modal_button(
                                "btn_modal_cancel",
                                cancel_label,
                                theme,
                                ModalButtonRole::Secondary,
                                cx.listener(|this, _, _window, cx| {
                                    this.handle_modal_cancel(cx);
                                }),
                            ))
                            .child(render_modal_button(
                                "btn_modal_discard",
                                discard_label,
                                theme,
                                ModalButtonRole::Secondary,
                                cx.listener(move |this, _, window, cx| {
                                    this.handle_modal_discard(&key_discard, window, cx);
                                }),
                            ))
                            .child(render_modal_button(
                                "btn_modal_save",
                                save_label,
                                theme,
                                ModalButtonRole::Primary,
                                cx.listener(move |this, _, window, cx| {
                                    this.handle_modal_save(&key_save, window, cx);
                                }),
                            )),
                    ),
            )
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_find();
        if self.state.doc().path.as_ref().is_some_and(|p| !zmark_core::file_ops::is_markdown(p)) {
            self.state.view_mode = ViewMode::Source;
        }
        if self.lightbox.is_open || self.lightbox_sharp.is_some() {
            self.lightbox.update_viewport(window.viewport_size());
            self.schedule_lightbox_sharp(window, cx);
        }
        if self.syntax_warm_pending {
            self.syntax_warm_pending = false;
            self.start_warm_coordination_task(cx);
        }
        if !self.preview_scroll_activation_observed {
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.cancel_source_composition();
                    this.end_source_drag();
                    this.end_preview_scroll_drag();
                    this.cancel_synced_scroll();
                    this.editor_scrollbar_dragging.set(false);
                }
                this.state.theme.native_selection = None;
                cx.notify();
            })
            .detach();
            self.preview_scroll_activation_observed = true;
        }
        {
            let mut images = self.local_images.lock().unwrap();
            images.begin_frame(self.state.active.clone(), self.state.doc().path.clone());
            for image in images.retired.drain(..) {
                if !self.lightbox_local_image.as_ref().is_some_and(|active| Arc::ptr_eq(active, &image)) {
                    image.release(window, cx);
                }
            }
        }
        crate::views::preview::release_evicted_diagram_images(window, cx);
        let image_view = cx.entity().clone();
        cx.defer(move |cx| image_view.update(cx, |this, cx| this.process_local_images(cx)));
        let is_sys_dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        self.state.update_appearance(is_sys_dark);
        self.state.theme.refresh_native_selection(window.is_window_active());
        crate::theme::sync_component_theme(&self.state.theme, cx);
        cx.set_reduce_motion(self.state.preferences.reduce_motion);
        let locale = self.state.preferences.locale.resolve();
        if let Some((_, tag, _)) = zmark_core::i18n::SupportedLocale::all().iter().find(|(candidate, _, _)| *candidate == locale) {
            let tag = if *tag == "pt" { "pt-BR" } else { *tag };
            if &*gpui_component::locale() != tag { gpui_component::set_locale(tag); }
        }
        self.ensure_inputs(window, cx);
        // A newly mounted Input may not yet have a focus path when it takes focus.
        if !self.source_accepts_input(window, cx) {
            self.cancel_source_composition();
            self.end_source_drag();
        }
        if self.source_focus_subscription.is_none() {
            self.source_focus_subscription = Some(cx.on_blur(&self.focus_handle, window, |this, _, cx| {
                this.cancel_source_composition();
                this.end_source_drag();
                cx.notify();
            }));
        }
        self.source_rows.lock().unwrap().clear();
        self.workspace_width = f32::from(window.viewport_size().width);
        self.window_handle = Some(window.window_handle());

        // Size-table virtual list: rebuild estimates when AST/layout inputs change,
        // absorb real prepaint heights, then keep split panes in sync without
        // relying on partially measured ListState offsets.
        let parsed = self.state.doc().parsed.clone();
        let rem = f32::from(window.rem_size());
        let font_size = self.state.preferences.font_size;
        let line_height = self.state.preferences.line_height;
        let max_width = self.state.preferences.max_column_width;
        if self.editor_scrollbar_dirty.replace(false) {
            self.cancel_pending_preview_seek();
            self.preview_list.cancel_scrollbar_drag();
            if self.state.view_mode == ViewMode::Split
                && !self.state.doc().is_parse_stale()
                && !parsed.anchors.is_empty()
            {
                let (current_y, max_y, _) = self.editor_scroll_metrics();
                if max_y > 0.5 {
                    let preview_target = self.preview_offset_for_editor_y(current_y, max_y, window);
                    self.drive_synced_scroll(ScrollOrigin::Preview, preview_target, false, window, cx);
                }
            }
        }
        let preview_scrollbar_moved = self.preview_list.take_scrollbar_scrolled();
        if preview_scrollbar_moved {
            self.cancel_pending_preview_seek();
            self.last_scroll_origin = Some((ScrollOrigin::Preview, std::time::Instant::now()));
        }
        let viewport_size = self.preview_list.viewport_bounds().size;
        if self
            .preview_viewport_size
            .is_some_and(|old| old != viewport_size)
        {
            self.cancel_pending_preview_seek();
            self.preview_list.cancel_scrollbar_drag();
            self.cancel_synced_scroll();
        }
        self.preview_viewport_size = Some(viewport_size);
        let drag_ratio = self.preview_list.active_drag_ratio();
        let (visible_range, measured_changed, layout_changed, range_changed) = if self.state.view_mode != ViewMode::Source {
            let measured_changed = self.preview_list.apply_measured(drag_ratio);
            let layout_changed = self.preview_list.ensure_layout(&parsed, max_width, font_size, line_height, rem);
            let (range, range_changed) = self.preview_list.visible_range(32.0);
            (range, measured_changed, layout_changed, range_changed)
        } else {
            (0..0, false, false, false)
        };
        let any_scrollbar_dragging =
            self.editor_scrollbar_dragging.get() || self.preview_list.is_scrollbar_dragging();
        if !any_scrollbar_dragging {
            let shared_height = if self.state.view_mode == ViewMode::Split {
                let (_, editor_max, _) = self.editor_scroll_metrics();
                let editor_vp =
                    f32::from(self.editor_scroll_handle.0.borrow().base_handle.bounds().size.height);
                let preview_max = self.preview_list.max_scroll();
                let preview_vp = f32::from(self.preview_list.viewport_bounds().size.height);
                if editor_max > 0.5 && preview_max > 0.5 && editor_vp > 1.0 && preview_vp > 1.0 {
                    let editor_total = editor_vp + editor_max;
                    let preview_total = self.preview_list.total_height();
                    Some(editor_total.max(preview_total))
                } else {
                    None
                }
            } else {
                None
            };
            if self.split_scrollbar_content_height.get() != shared_height {
                self.split_scrollbar_content_height.set(shared_height);
                self.preview_list.set_shared_content_height(shared_height);
                cx.notify();
            }
        }
        // Measurement/layout rewrites are programmatic: record the new offset so
        // the reconciliation below cannot mistake them for user scrolling and
        // echo them into the editor.
        if (measured_changed || layout_changed) && !preview_scrollbar_moved {
            self.last_preview_sync_offset = self.preview_list.offset();
        }
        if measured_changed || layout_changed || range_changed {
            cx.notify();
        }
        let preview_offset = self.preview_list.offset();
        if (preview_offset - self.last_preview_sync_offset).abs() > 0.5 {
            let recent_preview = matches!(
                self.last_scroll_origin,
                Some((ScrollOrigin::Preview, time))
                    if time.elapsed() < std::time::Duration::from_millis(100)
            );
            if recent_preview
                && self.state.view_mode == ViewMode::Split
                && !self.state.doc().is_parse_stale()
                && !parsed.anchors.is_empty()
            {
                // Native preview scrolling still needs one post-scroll echo; it is
                // bounded to preview-origin gestures so editor-origin and
                // measurement-driven offsets never move the editor.
                self.sync_preview_scroll(window, cx);
                self.last_preview_sync_offset = self.preview_list.offset();
            } else {
                self.last_preview_sync_offset = preview_offset;
            }
        }
        let is_fullscreen = window.is_fullscreen();

        let current_is_dark = self.state.theme.is_dark;
        if self.lightbox.is_open
            && self.lightbox.is_dark.is_some()
            && self.lightbox.is_dark != Some(current_is_dark)
        {
            if let Some(code_hash) = self.lightbox.code_hash {
                let themed = crate::views::preview::themed_diagram_path(code_hash, current_is_dark, window, cx)
                    .or_else(|| self.lightbox.svg_path.as_ref().and_then(|old| {
                        let path = resolve_lightbox_svg_path(old, code_hash, current_is_dark);
                        (path != *old && path.is_file()).then_some(path)
                    }));
                if let Some(svg_path) = themed {
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
        }

        let is_focused = self.focus_handle.is_focused(window);
        if is_focused {
            let pane = if self.preview_focused {
                PaneKind::Preview
            } else {
                PaneKind::Source
            };
            self.current_reading_pane = Some(pane);
            self.last_reading_pane = Some(pane);
        } else {
            self.current_reading_pane = None;
        }

        let visual_lines = if self.state.view_mode != ViewMode::Preview {
            self.compute_visual_lines(window)
        } else {
            self.cached_visual_lines.clone()
        };
        let editor_failures = self.collect_visible_editor_failures();
        let preview_failures = self.collect_visible_preview_failures();

        if self.is_preview_selecting && !self.focus_handle.is_focused(window) {
            self.is_preview_selecting = false;
            self.preview_autoscroll_task = None;
            self.preview_mouse_window_pos = None;
        }

        if !self.state.doc().is_parse_stale() {
            if self.source_code_blocks_rev != self.state.editor.revision
                || self.source_code_blocks.is_empty()
            {
                self.source_code_blocks = self.state.doc().parsed.code_blocks.clone();
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

        let folder_targets = self.state.workspace_roots.iter().flat_map(|root| {
            fn paths(nodes: &[crate::state::FileTreeNode], out: &mut Vec<(PathBuf, bool)>) {
                for node in nodes.iter().filter(|node| node.is_dir) { out.push((node.path.clone(), node.is_expanded)); paths(&node.children, out); }
            }
            let mut out = vec![(root.path.clone(), root.is_expanded)];
            paths(&root.tree, &mut out);
            out
        }).collect::<HashMap<_, _>>();
        self.folder_transitions.retain(|path, _| folder_targets.contains_key(path));
        let mut folder_values = HashMap::new();
        for (path, expanded) in folder_targets {
            let transition = self.folder_transitions.entry(path.clone()).or_default();
            let target = if expanded { 1.0 } else { 0.0 };
            let value = transition.sample(target, gpui_base::motion::Transition::new(std::time::Duration::from_millis(600)), window, cx);
            folder_values.insert(path, value.value);
        }
        if !self.sidebar_selection.lock().unwrap().initialized {
            if let Some(key) = self.state.active.clone() {
                let mut selection = self.sidebar_selection.lock().unwrap();
                selection.initialized = true; selection.focus = Some(key.clone()); selection.selected.insert(key);
            }
        }

        let is_dirty = self.state.is_dirty();
        (self.dirty_pulse_generation, self.was_dirty) = crate::views::motion::advance_dirty_generation(
            self.was_dirty,
            is_dirty,
            self.dirty_pulse_generation,
        );
        let dirty_generation = self.dirty_pulse_generation;
        let reduce_motion = self.state.preferences.reduce_motion;
        let export_notice_closing = self.export_notice_closing;
        let export_notice_generation = self.export_notice_generation;
        let theme = &self.state.theme;
        let strings = self.state.strings;
        let export_view = cx.entity().downgrade();
        let view_mode = self.state.view_mode;
        let has_conflict = self.state.has_conflict();
        let parse_paused = self.parse_paused;

        let view_click = cx.entity().clone();
        let on_click_line = Arc::new(
            move |vis_idx: usize, vis_col: usize, window: &mut Window, cx: &mut App| {
                view_click.update(cx, |this, cx| {
                    this.on_editor_click_line(vis_idx, vis_col, window, cx);
                    cx.notify();
                });
            },
        );

        let view_drawer = cx.entity().clone();
        let on_select_file = Arc::new(
            move |path: &std::path::PathBuf, window: &mut Window, cx: &mut App| {
                view_drawer.update(cx, |this, cx| {
                                    match this.state.load_file(path.clone()) {
                                        Ok(()) => this.on_document_swapped(Some(window), cx),
                                        Err(error) => this.error_banner = Some(format!("打开失败：{error}")),
                                    }
                                    cx.notify();
                });
            },
        );

        let view_rename = cx.entity().clone();
        let on_rename_file = Arc::new(
            move |path: &std::path::PathBuf, window: &mut Window, cx: &mut App| {
                view_rename.update(cx, |this, cx| {
                    this.begin_rename_target(crate::state::DocKey::Path(path.clone()), true, window, cx);
                });
            },
        );

        let view_directory = cx.entity().clone();
        let on_select_directory = Arc::new(move |path: &PathBuf, rename: bool, window: &mut Window, cx: &mut App| {
            view_directory.update(cx, |this, cx| {
                this.selected_directory = Some(path.clone());
                if rename {
                    this.begin_rename_target(crate::state::DocKey::Path(path.clone()), true, window, cx);
                } else if let Some(root) = this.state.workspace_roots.iter().position(|root| path.starts_with(&root.path)) {
                    this.toggle_folder(root, path, cx);
                }
                cx.notify();
            });
        });
        let view_cancel_rename = cx.entity().clone();
        let on_cancel_rename = Arc::new(move |_window: &mut Window, cx: &mut App| {
            view_cancel_rename.update(cx, |this, cx| this.finish_rename(false, cx));
        });

        let view_doc_switch = cx.entity().clone();
        let on_select_doc = Arc::new(
            move |key: &crate::state::DocKey, rename: bool, window: &mut Window, cx: &mut App| {
                view_doc_switch.update(cx, |this, cx| {
                    if rename {
                        this.begin_rename_target(key.clone(), true, window, cx);
                    } else {
                        this.state.switch_document(key);
                        this.on_document_swapped(Some(window), cx);
                    }
                    cx.notify();
                });
            },
        );

        let view_doc_close = cx.entity().clone();
        let on_close_doc = Arc::new(
            move |key: &crate::state::DocKey, window: &mut Window, cx: &mut App| {
                view_doc_close.update(cx, |this, cx| {
                    if let crate::state::DocKey::Path(path) = key {
                        this.trigger_trash_file(path.clone(), window, cx);
                    } else {
                        this.trigger_close_doc(key, Some(window), cx);
                    }
                });
            },
        );

        let view_draft = cx.entity().clone();
        let on_new_draft = Arc::new(move |window: &mut Window, cx: &mut App| {
            view_draft.update(cx, |this, cx| {
                this.trigger_draft(Some(window), cx);
            });
        });

        let view_tab = cx.entity().clone();
        let on_select_tab = Arc::new(
            move |tab: crate::state::DrawerTab, _window: &mut Window, cx: &mut App| {
                view_tab.update(cx, |this, cx| {
                    this.state.set_drawer_tab(tab);
                    cx.notify();
                });
            },
        );

        let view_heading = cx.entity().clone();
        let on_select_heading =
            Arc::new(move |block_idx: usize, window: &mut Window, cx: &mut App| {
                view_heading.update(cx, |this, cx| {
                    this.cancel_pending_preview_seek();
                    this.end_preview_scroll_drag();
                    this.cancel_synced_scroll();
                    let target = this
                        .preview_list
                        .offset_for_block(block_idx, 0.0)
                        .clamp(0.0, this.preview_list.max_scroll());
                    this.animate_preview_scroll_to(target, window, cx);
                });
            });

        let view_folder = cx.entity().clone();
        let on_open_folder = Arc::new(move |_window: &mut Window, cx: &mut App| {
            let rx = cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: true,
                multiple: false,
                prompt: None,
            });
            view_folder.update(cx, |_this, cx| {
                cx.spawn(async move |this, app| {
                    if let Ok(Ok(Some(mut paths))) = rx.await {
                        if let Some(path) = paths.pop() {
                            let _ = this.update(app, |this, cx| {
                                let path = path.canonicalize().unwrap_or(path);
                                if path.is_file() {
                                    this.restore_sidebar_path(&path, cx);
                                    match this.state.load_file(path) {
                                        Ok(()) => this.on_document_swapped(None, cx),
                                        Err(e) => this.error_banner = Some(e.to_string()),
                                    }
                                    cx.notify();
                                    return;
                                }
                                match this.open_or_add_workspace_dir(&path, cx) {
                                    Ok(()) => {
                                        this.error_banner = None;
                                        this.state.clear_error();
                                    }
                                    Err(msg) => {
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
        let on_toggle_root =
            Arc::new(move |root_idx: usize, _window: &mut Window, cx: &mut App| {
                view_toggle_root.update(cx, |this, cx| {
                    if root_idx < this.state.workspace_roots.len() {
                        let path = this.state.workspace_roots[root_idx].path.clone();
                        this.toggle_folder(root_idx, &path, cx);
                    }
                });
            });

        let view_remove_root = cx.entity().clone();
        let on_remove_root =
            Arc::new(move |root_idx: usize, _window: &mut Window, cx: &mut App| {
                view_remove_root.update(cx, |this, cx| {
                    this.state.request_remove_workspace_root(root_idx);
                    cx.notify();
                });
            });

        let view_refresh_root = cx.entity().clone();
        let on_refresh_root =
            Arc::new(move |root_idx: usize, _window: &mut Window, cx: &mut App| {
                view_refresh_root.update(cx, |this, cx| {
                    this.state.refresh_root(root_idx);
                    cx.notify();
                });
            });

        let view_tree = cx.entity().clone();
        let on_toggle_folder = Arc::new(
            move |root_idx: usize,
                  path: &std::path::PathBuf,
                  _window: &mut Window,
                  cx: &mut App| {
                view_tree.update(cx, |this, cx| {
                    this.toggle_folder(root_idx, path, cx);
                });
            },
        );

        let view_new_file = cx.entity().clone();
        let on_new_file_in_dir = Arc::new(
            move |dir: &std::path::PathBuf, window: &mut Window, cx: &mut App| {
                view_new_file.update(cx, |this, cx| {
                    match this.state.create_file_in_dir(dir) {
                        Ok(_) => {
                            this.on_document_swapped(Some(window), cx);
                        }
                        Err(e) => {
                            this.state
                                .set_error(format!("Failed to create file: {}", e));
                        }
                    }
                    cx.notify();
                });
            },
        );

        let loose_docs = self.state.loose_docs();

        let view_lb = cx.entity().clone();
        let on_open_lightbox = Arc::new(
            move |svg_path: std::path::PathBuf, hash: u64, natural_size: Option<(f32, f32)>, window: &mut Window, cx: &mut App| {
                view_lb.update(cx, |this, cx| {
                    if hash == crate::views::preview::LOCAL_IMAGE_LIGHTBOX_HASH {
                        this.open_image_lightbox(svg_path, window, cx);
                        if let Some(size) = natural_size {
                            this.lightbox.natural_size = size;
                        }
                    } else {
                        this.open_lightbox(svg_path, hash, window, cx);
                    }
                    cx.notify();
                });
            },
        );

        let show_drawer = self.state.drawer_open || self.drawer_closing;
        let is_drawer_closing = self.drawer_closing;

        let root = div()
            .key_context("Workspace")
            .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                if this.sidebar_busy { return; }
                let selection = this.sidebar_selection.lock().unwrap();
                if selection.selected.len() <= 1 { return; }
                let batch = selection.batch_bounds.is_some_and(|bounds| bounds.contains(&event.position));
                let additive = if cfg!(target_os = "macos") { event.modifiers.platform } else { event.modifiers.control };
                let selecting_file = event.button == MouseButton::Left && (event.modifiers.shift || additive)
                    && selection.rows.iter().any(|(key, bounds)| matches!(key, crate::state::DocKey::Path(_)) && bounds.contains(&event.position));
                drop(selection);
                if !batch && !selecting_file { this.exit_sidebar_multiselect(cx); }
            }))
            .track_focus(&self.focus_handle)
            .can_drop(|item, _window, _cx| {
                item.downcast_ref::<ExternalPaths>().is_some_and(|paths| paths.paths().iter().any(|p|
                    p.is_dir() || zmark_core::file_ops::is_supported_text(p)))
            })
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                let mut errors = Vec::new();
                let mut opened_file = false;
                for path in paths.paths() {
                    let path = path.canonicalize().unwrap_or_else(|_| path.clone());
                    if path.is_dir() {
                        if let Err(msg) = this.open_or_add_workspace_dir(&path, cx) {
                            errors.push(format!("{}: {}", path.display(), msg));
                        }
                    } else {
                        match this.state.load_file(path.clone()) {
                            Ok(()) => {
                                this.restore_sidebar_path(&path, cx);
                                opened_file = true;
                            }
                            Err(error) => errors.push(format!("{}: {}", path.display(), error)),
                        }
                    }
                }
                if opened_file { this.on_document_swapped(Some(window), cx); }
                this.drawer_close_task = None;
                this.drawer_closing = false;
                this.state.drawer_open = true;
                this.state.drawer_tab = crate::state::DrawerTab::Files;
                this.error_banner = if errors.is_empty() { None } else { Some(errors.join("\n")) };
                cx.notify();
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                this.source_mouse_move(event, window, cx);
                if this.is_resizing_split {
                    if event.pressed_button == Some(MouseButton::Left) {
                        let w = this.workspace_width;
                        let x = f32::from(event.position.x);
                        let left_offset = if this.state.drawer_open { crate::views::editor::DRAWER_WIDTH } else { 0.0 };
                        let avail_w = (w - left_offset).max(100.0);
                        let offset_x = (x - left_offset).max(0.0);
                        this.split_ratio = (offset_x / avail_w).clamp(0.20, 0.80);
                    } else {
                        this.is_resizing_split = false;
                    }
                    cx.notify();
                }
                if this.is_preview_selecting {
                    if event.pressed_button == Some(MouseButton::Left) {
                        this.preview_mouse_window_pos = Some(event.position);
                        this.trigger_preview_autoscroll_if_needed(cx);
                    } else {
                        this.end_preview_selection();
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(MouseButton::Middle, cx.listener(|this, _, _, cx| {
                this.release_source_drag(MouseButton::Middle);
                cx.notify();
            }))
            .on_mouse_up_out(MouseButton::Middle, cx.listener(|this, _, _, cx| {
                this.release_source_drag(MouseButton::Middle);
                cx.notify();
            }))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.release_source_drag(MouseButton::Left);
                this.is_resizing_split = false;
                this.end_preview_selection();
                cx.notify();
            }))
            .capture_any_mouse_up(cx.listener(|this, event: &MouseUpEvent, _window, cx| {
                if event.button != MouseButton::Left { return; }
                this.release_source_drag(MouseButton::Left);
                if this.is_resizing_split {
                    this.is_resizing_split = false;
                }
                if this.is_preview_selecting {
                    this.end_preview_selection();
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
                                this.restore_sidebar_path(&path, cx);
                                match this.state.load_file(path) {
                                    Ok(()) => this.on_document_swapped(None, cx),
                                    Err(error) => this.error_banner = Some(error.to_string()),
                                }
                                cx.notify();
                            });
                        }
                    }
                })
                .detach();
            }))
            .on_action(cx.listener(|this, _: &CancelRename, window, cx| {
                window.focus(&this.focus_handle, cx);
                this.finish_rename(false, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveFile, _window, cx| {
                this.trigger_save(cx);
            }))
            .on_action(cx.listener(|this, _: &ExportHtml, window, cx| {
                this.trigger_export_html(window, cx);
            }))
            .on_action(cx.listener(|this, _: &crate::actions::FindInFile, window,cx| {
                this.open_find(window,cx);
            }))
            .on_action(cx.listener(|this, _: &crate::actions::CloseFind, window,cx| this.close_find(window,cx)))
            .on_action(cx.listener(|this, _: &crate::actions::FindNext, window,cx| this.step_find(false,window,cx)))
            .on_action(cx.listener(|this, _: &crate::actions::FindPrevious, window,cx| this.step_find(true,window,cx)))
            .on_action(cx.listener(|this, _: &crate::actions::UndoSidebarRemoval, _,cx| {
                if !this.sidebar_busy && !this.is_renaming && !this.lightbox.is_open
                    && this.state.pending_close.is_none() && this.state.pending_remove_root.is_none() {
                    if let Some(paths) = this.sidebar_undo.clone() { this.persist_sidebar_hidden(paths,false,cx); }
                }
            }))
            .on_action(cx.listener(|this, _: &crate::actions::SourceUndo, window, cx| this.source_command(SourceCommand::Undo, window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::SourceRedo, window, cx| this.source_command(SourceCommand::Redo, window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::SourceCopy, window, cx| this.source_command(SourceCommand::Copy, window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::SourceCut, window, cx| this.source_command(SourceCommand::Cut, window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::SourcePaste, window, cx| this.source_command(SourceCommand::Paste, window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::SourceSelectAll, window, cx| this.source_command(SourceCommand::SelectAll, window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::SourceDelete, window, cx| this.source_command(SourceCommand::Delete, window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::SourceBold, window, cx| this.source_command(SourceCommand::Bold, window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::SourceNumberedList, window, cx| this.source_command(SourceCommand::NumberedList, window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::SourceIndent, window, cx| this.source_command(SourceCommand::Indent, window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::SourceOutdent, window, cx| this.source_command(SourceCommand::Outdent, window, cx)))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.sidebar_busy { cx.stop_propagation(); return; }
                if this.lightbox.is_open {
                    if event.keystroke.key == "escape" {
                        this.close_lightbox(window, cx);
                        cx.notify();
                    }
                    // Skip hidden-source editing without swallowing native window shortcuts.
                    return;
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

                let mods = event.keystroke.modifiers;
                let key = event.keystroke.key.as_str();
                if key == "escape" && !this.is_renaming && this.sidebar_selection.lock().unwrap().selected.len() > 1 {
                    this.exit_sidebar_multiselect(cx);
                    cx.stop_propagation();
                    return;
                }
                if !this.is_renaming && (mods.secondary(cx) || mods.control) && !mods.alt && !mods.shift && key.eq_ignore_ascii_case("f") {
                    this.open_find(window,cx); cx.stop_propagation(); return;
                }
                if (mods.secondary(cx) || mods.control) && mods.alt && key.eq_ignore_ascii_case("z") && !this.is_renaming {
                    if let Some(paths) = this.sidebar_undo.clone() { this.persist_sidebar_hidden(paths,false,cx); }
                    cx.stop_propagation(); return;
                }
                if this.find.open {
                    if key == "escape" { this.close_find(window,cx); cx.stop_propagation(); return; }
                    if key == "f3" || ((mods.secondary(cx) || mods.control) && key.eq_ignore_ascii_case("g")) {
                        this.step_find(mods.shift,window,cx); cx.stop_propagation(); return;
                    }
                    if this.find.input.as_ref().is_some_and(|input| input.read(cx).focus_handle(cx).is_focused(window)) { return; }
                }
                this.search_focused = this.search_input.as_ref()
                    .is_some_and(|input| input.read(cx).focus_handle(cx).is_focused(window));
                let rename_focused = this.rename_input.as_ref()
                    .is_some_and(|input| input.read(cx).focus_handle(cx).is_focused(window));
                if this.search_focused || rename_focused {
                    // Native Input owns its IME, selection, clipboard and history actions.
                    return;
                }

                if this.is_renaming {
                    return;
                }

                if this.source_key_down(event, window, cx) {
                    cx.stop_propagation();
                    return;
                }
                let key = event.keystroke.key.as_str();
                let mods = event.keystroke.modifiers;
                if (this.preview_focused || this.state.view_mode == ViewMode::Preview)
                    && !mods.platform
                    && !mods.control
                    && !mods.alt
                {
                    if let Some(next) = scrollbar_key_step(
                        key,
                        this.preview_list.scroll_ratio(),
                        this.preview_list.max_scroll(),
                    ) {
                        this.preview_scroll_to_ratio(next, window, cx);
                        cx.stop_propagation();
                        return;
                    }
                }

                if mods.secondary(cx) {
                    match key {
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
                                            this.restore_sidebar_path(&path, cx);
                                            match this.state.load_file(path) {
                                                Ok(()) => this.on_document_swapped(None, cx),
                                                Err(error) => this.error_banner = Some(format!("打开失败：{error}")),
                                            }
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
                        "b" if mods.shift => {
                            this.toggle_drawer(cx);
                        }
                        "1" => {
                            this.set_view_mode(ViewMode::Source, cx);
                        }
                        "2" => {
                            this.set_view_mode(ViewMode::Split, cx);
                        }
                        "c" | "C" => {
                            let preview = this.state.view_mode == ViewMode::Preview || (this.state.view_mode == ViewMode::Split && this.preview_focused);
                            if preview {
                                if let Some(selection) = this.preview_selection {
                                    let text = crate::views::preview::serialize_preview_selection(&this.state.doc().parsed, selection);
                                    if !text.is_empty() { cx.write_to_clipboard(ClipboardItem::new_string(text)); }
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

                // Printable text and composition are delivered once by ElementInputHandler.
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
                                        Arc::new(move |window: &mut Window, cx: &mut App| {
                                            if let Some(ref input) = si_clear {
                                                input.update(cx, |inp, cx| {
                                                    inp.set_value("", window, cx);
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
                                                let handle = input.read(cx).focus_handle(cx);
                                                window.focus(&handle, cx);
                                            }
                                            view.update(cx, |this, cx| {
                                                this.search_focused = true;
                                                cx.notify();
                                            });
                                        })
                                    };
                                    let row_view = cx.entity().clone();
                                    let on_row = Arc::new(move |key: &crate::state::DocKey, event: &MouseDownEvent, name: bool, window: &mut Window, cx: &mut App| {
                                        row_view.update(cx, |this,cx| this.sidebar_row_click(key,event,name,window,cx));
                                    });
                                    let trash_view = cx.entity().clone();
                                    let on_trash_directory = Arc::new(move |path: &PathBuf, window: &mut Window, cx: &mut App| {
                                        trash_view.update(cx, |this,cx| this.trash_sidebar_paths(vec![path.clone()],window,cx));
                                    });
                                    let remove_view = cx.entity().clone();
                                    let on_remove_path = Arc::new(move |path: &PathBuf, _: &mut Window, cx: &mut App| {
                                        remove_view.update(cx, |this,cx| this.persist_sidebar_hidden(vec![path.clone()],true,cx));
                                    });
                                    let undo_view = cx.entity().clone();
                                    let on_undo = Arc::new(move |_: &mut Window, cx: &mut App| {
                                        undo_view.update(cx, |this,cx| {
                                            if let Some(paths) = this.sidebar_undo.clone() { this.persist_sidebar_hidden(paths,false,cx); }
                                        });
                                    });
                                    let search_params = crate::views::drawer::DrawerSearchParams {
                                        search_query: self.state.search_query.clone(),
                                        has_search_matches: self.state.has_search_matches(),
                                        search_in_progress: self.state.search_in_progress,
                                        search_truncated: self.state.search_truncated,
                                        on_clear_search: Some(on_clear_search),
                                        on_focus_search: Some(on_focus_search),
                                        search_input: self.search_input.clone(),
                                        focused: self.search_focused,
                                        selected_directory: self.selected_directory.clone(),
                                        selection: self.sidebar_selection.clone(),
                                        hidden_paths: self.state.preferences.sidebar_hidden_paths.clone(),
                                        reduce_motion,
                                        on_row: Some(on_row),
                                        on_trash_directory: Some(on_trash_directory),
                                        undo_available: self.sidebar_undo.as_ref().is_some_and(|paths| !paths.is_empty()),
                                        on_undo: Some(on_undo),
                                        on_remove_path: Some(on_remove_path),
                                        folder_values: folder_values.clone(),
                                        rename_target: if self.is_renaming && self.rename_in_sidebar { self.rename_target.clone() } else { None },
                                        rename_input: self.rename_input.clone().filter(|_| self.is_renaming && self.rename_in_sidebar),
                                        on_select_directory: Some(on_select_directory),
                                        on_cancel_rename: Some(on_cancel_rename),
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
                                        on_rename_file,
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
                    .child(self.render_canvas_header(
                        window,
                        theme,
                        is_fullscreen,
                        is_dirty,
                        dirty_generation,
                        reduce_motion,
                        cx,
                    ))
                    .when(self.find.open, |el| el.child(self.render_find(window, cx)))
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
                    // HTML export success notice (auto-dismisses after 3s)
                    .when_some(self.export_notice.clone(), |el, (name, path)| {
                        let reveal_path = path.clone();
                        let close_view = export_view.clone();
                        el.child(crate::views::motion::animated(
                            div()
                                .id("export_success_banner")
                                .h(px(32.))
                                .px_4()
                                .bg(rgba(0x22c55e1a))
                                .border_b_1()
                                .border_color(rgba(0x22c55e33))
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
                                                .path("icons/check.svg")
                                                .size(px(14.))
                                                .text_color(rgb(0x22c55e)),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(rgb(0x16a34a))
                                                .child(format!("{} {}", strings.export_done, name)),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .id("btn_reveal_export")
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .cursor_pointer()
                                                .text_xs()
                                                .text_color(rgb(0x16a34a))
                                                .hover(|s| s.bg(rgba(0x22c55e22)))
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    move |_, _, cx| {
                                                        cx.reveal_path(&reveal_path);
                                                    },
                                                )
                                                .child(strings.reveal_in_finder),
                                        )
                                        .child(
                                            div()
                                                .id("btn_close_export_banner")
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .cursor_pointer()
                                                .text_xs()
                                                .text_color(rgb(0x16a34a))
                                                .hover(|s| s.bg(rgba(0x22c55e22)))
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    move |_, _, cx| {
                                                        let _ = close_view.update(cx, |this, cx| {
                                                            this.begin_export_notice_exit(cx);
                                                        });
                                                    },
                                                )
                                                .child("×"),
                                        ),
                                ),
                            reduce_motion,
                            format!("export_banner_{export_notice_generation}_{export_notice_closing}"),
                            160,
                            move |el, delta| {
                                let value = if export_notice_closing { 1.0 - delta } else { delta };
                                el.h(px(32. * value)).opacity(value).overflow_hidden()
                            },
                        ))
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
                                                        match this.state.reload_active_document() {
                                                            Ok(()) => this.on_document_swapped(Some(window), cx),
                                                            Err(error) => this.error_banner = Some(format!("重载失败：{error}")),
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
                                        if this.lightbox.is_open { return; }
                                        if is_selecting || sel.is_none() {
                                            this.preview_selection = sel;
                                        }
                                        this.is_preview_selecting = is_selecting;
                                        if is_selecting {
                                            this.preview_focused = true;
                                            this.preview_autoscroll_accum = 0.0;
                                            window.focus(&this.focus_handle, cx);
                                            this.preview_mouse_window_pos = Some(window.mouse_position());
                                            this.trigger_preview_autoscroll_if_needed(cx);
                                        } else {
                                            this.end_preview_selection();
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
                                            this.step_preview_autoscroll(velocity, mouse_pos, bounds, window.text_system(), cx);
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
                                                let Some(target) = zmark_core::ast::image_source_target_range(&source, range, &url) else {
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

                                let wheel_view = cx.entity().clone();
                                let on_wheel: crate::views::preview::OnPreviewWheelCallback =
                                    Arc::new(move |event, window, app| {
                                        wheel_view.update(app, |this, cx| {
                                            this.handle_preview_scroll_wheel(event, window, cx);
                                        });
                                    });

                                let preview_callbacks = crate::views::preview::PreviewCallbacks {
                                    on_copy: Some(on_copy.clone()),
                                    on_open_lightbox: Some(on_open_lightbox.clone()),
                                    on_selection_change: Some(on_selection_change),
                                    on_autoscroll: Some(on_autoscroll),
                                    on_image_recovery: Some(on_image_recovery),
                                    on_wheel: Some(on_wheel),
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
                                        match view_mode {
                                            ViewMode::Source => {
                                                let view_bounds = cx.entity().clone();
                                                div()
                                                    .size_full()
                                                    .flex()
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .h_full()
                                                            .relative()
                                                            .min_w(px(0.))
                                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _event: &MouseDownEvent, window, cx| {
                                                                window.focus(&this.focus_handle, cx);
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
                                                                self.source_display_lines(),
                                                                self.state.editor.char_count,
                                                                visual_lines.clone(),
                                                                self.document_layout_cache.line_offsets.clone(),
                                                                self.syntax_cache.clone(),
                                                                self.source_code_blocks.clone(),
                                                                !self.state.doc().is_parse_stale() && self.source_composition.is_none(),
                                                                self.source_display_cursor(),
                                                                theme,
                                                                strings,
                                                                self.state.preferences.font_size,
                                                                self.state.preferences.line_height,
                                                                self.state.preferences.show_line_numbers,
                                                                &self.editor_scroll_handle,
                                                                gutter_width,
                                                                on_click_line.clone(),
                                                                cx.entity().clone(),
                                                                Arc::new(self.source_display_selection_ranges()),
                                                                self.source_marked_range(),
                                                                &editor_failures,
                                                            ))
                                                            .child(self.render_editor_scrollbar()),
                                                    )
                                                    .into_any_element()
                                            }
                                            ViewMode::Preview => {
                                                let view_bounds = cx.entity().clone();
                                                div().size_full().flex()
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
                                                        visible_range.clone(),
                                                        copied_code_hash,
                                                        preview_callbacks.clone(),
                                                        self.preview_selection,
                                                        self.is_preview_selecting,
                                                        self.rendered_regions.clone(),
                                                        self.syntax_cache.clone(),
                                                        &preview_failures,
                                                        self.local_images.clone(),
                                                        reduce_motion,
                                                    ))
                                                    .child(self.render_preview_scrollbar()))
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
                                                        .overflow_hidden()
                                                        .on_scroll(cx.listener(|this, event: &ScrollEvent, window, cx| {
                                                            this.handle_editor_scroll_wheel(event, window, cx);
                                                        }))
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .min_w(px(0.))
                                                                .min_h(px(0.))
                                                                .h_full()
                                                                .flex()
                                                                .flex_col()
                                                                .relative()
                                                                .overflow_hidden()
                                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _event: &MouseDownEvent, window, cx| {
                                                                    window.focus(&this.focus_handle, cx);
                                                                    this.search_focused = false;
                                                                    cx.notify();
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
                                                                    self.source_display_lines(),
                                                                    self.state.editor.char_count,
                                                                    visual_lines.clone(),
                                                                    self.document_layout_cache.line_offsets.clone(),
                                                                    self.syntax_cache.clone(),
                                                                    self.source_code_blocks.clone(),
                                                                    !self.state.doc().is_parse_stale() && self.source_composition.is_none(),
                                                                    self.source_display_cursor(),
                                                                    theme,
                                                                    strings,
                                                                    self.state.preferences.font_size,
                                                                    self.state.preferences.line_height,
                                                                    self.state.preferences.show_line_numbers,
                                                                    &self.editor_scroll_handle,
                                                                    gutter_width,
                                                                    on_click_line,
                                                                    cx.entity().clone(),
                                                                    Arc::new(self.source_display_selection_ranges()),
                                                                    self.source_marked_range(),
                                                                    &editor_failures,
                                                                ))
                                                                .child(self.render_editor_scrollbar()),
                                                        )
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
                                                                    visible_range.clone(),
                                                                    copied_code_hash,
                                                                    preview_callbacks,
                                                                    self.preview_selection,
                                                                    self.is_preview_selecting,
                                                                    self.rendered_regions.clone(),
                                                                    self.syntax_cache.clone(),
                                                                    &preview_failures,
                                                                    self.local_images.clone(),
                                                                    reduce_motion,
                                                                ))
                                                                .child(self.render_preview_scrollbar()))
                                                        }),
                                                )
                                                .into_any_element(),
                                        }
                                    })
                            })
                    )
                    .when(self.lightbox.is_open, |root| {
                        let sharp = self.lightbox_sharp.as_ref().map(|(image, _)| image.clone())
                            .or_else(|| self.lightbox_local_image.as_ref().and_then(|image| image.render.clone()));
                        root.child(render_lightbox(&mut self.lightbox, sharp, window.viewport_size(), theme, strings, reduce_motion, cx))
                    })
                    .when(self.state.pending_remove_root.is_some(), |root| {
                        root.child(self.render_remove_root_modal(theme, cx))
                    })
                    .when(self.state.pending_close.is_some(), |root| {
                        root.child(self.render_close_modal(theme, cx))
                    });

        self.complete_local_image_decode(window, cx);
        let pinned_diagram = self.lightbox.svg_path.clone();
        let release_hidden = std::mem::take(&mut self.release_hidden_diagrams)
            || self.state.view_mode == ViewMode::Source;
        window.on_next_frame(move |window, cx| {
            let evicted = crate::views::preview::finish_diagram_frame(pinned_diagram.as_deref());
            let released = release_hidden && crate::views::preview::release_hidden_diagram_images(
                pinned_diagram.as_deref(), window, cx,
            );
            if evicted || released {
                window.refresh();
            }
        });
        if crate::views::preview::diagram_cache_has_pending_release() {
            // The current frame may have evicted diagrams while building the
            // preview. Re-render once so their textures can be released before
            // the next interaction.
            window.request_animation_frame();
        }
        root
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
        base.bg(theme.control_selection_bg()).child(icon)
    } else {
        base.hover(|s| s.bg(theme.hover_bg())).child(icon)
    }
}

/// Resolves the SVG path for the lightbox, ensuring that the `is_dark` variant
/// matches the current theme active in the preview pane (ADR 0036).
pub fn resolve_lightbox_svg_path(
    svg_path: &std::path::Path,
    code_hash: u64,
    is_dark: bool,
) -> std::path::PathBuf {
    if let Some(path) = crate::views::preview::cached_diagram_path(code_hash, is_dark) {
        if path.is_file() { return path; }
    }
    let expected_variant = if is_dark { "dark" } else { "light" };
    let filename = format!("{:016x}_{}.svg", code_hash, expected_variant);

    let preview_cache_dir = std::env::temp_dir().join("zmark").join("mermaid");
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
            ViewMode::Source | ViewMode::Split => editor_text_start - 8.0,
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

/// Default directory and file name for the HTML export save panel.
///
/// Saved documents export next to themselves; drafts export to the user's home
/// directory using the localized untitled name. Path separators are stripped so
/// a translated name can never escape the chosen directory.
pub fn export_suggested_path(
    doc_path: Option<&Path>,
    untitled_file: &str,
    home: &Path,
) -> (PathBuf, String) {
    match doc_path {
        Some(path) => {
            let dir = path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| home.to_path_buf());
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(untitled_file);
            (dir, format!("{stem}.html"))
        }
        None => {
            let stem = untitled_file.strip_suffix(".md").unwrap_or(untitled_file);
            let stem = stem.replace(['/', '\\'], "-");
            (home.to_path_buf(), format!("{stem}.html"))
        }
    }
}

/// Predicts GPUI's content offset after a wheel delta, before native scrolling runs.
fn projected_scroll_offset(current: f32, wheel_delta_y: f32, max: f32) -> f32 {
    (current - wheel_delta_y).clamp(0.0, max.max(0.0))
}

/// Outline-jump duration: short hops stay snappy, long jumps cap at 600ms.
pub fn outline_scroll_duration_ms(distance_px: f32) -> u64 {
    let d = distance_px.abs();
    if d < 1.0 {
        0
    } else {
        (240.0 + d * 0.15).min(600.0) as u64
    }
}

pub fn outline_scroll_ease(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(4)
}

#[cfg(test)]
mod scroll_tests {
    use super::{outline_scroll_duration_ms, outline_scroll_ease, projected_scroll_offset};

    #[test]
    fn wheel_projection_matches_scroll_direction_and_clamps_at_edges() {
        assert_eq!(projected_scroll_offset(50.0, -20.0, 100.0), 70.0);
        assert_eq!(projected_scroll_offset(50.0, 20.0, 100.0), 30.0);
        assert_eq!(projected_scroll_offset(0.0, 20.0, 100.0), 0.0);
        assert_eq!(projected_scroll_offset(100.0, -20.0, 100.0), 100.0);
        assert_eq!(projected_scroll_offset(50.0, 0.0, 100.0), 50.0);
    }

    #[test]
    fn outline_duration_scales_with_distance_and_is_capped() {
        assert_eq!(outline_scroll_duration_ms(0.4), 0);
        assert_eq!(outline_scroll_duration_ms(100.0), 255);
        assert_eq!(outline_scroll_duration_ms(-100000.0), 600);
    }

    #[test]
    fn outline_ease_runs_from_zero_to_one_and_decelerates() {
        assert_eq!(outline_scroll_ease(0.0), 0.0);
        assert_eq!(outline_scroll_ease(1.0), 1.0);
        assert!(outline_scroll_ease(0.5) > 0.9);
    }
}

/// Fraction (0..1) of the target preview block to scroll to, combining the progress
/// inside the source line with the position of a wrapped sub-line.
pub fn calculate_subline_scroll_fraction(
    inner_progress: f32,
    subline_idx: usize,
    total_sublines: usize,
) -> f32 {
    let total = total_sublines.max(1);
    let subline_fraction = (subline_idx as f32) / (total as f32);
    (inner_progress + subline_fraction).clamp(0.0, 1.0)
}

/// Resolves background and text colors for view mode switcher tabs.
///
/// Active tabs receive `theme.control_selection_bg()` and `theme.selection_text()`
/// strictly distinct from `theme.surface`.
/// Inactive tabs receive transparent background and `theme.text_muted`.
pub fn mode_tab_colors(
    active: bool,
    theme: &crate::theme::ThemeColors,
) -> (gpui::Hsla, gpui::Hsla) {
    if active {
        (theme.control_selection_bg(), theme.selection_text())
    } else {
        (gpui::transparent_black(), theme.text_muted)
    }
}

fn render_animated_icon_button<F>(
    id: &'static str,
    tooltip_text: impl Into<String>,
    theme: &crate::theme::ThemeColors,
    icon: impl IntoElement + 'static,
    reduce_motion: bool,
    press_generation: u64,
    handler: F,
) -> AnyElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let button = render_icon_button(id, tooltip_text, false, theme, icon, handler);
    crate::views::motion::animated(
        div().child(button),
        reduce_motion,
        format!("press_{id}_{press_generation}"),
        90,
        |el, delta| el.opacity(0.6 + 0.4 * delta),
    )
}

fn render_mode_icon_tab<F>(
    tab_id: &'static str,
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
        .id(tab_id)
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
        base.bg(bg_color).text_color(text_color).child(icon)
    } else {
        base.text_color(text_color)
            .hover(|s| s.bg(theme.hover_bg()).text_color(theme.text_primary))
            .child(icon)
    }
}

fn render_modal_button(
    id: &'static str,
    label: String,
    theme: &crate::theme::ThemeColors,
    role: ModalButtonRole,
    on_press: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let (bg, text, border, hover_bg) = match role {
        ModalButtonRole::Secondary => (
            theme.bg,
            theme.text_primary,
            theme.border,
            theme.hover_bg(),
        ),
        ModalButtonRole::Danger => (
            theme.conflict_bg,
            rgb(0xef4444).into(),
            theme.conflict_border,
            theme.conflict_border.opacity(0.35),
        ),
        ModalButtonRole::Primary => (
            theme.accent,
            rgb(0xffffff).into(),
            theme.accent,
            theme.accent.opacity(0.9),
        ),
    };
    div()
        .id(id)
        .min_w(px(72.))
        .h(px(32.))
        .px_3()
        .rounded_md()
        .cursor_pointer()
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .text_color(text)
        .flex()
        .items_center()
        .justify_center()
        .hover(move |style| style.bg(hover_bg))
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            cx.stop_propagation();
            on_press(event, window, cx);
        })
        .child(label)
}

#[derive(Clone, Copy)]
enum ModalButtonRole {
    Secondary,
    Danger,
    Primary,
}

/// Windows has no native title bar when `appears_transparent` is set, so the
/// canvas header is the single title bar. These client-drawn buttons use the
/// native non-client hit-test areas, which keeps minimise/maximise/close and
/// Snap behaviour identical to a system caption bar.
pub(crate) fn render_window_controls(theme: &crate::theme::ThemeColors) -> impl IntoElement {
    let color = theme.text_primary;
    let hover = theme.hover_bg();
    div()
        .id("windows_window_controls")
        .flex()
        .flex_shrink_0()
        .h_full()
        .items_center()
        .child(render_window_control_button(
            "btn_window_minimize",
            WindowControlArea::Min,
            render_window_minimize_icon(color),
            hover,
        ))
        .child(render_window_control_button(
            "btn_window_maximize",
            WindowControlArea::Max,
            render_window_maximize_icon(color),
            hover,
        ))
        .child(render_window_control_button(
            "btn_window_close",
            WindowControlArea::Close,
            render_window_close_icon(color),
            hover,
        ))
}

fn render_window_control_button(
    id: &'static str,
    area: WindowControlArea,
    icon: impl IntoElement + 'static,
    hover: Hsla,
) -> impl IntoElement {
    div()
        .id(id)
        .w(px(crate::platform::WINDOW_CONTROL_BUTTON_WIDTH))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|style| style.bg(hover))
        // `occlude` keeps the focusable root out of the hit test. Otherwise its
        // focus listener calls `prevent_default()`, and GPUI's Windows
        // non-client handler then swallows the click (no drag/min/max/close).
        .occlude()
        .window_control_area(area)
        .child(icon)
}

fn render_window_minimize_icon(color: Hsla) -> impl IntoElement {
    div().w(px(10.)).h(px(1.)).bg(color)
}

fn render_window_maximize_icon(color: Hsla) -> impl IntoElement {
    div()
        .w(px(10.))
        .h(px(10.))
        .border_1()
        .border_color(color)
}

fn render_window_close_icon(color: Hsla) -> impl IntoElement {
    svg()
        .path("icons/x.svg")
        .size(px(10.))
        .text_color(color)
}

// Standard Lucide SVG icons rendered natively by GPUI
fn render_sidebar_icon(color: Hsla) -> impl IntoElement {
    svg()
        .path("icons/panel-left.svg")
        .size(px(16.))
        .text_color(color)
}

fn render_source_mode_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/code.svg").size(px(16.)).text_color(color)
}

fn render_split_mode_icon(color: Hsla) -> impl IntoElement {
    svg()
        .path("icons/columns-2.svg")
        .size(px(16.))
        .text_color(color)
}

fn render_preview_mode_icon(color: Hsla) -> impl IntoElement {
    svg()
        .path("icons/book-open.svg")
        .size(px(16.))
        .text_color(color)
}

fn render_export_icon(color: Hsla) -> impl IntoElement {
    svg()
        .path("icons/download.svg")
        .size(px(16.))
        .text_color(color)
}

fn format_root_add_error(
    strings: &zmark_core::i18n::I18nStrings,
    err: crate::state::RootAddError,
) -> String {
    match err {
        crate::state::RootAddError::NotFound => strings.error_root_not_found.to_string(),
        crate::state::RootAddError::NotADirectory => strings.error_root_not_dir.to_string(),
        crate::state::RootAddError::AlreadyExists => strings.error_root_already_exists.to_string(),
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
            format!(
                "{}: {}: {}",
                strings.error_canonicalize_failed,
                p.display(),
                err
            )
        }
    }
}

fn scrollbar_key_step(key: &str, current_ratio: f32, max_scroll: f32) -> Option<f32> {
    let step = (40.0 / max_scroll.max(1.0)).min(1.0);
    match key {
        "up" | "left" => Some(current_ratio - step),
        "down" | "right" => Some(current_ratio + step),
        "pageup" => Some(current_ratio - (step * 8.0).max(0.1)),
        "pagedown" => Some(current_ratio + (step * 8.0).max(0.1)),
        "home" => Some(0.0),
        "end" => Some(1.0),
        _ => None,
    }
}

fn render_settings_icon(color: Hsla) -> impl IntoElement {
    svg()
        .path("icons/settings.svg")
        .size(px(16.))
        .text_color(color)
}
