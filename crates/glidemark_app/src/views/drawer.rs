use gpui::prelude::*;
use gpui::*;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use glidemark_core::ast::HeadingItem;
use glidemark_core::i18n::I18nStrings;

use crate::state::{root_label, DocKey, DrawerTab, FileTreeNode, OpenDoc, WorkspaceRoot};
use crate::theme::ThemeColors;

#[derive(Clone, Default)]
pub struct DrawerSearchParams {
    pub search_query: String,
    pub has_search_matches: bool,
    pub search_in_progress: bool,
    pub search_truncated: bool,
    pub on_clear_search: Option<Arc<dyn Fn(&mut Window, &mut App)>>,
    pub on_focus_search: Option<Arc<dyn Fn(&mut Window, &mut App)>>,
    pub search_input: Option<Entity<SearchInput>>,
}

/// Search input component providing full Chinese IME support via `EntityInputHandler`.
#[allow(dead_code)]
#[derive(Clone)]
pub struct SearchInput {
    pub focus_handle: FocusHandle,
    pub content: SharedString,
    pub placeholder: SharedString,
    pub selected_range: Range<usize>,
    pub selection_reversed: bool,
    pub marked_range: Option<Range<usize>>,
    pub last_layout: Option<ShapedLine>,
    pub last_bounds: Option<Bounds<Pixels>>,
    pub is_selecting: bool,
    pub on_change: Option<Arc<dyn Fn(&str, &mut Window, &mut Context<Self>) + 'static>>,
}

#[allow(dead_code)]
impl SearchInput {
    pub fn new(focus_handle: FocusHandle) -> Self {
        Self {
            focus_handle,
            content: "".into(),
            placeholder: "Search files...".into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            on_change: None,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn on_change(
        mut self,
        callback: impl Fn(&str, &mut Window, &mut Context<Self>) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(callback));
        self
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn set_text(&mut self, text: impl Into<SharedString>) {
        self.content = text.into();
        self.selected_range = self.content.len()..self.content.len();
        self.marked_range = None;
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.content = "".into();
        self.selected_range = 0..0;
        self.marked_range = None;
        cx.notify();
    }

    pub fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    pub fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    pub fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    pub fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .char_indices()
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    pub fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .char_indices()
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    pub fn move_to(&mut self, offset: usize) {
        let offset = offset.min(self.content.len());
        self.selected_range = offset..offset;
    }

    pub fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    pub fn select_to(&mut self, offset: usize) {
        let offset = offset.min(self.content.len());
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
    }

    pub fn handle_replace_text(&mut self, range_utf16: Option<Range<usize>>, new_text: &str) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range.clone());

        let start = range.start.min(self.content.len());
        let end = range.end.min(self.content.len()).max(start);

        self.content = format!("{}{}{}", &self.content[0..start], new_text, &self.content[end..]).into();
        self.selected_range = start + new_text.len()..start + new_text.len();
        self.marked_range = None;
    }

    pub fn handle_replace_and_mark_text(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range.clone());

        let start = range.start.min(self.content.len());
        let end = range.end.min(self.content.len()).max(start);

        self.content = format!("{}{}{}", &self.content[0..start], new_text, &self.content[end..]).into();
        if !new_text.is_empty() {
            self.marked_range = Some(start..start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .map(|new_range| new_range.start + start..new_range.end + start)
            .unwrap_or_else(|| start + new_text.len()..start + new_text.len());
    }

    pub fn handle_unmark_text(&mut self) {
        self.marked_range = None;
    }

    pub fn handle_backspace(&mut self) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            self.select_to(prev);
        }
        self.handle_replace_text(None, "");
    }

    pub fn handle_delete(&mut self) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            self.select_to(next);
        }
        self.handle_replace_text(None, "");
    }

    pub fn handle_left(&mut self) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            self.move_to(prev);
        } else {
            self.move_to(self.selected_range.start);
        }
    }

    pub fn handle_right(&mut self) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.selected_range.end);
            self.move_to(next);
        } else {
            self.move_to(self.selected_range.end);
        }
    }

    pub fn handle_select_left(&mut self) {
        let prev = self.previous_boundary(self.cursor_offset());
        self.select_to(prev);
    }

    pub fn handle_select_right(&mut self) {
        let next = self.next_boundary(self.cursor_offset());
        self.select_to(next);
    }

    pub fn handle_select_all(&mut self) {
        self.move_to(0);
        self.select_to(self.content.len());
    }

    pub fn handle_home(&mut self) {
        self.move_to(0);
    }

    pub fn handle_end(&mut self) {
        self.move_to(self.content.len());
    }

    pub fn handle_paste_text(&mut self, text: &str) {
        self.handle_replace_text(None, &text.replace('\n', " "));
    }

    pub fn backspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.handle_backspace();
        self.notify_change(window, cx);
        cx.notify();
    }

    pub fn delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.handle_delete();
        self.notify_change(window, cx);
        cx.notify();
    }

    pub fn left(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.handle_left();
        cx.notify();
    }

    pub fn right(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.handle_right();
        cx.notify();
    }

    pub fn select_left(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.handle_select_left();
        cx.notify();
    }

    pub fn select_right(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.handle_select_right();
        cx.notify();
    }

    pub fn select_all(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.handle_select_all();
        cx.notify();
    }

    pub fn home(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.handle_home();
        cx.notify();
    }

    pub fn end(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.handle_end();
        cx.notify();
    }

    pub fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.handle_paste_text(&text);
            self.notify_change(window, cx);
            cx.notify();
        }
    }

    pub fn copy(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let text = self.content[self.selected_range.clone()].to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    pub fn cut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let text = self.content[self.selected_range.clone()].to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.handle_replace_text(None, "");
            self.notify_change(window, cx);
            cx.notify();
        }
    }

    fn notify_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref cb) = self.on_change {
            let cb = cb.clone();
            let text = self.content.to_string();
            cb(&text, window, cx);
        }
    }
}

