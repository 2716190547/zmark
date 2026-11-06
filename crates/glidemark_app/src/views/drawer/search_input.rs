use gpui::prelude::*;
use gpui::*;
use std::ops::Range;
use std::sync::Arc;

fn utf16_to_utf8_offset(s: &str, utf16_offset: usize) -> usize {
    let mut utf8 = 0;
    let mut utf16 = 0;
    for ch in s.chars() {
        if utf16 >= utf16_offset {
            break;
        }
        utf16 += ch.len_utf16();
        utf8 += ch.len_utf8();
    }
    utf8
}

fn utf8_to_utf16_offset(s: &str, utf8_offset: usize) -> usize {
    let mut utf16 = 0;
    let mut utf8 = 0;
    for ch in s.chars() {
        if utf8 >= utf8_offset {
            break;
        }
        utf8 += ch.len_utf8();
        utf16 += ch.len_utf16();
    }
    utf16
}

/// Search input component providing full Chinese IME support via `EntityInputHandler`.
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
    pub on_change: Option<Arc<dyn Fn(&str, &mut Window, &mut Context<Self>) + 'static>>,
}

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

    #[allow(dead_code)]
    pub fn set_text(&mut self, text: impl Into<SharedString>) {
        self.content = text.into();
        self.selected_range = self.content.len()..self.content.len();
        self.selection_reversed = false;
        self.marked_range = None;
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.content = "".into();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        cx.notify();
    }

    pub fn offset_from_utf16(&self, offset: usize) -> usize {
        utf16_to_utf8_offset(&self.content, offset)
    }

    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        utf8_to_utf16_offset(&self.content, offset)
    }

    pub fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        let start = self.offset_to_utf16(range.start.min(range.end));
        let end = self.offset_to_utf16(range.start.max(range.end));
        start..end
    }

    pub fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        let start = self.offset_from_utf16(range_utf16.start.min(range_utf16.end));
        let end = self.offset_from_utf16(range_utf16.start.max(range_utf16.end));
        start..end
    }

    fn floor_char_boundary(&self, offset: usize) -> usize {
        let offset = offset.min(self.content.len());
        let mut i = offset;
        while i > 0 && !self.content.is_char_boundary(i) {
            i -= 1;
        }
        i
    }

    fn ceil_char_boundary(&self, offset: usize) -> usize {
        let offset = offset.min(self.content.len());
        let mut i = offset;
        while i < self.content.len() && !self.content.is_char_boundary(i) {
            i += 1;
        }
        i
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
        let offset = self.floor_char_boundary(offset);
        self.selected_range = offset..offset;
        self.selection_reversed = false;
    }

    pub fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    pub fn select_to(&mut self, offset: usize) {
        let offset = self.floor_char_boundary(offset);
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

        let r_start = range.start.min(range.end);
        let r_end = range.start.max(range.end);
        let start = self.floor_char_boundary(r_start);
        let end = self.ceil_char_boundary(r_end);

        self.content = format!("{}{}{}", &self.content[0..start], new_text, &self.content[end..]).into();
        self.selected_range = start + new_text.len()..start + new_text.len();
        self.selection_reversed = false;
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

        let r_start = range.start.min(range.end);
        let r_end = range.start.max(range.end);
        let start = self.floor_char_boundary(r_start);
        let end = self.ceil_char_boundary(r_end);

        self.content = format!("{}{}{}", &self.content[0..start], new_text, &self.content[end..]).into();
        if !new_text.is_empty() {
            self.marked_range = Some(start..start + new_text.len());
        } else {
            self.marked_range = None;
        }

        let sel_offset_start = new_selected_range_utf16
            .as_ref()
            .map(|r| utf16_to_utf8_offset(new_text, r.start.min(r.end)))
            .unwrap_or(new_text.len());
        let sel_offset_end = new_selected_range_utf16
            .as_ref()
            .map(|r| utf16_to_utf8_offset(new_text, r.start.max(r.end)))
            .unwrap_or(new_text.len());

        self.selected_range = (start + sel_offset_start)..(start + sel_offset_end);
        self.selection_reversed = false;
    }

    pub fn handle_unmark_text(&mut self) {
        self.marked_range = None;
    }

    pub fn handle_backspace(&mut self) {
        if self.selected_range.is_empty() {
            let cursor = self.cursor_offset();
            if cursor == 0 {
                return;
            }
            let prev = self.previous_boundary(cursor);
            self.select_to(prev);
        }
        self.handle_replace_text(None, "");
    }

    pub fn handle_delete(&mut self) {
        if self.selected_range.is_empty() {
            let cursor = self.cursor_offset();
            if cursor >= self.content.len() {
                return;
            }
            let next = self.next_boundary(cursor);
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
            .text_size(px(13.))
            .track_focus(&focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                let mods = event.keystroke.modifiers;
                if mods.platform {
                    match key {
                        "v" | "V" => {
                            this.paste(window, cx);
                            cx.stop_propagation();
                            return;
                        }
                        "c" | "C" => {
                            this.copy(window, cx);
                            cx.stop_propagation();
                            return;
                        }
                        "x" | "X" => {
                            this.cut(window, cx);
                            cx.stop_propagation();
                            return;
                        }
                        "a" | "A" => {
                            this.select_all(window, cx);
                            cx.stop_propagation();
                            return;
                        }
                        _ => {}
                    }
                }
                match key {
                    "backspace" => {
                        this.backspace(window, cx);
                        cx.stop_propagation();
                    }
                    "delete" => {
                        this.delete(window, cx);
                        cx.stop_propagation();
                    }
                    "left" => {
                        if mods.shift {
                            this.select_left(window, cx);
                        } else {
                            this.left(window, cx);
                        }
                        cx.stop_propagation();
                    }
                    "right" => {
                        if mods.shift {
                            this.select_right(window, cx);
                        } else {
                            this.right(window, cx);
                        }
                        cx.stop_propagation();
                    }
                    "home" => {
                        this.home(window, cx);
                        cx.stop_propagation();
                    }
                    "end" => {
                        this.end(window, cx);
                        cx.stop_propagation();
                    }
                    "escape" => {
                        this.content = "".into();
                        this.selected_range = 0..0;
                        this.selection_reversed = false;
                        this.marked_range = None;
                        this.notify_change(window, cx);
                        cx.notify();
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .child(SearchInputElement {
                input: cx.entity().clone(),
            })
    }
}

pub struct SearchInputElement {
    pub input: Entity<SearchInput>,
}

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
            (input.placeholder.clone(), style.color.opacity(0.45))
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
        // Caret and selection derive from the themed text colour so no palette falls
        // back to the system blue.
        let caret_color = style.color.opacity(0.85);
        let selection_color = style.color.opacity(0.12);
        let (selection, cursor_quad) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos, bounds.top() + px(3.)),
                        size(px(1.5), bounds.bottom() - bounds.top() - px(6.)),
                    ),
                    caret_color,
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
                    selection_color,
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