impl EntityInputHandler for SearchInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        let start = range.start.min(self.content.len());
        let end = range.end.min(self.content.len()).max(start);
        Some(self.content[start..end].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.handle_unmark_text();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.handle_replace_text(range_utf16, new_text);
        self.notify_change(window, cx);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.handle_replace_and_mark_text(range_utf16, new_text, new_selected_range_utf16);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let start_x = if range.start <= self.content.len() {
            last_layout.x_for_index(range.start)
        } else {
            px(0.)
        };
        let end_x = if range.end <= self.content.len() {
            last_layout.x_for_index(range.end)
        } else {
            start_x
        };
        Some(Bounds::from_corners(
            point(bounds.left() + start_x, bounds.top()),
            point(bounds.left() + end_x, bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        let utf8_index = last_layout.index_for_x(point.x - line_point.x)?;
        Some(self.offset_to_utf16(utf8_index))
    }
}

impl Focusable for SearchInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SearchInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        div()
            .id("drawer_search_input_view")
            .flex()
            .flex_1()
            .items_center()
            .track_focus(&focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                let mods = event.keystroke.modifiers;
                if mods.platform {
                    match key {
                        "v" | "V" => {
                            this.paste(window, cx);
                            return;
                        }
                        "c" | "C" => {
                            this.copy(window, cx);
                            return;
                        }
                        "x" | "X" => {
                            this.cut(window, cx);
                            return;
                        }
                        "a" | "A" => {
                            this.select_all(window, cx);
                            return;
                        }
                        _ => {}
                    }
                }
                match key {
                    "backspace" => this.backspace(window, cx),
                    "delete" => this.delete(window, cx),
                    "left" => {
                        if mods.shift {
                            this.select_left(window, cx);
                        } else {
                            this.left(window, cx);
                        }
                    }
                    "right" => {
                        if mods.shift {
                            this.select_right(window, cx);
                        } else {
                            this.right(window, cx);
                        }
                    }
                    "home" => this.home(window, cx),
                    "end" => this.end(window, cx),
                    "escape" => {
                        this.content = "".into();
                        this.selected_range = 0..0;
                        this.marked_range = None;
                        this.notify_change(window, cx);
                        cx.notify();
                    }
                    _ => {}
                }
            }))
            .child(SearchInputElement {
                input: cx.entity().clone(),
            })
    }
}

#[allow(dead_code)]
pub struct SearchInputElement {
    pub input: Entity<SearchInput>,
}

#[allow(dead_code)]
pub struct SearchInputPrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for SearchInputElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SearchInputElement {
    type RequestLayoutState = ();
    type PrepaintState = SearchInputPrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), hsla(0., 0., 0., 0.35))
        } else {
            (content, style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let runs = if let Some(marked_range) = input.marked_range.as_ref() {
            let start = marked_range.start.min(display_text.len());
            let end = marked_range.end.min(display_text.len()).max(start);
            vec![
                TextRun {
                    len: start,
                    ..run.clone()
                },
                TextRun {
                    len: end - start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let cursor_pos = line.x_for_index(cursor.min(line.len));
        let (selection, cursor_quad) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos, bounds.top()),
                        size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    gpui::blue(),
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected_range.start.min(line.len)),
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected_range.end.min(line.len)),
                            bounds.bottom(),
                        ),
                    ),
                    rgba(0x3311ff30),
                )),
                None,
            )
        };

        SearchInputPrepaintState {
            line: Some(line),
            cursor: cursor_quad,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }
        let line = prepaint.line.take().unwrap();
        let _ = line.paint(bounds.origin, window.line_height(), window, cx);

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

#[allow(dead_code)]
pub fn render_drawer(
    is_fullscreen: bool,
    active_tab: DrawerTab,
    workspace_roots: &[WorkspaceRoot],
    loose_docs: &[DocKey],
    open_docs: &[OpenDoc],
    active_key: Option<&DocKey>,
    headings: &[HeadingItem],
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    scroll_handle: &ScrollHandle,
    on_select_tab: Arc<dyn Fn(DrawerTab, &mut Window, &mut App)>,
    on_toggle_root: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_remove_root: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_refresh_root: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_toggle_folder: Arc<dyn Fn(usize, &PathBuf, &mut Window, &mut App)>,
    on_select_file: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_select_doc: Arc<dyn Fn(&DocKey, &mut Window, &mut App)>,
    on_close_doc: Arc<dyn Fn(&DocKey, &mut Window, &mut App)>,
    on_new_draft: Arc<dyn Fn(&mut Window, &mut App)>,
    on_select_heading: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_open_folder: Arc<dyn Fn(&mut Window, &mut App)>,
    on_toggle_drawer: Arc<dyn Fn(&mut Window, &mut App)>,
    on_new_file_in_dir: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
) -> impl IntoElement {
    render_drawer_with_search(
        is_fullscreen,
        active_tab,
        workspace_roots,
        loose_docs,
        open_docs,
        active_key,
        headings,
        theme,
        strings,
        scroll_handle,
        &DrawerSearchParams::default(),
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
}

pub fn render_drawer_with_search(
    is_fullscreen: bool,
    active_tab: DrawerTab,
    workspace_roots: &[WorkspaceRoot],
    loose_docs: &[DocKey],
    open_docs: &[OpenDoc],
    active_key: Option<&DocKey>,
    headings: &[HeadingItem],
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    scroll_handle: &ScrollHandle,
    search: &DrawerSearchParams,
    on_select_tab: Arc<dyn Fn(DrawerTab, &mut Window, &mut App)>,
    on_toggle_root: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_remove_root: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_refresh_root: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_toggle_folder: Arc<dyn Fn(usize, &PathBuf, &mut Window, &mut App)>,
    on_select_file: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_select_doc: Arc<dyn Fn(&DocKey, &mut Window, &mut App)>,
    on_close_doc: Arc<dyn Fn(&DocKey, &mut Window, &mut App)>,
    _on_new_draft: Arc<dyn Fn(&mut Window, &mut App)>,
    on_select_heading: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_open_folder: Arc<dyn Fn(&mut Window, &mut App)>,
    on_toggle_drawer: Arc<dyn Fn(&mut Window, &mut App)>,
    on_new_file_in_dir: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
) -> impl IntoElement {
    let t1 = on_select_tab.clone();
    let t2 = on_select_tab.clone();
    let t_folder = on_open_folder.clone();
    let t_toggle = on_toggle_drawer.clone();

    let is_macos = cfg!(target_os = "macos");
    let drawer_header_pl = px(crate::views::workspace::adaptive_header_padding(is_macos, is_fullscreen, false));

    let is_searching = !search.search_query.is_empty();

    let mut search_row = div()
        .id("drawer_search_container")
        .w_full()
        .h(px(28.))
        .px_2()
        .rounded(px(6.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.surface)
        .flex()
        .items_center()
        .justify_between()
        .gap_1p5();

    if let Some(ref on_focus) = search.on_focus_search {
        let on_focus_cb = on_focus.clone();
        search_row = search_row
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                on_focus_cb(window, cx);
            });
    }

    let search_left = div()
        .flex()
        .items_center()
        .gap_1p5()
        .min_w(px(0.))
        .flex_1()
        .child(
            svg()
                .path("icons/search.svg")
                .size(px(14.))
                .text_color(theme.text_muted),
        )
        .child(if let Some(ref input) = search.search_input {
            div()
                .id("drawer_search_input_host")
                .min_w(px(0.))
                .flex_1()
                .child(input.clone())
                .into_any_element()
        } else {
            div()
                .id("drawer_search_query_display")
                .min_w(px(0.))
                .flex_1()
                .truncate()
                .text_size(px(12.))
                .font_weight(FontWeight::NORMAL)
                .text_color(if is_searching {
                    theme.text_primary
                } else {
                    theme.text_muted
                })
                .child(if is_searching {
                    search.search_query.clone()
                } else {
                    strings.search_placeholder.to_string()
                })
                .into_any_element()
        });

    search_row = search_row.child(search_left);

    if is_searching {
        let on_clear = search.on_clear_search.clone();
        let clear_btn = div()
            .id("btn_clear_search")
            .w(px(16.))
            .h(px(16.))
            .rounded(px(3.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_size(px(12.))
            .text_color(theme.text_muted)
            .hover(|s| s.bg(rgba(0xffffff20)).text_color(theme.text_primary))
            .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                cx.stop_propagation();
                if let Some(ref cb) = on_clear {
                    cb(window, cx);
                }
            })
            .child("×");
        search_row = search_row.child(clear_btn);
    }

    let mut fixed_search_area = div()
        .id("drawer_search_fixed_container")
        .px_2()
        .pt_2()
        .flex()
        .flex_col()
        .gap_1()
        .child(search_row);

    // Status indicators below search input
    if !search.search_in_progress && is_searching && !search.has_search_matches {
        fixed_search_area = fixed_search_area.child(
            div()
                .id("drawer_search_no_results")
                .px_2()
                .py_1()
                .text_size(px(11.))
                .text_color(theme.text_muted)
                .child(strings.search_no_results),
        );
    }

    if search.search_in_progress {
        fixed_search_area = fixed_search_area.child(
            div()
                .id("drawer_search_scanning")
                .px_2()
                .py_0p5()
                .text_size(px(11.))
                .text_color(theme.text_muted)
                .child(strings.search_scanning),
        );
    }

    if search.search_truncated {
        fixed_search_area = fixed_search_area.child(
            div()
                .id("drawer_search_incomplete")
                .px_2()
                .py_0p5()
                .text_size(px(11.))
                .text_color(theme.conflict_border)
                .child(strings.search_incomplete),
        );
    }

    let mut root_container = div()
        .w(px(crate::views::editor::DRAWER_WIDTH))
        .h_full()
        .bg(theme.bg)
        .border_r_1()
        .border_color(theme.border)
        .flex()
        .flex_col()
        // Top Header: 38px matching right canvas header, seamless with no horizontal bottom border
        .child(
            div()
                .h(px(38.))
                .pl(drawer_header_pl)
                .pr_2()
                .flex()
                .items_center()
                .justify_between()
                .gap_1()
                .child(render_icon_btn(
                    "drawer_collapse_btn",
                    &format!("{} (⌘B)", strings.collapse_sidebar),
                    "icons/panel-left.svg",
                    false,
                    theme,
                    move |_, window, cx| t_toggle(window, cx),
                ))
                .child(render_icon_btn(
                    "drawer_tab_files",
                    strings.tab_files,
                    "icons/files.svg",
                    active_tab == DrawerTab::Files,
                    theme,
                    move |_, window, cx| t1(DrawerTab::Files, window, cx),
                ))
                .child(render_icon_btn(
                    "drawer_tab_outline",
                    strings.tab_outline,
                    "icons/list.svg",
                    active_tab == DrawerTab::Outline,
                    theme,
                    move |_, window, cx| t2(DrawerTab::Outline, window, cx),
                ))
                .child(render_icon_btn(
                    "drawer_add_folder_btn",
                    strings.add_folder,
                    "icons/folder-plus.svg",
                    false,
                    theme,
                    move |_, window, cx| t_folder(window, cx),
                ))
                .child(
                    div()
                        .id("drawer_header_drag_area")
                        .flex_1()
                        .h_full()
                        .window_control_area(WindowControlArea::Drag)
                        .on_mouse_down(MouseButton::Left, |_e, window, _| {
                            window.start_window_move();
                        }),
                ),
        );

    if active_tab == DrawerTab::Files {
        root_container = root_container.child(fixed_search_area);
    }

    root_container.child(
        div()
            .id("drawer_content_scroll")
            .track_scroll(scroll_handle)
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .p_2()
            .flex()
            .flex_col()
            .gap_1()
            .child(match active_tab {
                DrawerTab::Files => {
                    let mut content = div().flex().flex_col().gap_1();

                    // ==========================================
                    // 1. Draft Item (decentralized: rendered only when an active/open scratch doc exists)
                    // ==========================================
                    if let Some(draft_doc) = open_docs.iter().find(|d| d.key.is_scratch()) {
                        let draft_title = draft_doc.doc.display_name_with_fallback(strings.untitled_file);
                        let show_draft = search.search_query.is_empty()
                            || draft_title.to_lowercase().contains(&search.search_query.to_lowercase());
                        if show_draft {
                                let is_draft_active = active_key == Some(&draft_doc.key);
                                let d_key = draft_doc.key.clone();
                                let t_switch_draft = on_select_doc.clone();
                                let t_close_draft = on_close_doc.clone();

                                let close_key = d_key.clone();
                                let on_close = t_close_draft.clone();
                            let draft_right = div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .id("btn_close_draft")
                                        .w(px(16.))
                                        .h(px(16.))
                                        .rounded(px(3.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_size(px(12.))
                                        .text_color(theme.text_muted)
                                        .opacity(0.0)
                                        .group_hover("drawer_draft_row", |s| s.opacity(1.0))
                                        .hover(|s| s.bg(rgba(0xffffff20)).text_color(theme.text_primary))
                                        .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                                            cx.stop_propagation();
                                            on_close(&close_key, window, cx);
                                        })
                                        .child("×"),
                                );

                            let draft_tooltip_text = format!("{} (⌘N)", strings.draft_focus_hint);
                            let sel_key = d_key.clone();
                            let mut draft_item = div()
                                .id("drawer_draft_item")
                                .group("drawer_draft_row")
                                .w_full()
                                .h(px(28.))
                                .px_2()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_between()
                                .text_size(px(13.))
                                .font_weight(FontWeight::NORMAL)
                                .tooltip(move |_window, cx| {
                                    let t = draft_tooltip_text.clone();
                                    cx.new(|_| DrawerTooltipView { text: t }).into()
                                })
                                .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                    t_switch_draft(&sel_key, window, cx);
                                });

                            if is_draft_active {
                                draft_item = draft_item
                                    .bg(theme.selection_bg())
                                    .text_color(theme.selection_text())
                                    .font_weight(FontWeight::MEDIUM);
                            } else {
                                draft_item = draft_item
                                    .text_color(theme.text_primary)
                                    .hover(|s| s.bg(theme.hover_bg()));
                            }

                            let draft_item = draft_item
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1p5()
                                        .min_w(px(0.))
                                        .child(
                                            svg()
                                                .path("icons/files.svg")
                                                .size(px(13.))
                                                .text_color(if is_draft_active {
                                                    theme.selection_text()
                                                } else {
                                                    theme.text_muted
                                                }),
                                        )
                                        .child(
                                            div()
                                                .min_w(px(0.))
                                                .truncate()
                                                .child(draft_title),
                                        ),
                                )
                                .child(draft_right);

                            content = content.child(draft_item);
                            }
                        }

                        // ==========================================
                        // 2. Section 2: Workspace Roots Group (工作区多根组)
                        // ==========================================
                        let mut roots_section = div()
                            .flex()
                            .flex_col()
                            .gap_0p5();

                        if workspace_roots.is_empty() {
                            roots_section = roots_section.child(
                                div()
                                    .px_2()
                                    .py_2()
                                    .flex()
                                    .items_center()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(theme.text_muted)
                                    .child(strings.workspace_empty),
                            );
                        } else {
                            for (root_idx, root) in workspace_roots.iter().enumerate() {
                                let root_path_str = root.path.to_string_lossy().to_string();
                                let root_name = root_label(workspace_roots, root_idx);
                                let is_missing = root.is_missing;
                                let is_deferred = root.deferred;
                                let is_expanded = root.is_expanded;
                                let is_truncated = root.truncated;
                                let on_toggle_r = on_toggle_root.clone();
                                let on_refresh_r = on_refresh_root.clone();
                                let on_remove_r = on_remove_root.clone();

                                let mut root_header = div()
                                    .id(SharedString::from(format!("drawer_root_header_{}", root_idx)))
                                    .group("drawer_root_row")
                                    .w_full()
                                    .h(px(28.))
                                    .px_2()
                                    .rounded(px(6.))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .hover(|s| s.bg(theme.hover_bg()))
                                    .tooltip(move |_window, cx| {
                                        let t = root_path_str.clone();
                                        cx.new(|_| DrawerTooltipView { text: t }).into()
                                    });

                                let on_refresh_header = on_refresh_r.clone();
                                let on_remove_header = on_remove_r.clone();
                                root_header = root_header
                                    .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                                        if is_missing {
                                            return;
                                        }
                                        if is_deferred {
                                            on_refresh_header(root_idx, window, cx);
                                        } else {
                                            on_toggle_r(root_idx, window, cx);
                                        }
                                    })
                                    .on_mouse_down(MouseButton::Right, move |_e, window, cx| {
                                        cx.stop_propagation();
                                        on_remove_header(root_idx, window, cx);
                                    });

                                let folder_icon = if is_expanded {
                                    "icons/folder-open.svg"
                                } else {
                                    "icons/folder.svg"
                                };

                                let left_part = div()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .min_w(px(0.))
                                    .child(
                                        svg()
                                            .path(folder_icon)
                                            .size(px(14.))
                                            .text_color(theme.text_muted),
                                    )
                                    .child(
                                        div()
                                            .min_w(px(0.))
                                            .truncate()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(if is_missing { theme.text_muted } else { theme.text_primary })
                                            .child(root_name),
                                    )
                                    .when(is_missing, |d| {
                                        d.child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(theme.text_muted)
                                                .child(format!("({})", strings.missing_root)),
                                        )
                                    })
                                    .when(is_deferred, |d| {
                                        d.child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(theme.accent)
                                                .child(format!("({})", strings.deferred_hint)),
                                        )
                                    });

                                let on_new_file_root = on_new_file_in_dir.clone();
                                let root_dir_path = root.path.clone();
                                let right_part = div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("btn_new_file_root_{}", root_idx)))
                                            .w(px(16.))
                                            .h(px(16.))
                                            .rounded(px(3.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(12.))
                                            .text_color(theme.text_muted)
                                            .opacity(0.0)
                                            .group_hover("drawer_root_row", |s| s.opacity(1.0))
                                            .hover(|s| s.bg(rgba(0xffffff20)).text_color(theme.text_primary))
                                            .tooltip(move |_window, cx| {
                                                let t = strings.new_file.to_string();
                                                cx.new(|_| DrawerTooltipView { text: t }).into()
                                            })
                                            .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                                                cx.stop_propagation();
                                                on_new_file_root(&root_dir_path, window, cx);
                                            })
                                            .child("+"),
                                    );

                                root_header = root_header.child(left_part).child(right_part);
                                roots_section = roots_section.child(root_header);

                                if is_expanded && !is_missing {
                                    let mut root_children_container = div().flex().flex_col().gap_0p5();

                                    if root.tree.is_empty() && !is_deferred {
                                        root_children_container = root_children_container.child(
                                            div()
                                                .pl(px(24.))
                                                .py_1()
                                                .text_size(px(11.))
                                                .text_color(theme.text_muted)
                                                .child(strings.no_markdown_files),
                                        );
                                    } else {
                                        let mut tree_elements = Vec::new();
                                        render_file_tree_nodes(
                                            &root.tree,
                                            0,
                                            root_idx,
                                            open_docs,
                                            active_key,
                                            theme,
                                            strings,
                                            &search.search_query,
                                            &on_toggle_folder,
                                            &on_new_file_in_dir,
                                            &on_select_file,
                                            &on_close_doc,
                                            &mut tree_elements,
                                        );
                                        root_children_container = root_children_container.children(tree_elements);
                                    }

                                    if is_truncated {
                                        let on_refresh = on_refresh_r.clone();
                                        root_children_container = root_children_container.child(
                                            div()
                                                .id(SharedString::from(format!("btn_retry_truncated_{}", root_idx)))
                                                .pl(px(24.))
                                                .pr_2()
                                                .py_1()
                                                .rounded(px(4.))
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .gap_1p5()
                                                .hover(|s| s.bg(theme.hover_bg()))
                                                .tooltip(move |_window, cx| {
                                                    let t = strings.retry_scan.to_string();
                                                    cx.new(|_| DrawerTooltipView { text: t }).into()
                                                })
                                                .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                                                    cx.stop_propagation();
                                                    on_refresh(root_idx, window, cx);
                                                })
                                                .child(
                                                    div()
                                                        .text_size(px(11.))
                                                        .text_color(theme.text_muted)
                                                        .child(format!("({})", strings.truncated_hint)),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(11.))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(theme.accent)
                                                        .child(format!("· {}", strings.retry_scan)),
                                                ),
                                        );
                                    }

                                    roots_section = roots_section.child(root_children_container);
                                }
                            }
                        }

                        content = content.child(roots_section);

                        // ==========================================
                        // 3. Section 3: Loose Files Group (外部文件组)
                        // ==========================================
                        let loose_file_paths: Vec<&PathBuf> = loose_docs
                            .iter()
                            .filter_map(|k| match k {
                                DocKey::Path(p) => {
                                    if !search.search_query.is_empty() {
                                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                                        if !name.to_lowercase().contains(&search.search_query.to_lowercase()) {
                                            return None;
                                        }
                                    }
                                    Some(p)
                                }
                                DocKey::Scratch(_) => None,
                            })
                            .collect();

                        if !loose_file_paths.is_empty() {
                            let mut loose_section = div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    div()
                                        .px_2()
                                        .pt_3()
                                        .pb_0p5()
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text_muted)
                                        .child(strings.loose_files),
                                );

                            for (idx, p) in loose_file_paths.iter().enumerate() {
                                let target_key = DocKey::Path((*p).clone());
                                let is_active = active_key == Some(&target_key);
                                let file_name = p
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or(strings.untitled_file)
                                    .to_string();
                                let tooltip_path = p.to_string_lossy().to_string();
                                let on_select = on_select_doc.clone();
                                let on_close = on_close_doc.clone();
                                let sel_key = target_key.clone();
                                let close_key = target_key.clone();

                                let mut item = div()
                                    .id(SharedString::from(format!("drawer_loose_file_{}", idx)))
                                    .group("drawer_loose_file_row")
                                    .w_full()
                                    .h(px(28.))
                                    .px_2()
                                    .rounded(px(6.))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::NORMAL)
                                    .tooltip(move |_window, cx| {
                                        let t = tooltip_path.clone();
                                        cx.new(|_| DrawerTooltipView { text: t }).into()
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                        on_select(&sel_key, window, cx);
                                    });

                                if is_active {
                                    item = item
                                        .bg(theme.selection_bg())
                                        .text_color(theme.selection_text())
                                        .font_weight(FontWeight::MEDIUM);
                                } else {
                                    item = item
                                        .text_color(theme.text_primary)
                                        .hover(|s| s.bg(theme.hover_bg()));
                                }

                                let left_part = div()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .min_w(px(0.))
                                    .child(
                                        svg()
                                            .path("icons/files.svg")
                                            .size(px(13.))
                                            .text_color(if is_active {
                                                theme.selection_text()
                                            } else {
                                                theme.text_muted
                                            }),
                                    )
                                    .child(
                                        div()
                                            .min_w(px(0.))
                                            .truncate()
                                            .child(file_name),
                                    );

                                let right_part = div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("btn_close_loose_file_{}", idx)))
                                            .w(px(16.))
                                            .h(px(16.))
                                            .rounded(px(3.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(12.))
                                            .text_color(theme.text_muted)
                                            .opacity(0.0)
                                            .group_hover("drawer_loose_file_row", |s| s.opacity(1.0))
                                            .hover(|s| s.bg(rgba(0xffffff20)).text_color(theme.text_primary))
                                            .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                                                cx.stop_propagation();
                                                on_close(&close_key, window, cx);
                                            })
                                            .child("×"),
                                    );

                                loose_section = loose_section.child(item.child(left_part).child(right_part));
                            }

                            content = content.child(loose_section);
                        }

                        content.into_any_element()
                    }
                    DrawerTab::Outline => {
                        if headings.is_empty() {
                            div()
                                .p_4()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child(strings.outline_empty)
                                .into_any_element()
                        } else {
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .children(headings.iter().enumerate().map(|(idx, h)| {
                                    let block_idx = h.block_index;
                                    let on_jump = on_select_heading.clone();
                                    let indent = ((h.level.saturating_sub(1) as f32) * 12.0).min(36.0);

                                    let (font_weight, text_color) = match h.level {
                                        1 => (FontWeight::BOLD, theme.text_primary),
                                        2 => (FontWeight::SEMIBOLD, theme.text_primary),
                                        _ => (FontWeight::NORMAL, theme.text_muted),
                                    };

                                    let heading_text = h.text.clone();
                                    let tooltip_heading = heading_text.clone();

                                    div()
                                        .id(("drawer_heading_item", idx))
                                        .w_full()
                                        .min_w(px(0.))
                                        .pl(px(indent + 8.0))
                                        .pr_2()
                                        .py_1()
                                        .rounded_md()
                                        .text_xs()
                                        .font_weight(font_weight)
                                        .text_color(text_color)
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.hover_bg()).text_color(theme.accent))
                                        .tooltip(move |_window, cx| {
                                            let t = tooltip_heading.clone();
                                            cx.new(|_| DrawerTooltipView { text: t }).into()
                                        })
                                        .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                            on_jump(block_idx, window, cx);
                                        })
                                        .child(
                                            div()
                                                .w_full()
                                                .min_w(px(0.))
                                                .truncate()
                                                .child(heading_text),
                                        )
                                }))
                                .into_any_element()
                        }
                    }
                }),
        )
}

fn dir_matches_search(node: &FileTreeNode, q_lower: &str) -> bool {
    if node.name.to_lowercase().contains(q_lower) {
        return true;
    }
    crate::state::node_tree_has_match(&node.children, q_lower)
}

fn render_file_tree_nodes(
    nodes: &[FileTreeNode],
    depth: usize,
    root_idx: usize,
    open_docs: &[OpenDoc],
    active_key: Option<&DocKey>,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    search_query: &str,
    on_toggle_folder: &Arc<dyn Fn(usize, &PathBuf, &mut Window, &mut App)>,
    on_new_file_in_dir: &Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_select_file: &Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_close_doc: &Arc<dyn Fn(&DocKey, &mut Window, &mut App)>,
    out: &mut Vec<AnyElement>,
) {
    let q_lower = search_query.to_lowercase();
    for (node_idx, node) in nodes.iter().enumerate() {
        let depth_indent = (depth as f32 + 1.0) * 12.0;
        if node.is_dir {
            if !search_query.is_empty() && !dir_matches_search(node, &q_lower) {
                continue;
            }
            let p = node.path.clone();
            let on_toggle = on_toggle_folder.clone();
            let is_expanded = node.is_expanded;
            let folder_name = node.name.clone();

            let folder_icon = if is_expanded {
                "icons/folder-open.svg"
            } else {
                "icons/folder.svg"
            };

            let left_part = div()
                .flex()
                .items_center()
                .gap_1p5()
                .min_w(px(0.))
                .child(
                    svg()
                        .path(folder_icon)
                        .size(px(13.))
                        .text_color(theme.text_muted),
                )
                .child(
                    div()
                        .min_w(px(0.))
                        .truncate()
                        .child(folder_name),
                );

            let on_new_file_dir = on_new_file_in_dir.clone();
            let dir_path = p.clone();
            let right_part = div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .id(SharedString::from(format!("btn_new_file_dir_{}_{}_{}", root_idx, depth, node_idx)))
                        .w(px(16.))
                        .h(px(16.))
                        .rounded(px(3.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.))
                        .text_color(theme.text_muted)
                        .opacity(0.0)
                        .group_hover("drawer_dir_row", |s| s.opacity(1.0))
                        .hover(|s| s.bg(rgba(0xffffff20)).text_color(theme.text_primary))
                        .tooltip(move |_window, cx| {
                            let t = strings.new_file.to_string();
                            cx.new(|_| DrawerTooltipView { text: t }).into()
                        })
                        .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                            cx.stop_propagation();
                            on_new_file_dir(&dir_path, window, cx);
                        })
                        .child("+"),
                );

            out.push(
                div()
                    .id(SharedString::from(format!("drawer_tree_dir_{}_{}_{}", root_idx, depth, node_idx)))
                    .group("drawer_dir_row")
                    .w_full()
                    .h(px(28.))
                    .pl(px(depth_indent + 4.0))
                    .pr_2()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_muted)
                    .hover(|s| s.bg(theme.hover_bg()).text_color(theme.text_primary))
                    .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                        on_toggle(root_idx, &p, window, cx);
                    })
                    .child(left_part)
                    .child(right_part)
                    .into_any_element(),
            );

            if is_expanded {
                render_file_tree_nodes(
                    &node.children,
                    depth + 1,
                    root_idx,
                    open_docs,
                    active_key,
                    theme,
                    strings,
                    search_query,
                    on_toggle_folder,
                    on_new_file_in_dir,
                    on_select_file,
                    on_close_doc,
                    out,
                );
            }
        } else {
            if !search_query.is_empty() && !node.name.to_lowercase().contains(&q_lower) {
                continue;
            }
            let p = node.path.clone();
            let target_key = DocKey::Path(p.clone());
            let is_active = active_key == Some(&target_key);
            let open_doc = open_docs.iter().find(|d| d.key == target_key);
            let is_open = open_doc.is_some();

            let file_name = node.name.clone();
            let tooltip_name = file_name.clone();
            let on_select = on_select_file.clone();
            let on_close = on_close_doc.clone();
            let close_key = target_key.clone();

            let mut item = div()
                .id(SharedString::from(format!("drawer_tree_file_{}_{}_{}", root_idx, depth, node_idx)))
                .group("drawer_file_row")
                .w_full()
                .h(px(44.))
                .pl(px(depth_indent + 8.0))
                .pr_1p5()
                .py_1()
                .rounded(px(6.))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(13.))
                .font_weight(FontWeight::NORMAL)
                .tooltip(move |_window, cx| {
                    let t = tooltip_name.clone();
                    cx.new(|_| DrawerTooltipView { text: t }).into()
                })
                .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                    on_select(&p, window, cx);
                });

            if is_active {
                item = item
                    .bg(theme.selection_bg())
                    .text_color(theme.selection_text())
                    .font_weight(FontWeight::MEDIUM);
            } else {
                item = item
                    .text_color(theme.text_primary)
                    .hover(|s| s.bg(theme.hover_bg()));
            }

            let mut left_part = div()
                .flex()
                .flex_col()
                .justify_center()
                .min_w(px(0.))
                .child(
                    div()
                        .min_w(px(0.))
                        .truncate()
                        .text_size(px(13.))
                        .child(file_name),
                );

            if let Some(ref mtime_str) = node.mtime {
                left_part = left_part.child(
                    div()
                        .min_w(px(0.))
                        .truncate()
                        .text_size(px(11.))
                        .text_color(theme.text_muted)
                        .child(mtime_str.clone()),
                );
            }

            let mut right_part = div().flex().items_center().gap_1();

            if is_open {
                right_part = right_part.child(
                    div()
                        .id(SharedString::from(format!("btn_close_file_{}_{}_{}", root_idx, depth, node_idx)))
                        .w(px(16.))
                        .h(px(16.))
                        .rounded(px(3.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.))
                        .text_color(theme.text_muted)
                        .opacity(0.0)
                        .group_hover("drawer_file_row", |s| s.opacity(1.0))
                        .hover(|s| s.bg(rgba(0xffffff20)).text_color(theme.text_primary))
                        .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                            cx.stop_propagation();
                            on_close(&close_key, window, cx);
                        })
                        .child("×"),
                );
            }

            out.push(item.child(left_part).child(right_part).into_any_element());
        }
    }
}

struct DrawerTooltipView {
    text: String,
}

impl Render for DrawerTooltipView {
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
            .font_family("ui-sans-serif, system-ui")
            .shadow_md()
            .child(self.text.clone())
    }
}

fn render_icon_btn<F>(
    id: impl Into<ElementId>,
    tooltip_text: &str,
    icon_path: &'static str,
    active: bool,
    theme: &ThemeColors,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let text = tooltip_text.to_string();
    let base = div()
        .id(id)
        .tooltip(move |_window, cx| {
            let t = text.clone();
            cx.new(|_| DrawerTooltipView { text: t }).into()
        })
        .w(px(28.))
        .h(px(28.))
        .rounded_md()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, handler);

    let color = if active { theme.selection_text() } else { theme.text_muted };
    let icon_el = svg().path(icon_path).size(px(16.)).text_color(color);

    if active {
        base.bg(theme.selection_bg())
            .rounded(px(6.))
            .child(icon_el)
    } else {
        base.rounded(px(6.))
            .hover(|s| s.bg(theme.hover_bg()).text_color(theme.text_primary))
            .child(icon_el)
    }
}
