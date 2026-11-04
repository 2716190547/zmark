//! Shared syntax highlighting domain logic and contracts for GlideMark.
//!
//! Provides pure, headless syntect-based tokenization mapping to a minimal 6-role
//! semantic model (`SyntaxRole`), strictly guaranteeing contiguous byte-range coverage,
//! multi-line statefulness, input thresholds, and bounded LRU caching.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "test-observation")]
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

#[cfg(feature = "test-observation")]
pub static TEST_MOCK_CLOCK_MS: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "test-observation")]
pub static TEST_INITIAL_ADVANCE_MS: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "test-observation")]
pub static TEST_LINE_DELTAS_MS: [AtomicU64; 8] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];

#[cfg(feature = "test-observation")]
pub static TEST_LINE_COUNT: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature = "test-observation")]
pub fn reset_test_clock() {
    TEST_MOCK_CLOCK_MS.store(0, Ordering::Relaxed);
    TEST_INITIAL_ADVANCE_MS.store(0, Ordering::Relaxed);
    TEST_LINE_COUNT.store(0, Ordering::Relaxed);
    for slot in &TEST_LINE_DELTAS_MS {
        slot.store(0, Ordering::Relaxed);
    }
}

/// Canonical key identifying a code block's syntax highlighting work.
///
/// Equality and hashing are strictly content-based (language identifier and payload string),
/// NOT `Arc` pointer address.
#[derive(Clone, Debug)]
pub struct CodeKey {
    pub language: Option<&'static str>,
    pub payload: Arc<str>,
}

impl PartialEq for CodeKey {
    fn eq(&self, other: &Self) -> bool {
        self.language == other.language && *self.payload == *other.payload
    }
}

impl Eq for CodeKey {}

impl std::hash::Hash for CodeKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.language.hash(state);
        (*self.payload).hash(state);
    }
}

impl CodeKey {
    pub fn new(language: Option<&'static str>, payload: Arc<str>) -> Self {
        Self { language, payload }
    }

    pub fn from_raw(raw_info: Option<&str>, payload: Arc<str>) -> Self {
        let language = resolve_canonical_language(raw_info);
        Self { language, payload }
    }

    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }

    pub fn is_plain(&self) -> bool {
        self.language.is_none()
    }
}

/// Minimal 6-role semantic contract shared across source editor and markdown preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxRole {
    Plain,
    Keyword,
    Attribute,
    String,
    Number,
    Comment,
}

/// A contiguous, half-open UTF-8 byte range `[start, end)` tagged with a `SyntaxRole`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxSpan {
    pub range: Range<usize>,
    pub role: SyntaxRole,
}

/// Result of parsing a code block with bounded resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseResult {
    Ready(Arc<[SyntaxSpan]>),
    BudgetExceeded,
    Cancelled,
    HardLimit,
    UnsupportedLanguage,
    MappingInvalid,
    InitFailed,
}

impl ParseResult {
    pub fn is_ready(&self) -> bool {
        matches!(self, ParseResult::Ready(_))
    }

    pub fn spans(&self) -> Option<&Arc<[SyntaxSpan]>> {
        match self {
            ParseResult::Ready(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_spans(&self) -> &[SyntaxSpan] {
        match self {
            ParseResult::Ready(s) => s,
            _ => &[],
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, SyntaxSpan> {
        self.as_spans().iter()
    }
}

impl std::ops::Deref for ParseResult {
    type Target = [SyntaxSpan];
    fn deref(&self) -> &Self::Target {
        self.as_spans()
    }
}

impl<'a> IntoIterator for &'a ParseResult {
    type Item = &'a SyntaxSpan;
    type IntoIter = std::slice::Iter<'a, SyntaxSpan>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl From<ParseResult> for Arc<[SyntaxSpan]> {
    fn from(res: ParseResult) -> Self {
        match res {
            ParseResult::Ready(spans) => spans,
            _ => Arc::from([]),
        }
    }
}

impl From<&ParseResult> for Arc<[SyntaxSpan]> {
    fn from(res: &ParseResult) -> Self {
        match res {
            ParseResult::Ready(spans) => spans.clone(),
            _ => Arc::from([]),
        }
    }
}

/// Maximum allowed code block payload in bytes (64 KiB).
pub const MAX_CODE_BLOCK_BYTES: usize = 64 * 1024;

/// Maximum allowed physical lines in a single code block.
pub const MAX_CODE_BLOCK_LINES: usize = 2000;

/// Maximum allowed Unicode scalar count (chars) on any single physical line.
pub const MAX_LINE_UNICODE_SCALARS: usize = 1000;

/// Soft timeout budget for code block parsing (50 ms).
pub const PARSE_SOFT_BUDGET_MS: u64 = 50;

/// Maximum number of code blocks retained in the shared syntax cache.
pub const MAX_CACHE_ENTRIES: usize = 32;

/// Maximum total memory retained in the shared syntax cache (4 MiB).
pub const MAX_CACHE_BYTES: usize = 4 * 1024 * 1024;

/// Global syntect built-in syntax set (initialized on demand without blocking warm-up).
pub static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

/// Returns `true` if `SYNTAX_SET` has been initialized.
pub fn is_syntax_set_ready() -> bool {
    SYNTAX_SET.get().is_some()
}

/// Initializes `SYNTAX_SET` with syntect defaults and returns a static reference.
pub fn init_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(|| {
        let ss = SyntaxSet::load_defaults_newlines();
        // Pre-compile regexes for canonical syntaxes during initialization so parsing
        // code blocks never incurs one-time onig compilation against their parse budget.
        let warm_samples: &[(&str, &str)] = &[
            (
                "XML",
                "<!-- XML comment -->\n<root id=\"1\">\n  <item attr=\"val\">text</item>\n</root>\n",
            ),
            (
                "HTML",
                "<!DOCTYPE html>\n<html>\n  <head><title>GlideMark</title></head>\n  <body><p class=\"main\">Hello</p></body>\n</html>\n",
            ),
            (
                "Java",
                "// Java sample\npublic class Main {\n    public static void main(String[] args) {\n        System.out.println(42);\n    }\n}\n",
            ),
            (
                "Python",
                "# Python sample\ndef hello(name: str) -> str:\n    \"\"\"Greeting\"\"\"\n    return f'Hello {name}'\n",
            ),
            (
                "JavaScript",
                "// JS sample\nfunction calc(x) {\n    const mult = 42;\n    return x * mult;\n}\n",
            ),
            (
                "JSON",
                "{\n  \"name\": \"glidemark\",\n  \"count\": 123,\n  \"enabled\": true\n}\n",
            ),
            (
                "YAML",
                "# YAML sample\nkey: value\nitems:\n  - name: test\n    value: 1\n",
            ),
            (
                "SQL",
                "-- SQL query\nSELECT id, name, count(*)\nFROM users\nWHERE active = 1\nGROUP BY id, name;\n",
            ),
            (
                "Bourne Again Shell (bash)",
                "#!/bin/bash\n# Shell sample\nNAME=\"GlideMark\"\necho \"Running $NAME\"\nexit 0\n",
            ),
            (
                "Rust",
                "// Rust sample\npub fn add(a: i32, b: i32) -> i32 {\n    let sum = a + b;\n    sum\n}\n",
            ),
        ];
        for (name, sample) in warm_samples {
            if let Some(syntax) = ss.find_syntax_by_name(name) {
                let mut state = ParseState::new(syntax);
                for line in sample.split_inclusive('\n') {
                    let _ = state.parse_line(line, &ss);
                }
            }
        }
        ss
    })
}

/// Maps a raw fence info string or already-canonical name to a canonical language identifier recognized by GlideMark.
///
/// Rules:
/// - Accepts raw aliases ("bash", "sh", "rust", "js", etc.).
/// - Accepts already-canonical names ("Bourne Again Shell (bash)", "Rust", "JavaScript", etc.) so canonical names never get lost.
/// - Matches against the approved canonical list case-insensitively.
/// - Returns `None` for empty/text/plain or unknown/unsupported languages.
pub fn resolve_canonical_language(raw_info: Option<&str>) -> Option<&'static str> {
    let raw = raw_info?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();

    // Check multi-word canonical name "Bourne Again Shell (bash)" first
    if lower == "bourne again shell (bash)"
        || lower.starts_with("bourne again shell (bash) ")
        || lower.starts_with("bourne again shell (bash),")
        || lower.starts_with("bourne again shell (bash)\t")
    {
        return Some("Bourne Again Shell (bash)");
    }

    let token = lower
        .split(|c: char| c.is_whitespace() || c == ',')
        .next()?
        .trim();

    match token {
        "xml" => Some("XML"),
        "html" | "htm" => Some("HTML"),
        "java" => Some("Java"),
        "python" | "py" => Some("Python"),
        "javascript" | "js" => Some("JavaScript"),
        "json" => Some("JSON"),
        "yaml" | "yml" => Some("YAML"),
        "sql" => Some("SQL"),
        "shell" | "sh" | "bash" | "bourne again shell (bash)" => Some("Bourne Again Shell (bash)"),
        "rust" | "rs" => Some("Rust"),
        // Unhighlighted aliases
        "" | "text" | "txt" | "plain" | "plaintext" => None,
        // All others fallback to Plain (TS, TOML, zsh, etc. not in this batch)
        _ => None,
    }
}

/// Finds the `SyntaxReference` corresponding to a canonical language.
fn find_syntax_for_canonical(canonical: &str) -> Option<&'static SyntaxReference> {
    let ss = init_syntax_set();
    match canonical {
        "XML" => ss.find_syntax_by_name("XML"),
        "HTML" => ss.find_syntax_by_name("HTML"),
        "Java" => ss.find_syntax_by_name("Java"),
        "Python" => ss.find_syntax_by_name("Python"),
        "JavaScript" => ss.find_syntax_by_name("JavaScript"),
        "JSON" => ss.find_syntax_by_name("JSON"),
        "YAML" => ss.find_syntax_by_name("YAML"),
        "SQL" => ss.find_syntax_by_name("SQL"),
        "Bourne Again Shell (bash)" => ss.find_syntax_by_name("Bourne Again Shell (bash)"),
        "Rust" => ss.find_syntax_by_name("Rust"),
        _ => ss.find_syntax_by_token(canonical),
    }
}

/// Maps a syntect scope stack to the most specific `SyntaxRole`.
///
/// Searches from top (innermost/most specific scope) downwards.
pub fn scope_stack_to_role(stack: &ScopeStack) -> SyntaxRole {
    for scope in stack.as_slice().iter().rev() {
        let name = scope.build_string();
        // XML / HTML Tag names
        if name.starts_with("entity.name.tag") {
            return SyntaxRole::Keyword;
        }
        // XML / HTML Attribute names
        if name.starts_with("entity.other.attribute-name")
            || name.starts_with("variable.other.attribute")
        {
            return SyntaxRole::Attribute;
        }
        // Comments
        if name.starts_with("comment") || name.starts_with("punctuation.definition.comment") {
            return SyntaxRole::Comment;
        }
        // Strings
        if name.starts_with("string") || name.starts_with("punctuation.definition.string") {
            return SyntaxRole::String;
        }
        // Numbers
        if name.starts_with("constant.numeric") {
            return SyntaxRole::Number;
        }
        // Keywords and Storage types/modifiers
        if name.starts_with("keyword")
            || name.starts_with("storage.type")
            || name.starts_with("storage.modifier")
        {
            return SyntaxRole::Keyword;
        }
    }
    SyntaxRole::Plain
}

/// Normalizes raw spans by:
/// 1. Filling any gaps with `SyntaxRole::Plain` to ensure `0..code.len()` is completely covered.
/// 2. Merging contiguous adjacent spans having the same `SyntaxRole`.
/// 3. Guaranteeing valid UTF-8 character boundaries and strict ordering.
pub fn normalize_and_fill_spans(code: &str, raw_spans: Vec<SyntaxSpan>) -> Vec<SyntaxSpan> {
    if code.is_empty() {
        return Vec::new();
    }
    let code_len = code.len();
    let mut normalized: Vec<SyntaxSpan> = Vec::with_capacity(raw_spans.len() + 2);
    let mut current_offset = 0;

    for span in raw_spans {
        let start = span.range.start.clamp(0, code_len);
        let end = span.range.end.clamp(0, code_len);
        if start >= end {
            continue;
        }

        // Fill gap before this span with Plain
        if start > current_offset {
            let gap_range = current_offset..start;
            if let Some(last) = normalized.last_mut() {
                if last.role == SyntaxRole::Plain && last.range.end == gap_range.start {
                    last.range.end = gap_range.end;
                } else {
                    normalized.push(SyntaxSpan {
                        range: gap_range,
                        role: SyntaxRole::Plain,
                    });
                }
            } else {
                normalized.push(SyntaxSpan {
                    range: gap_range,
                    role: SyntaxRole::Plain,
                });
            }
            current_offset = start;
        }

        // If span starts before current_offset, adjust it (non-overlapping guarantee)
        let actual_start = start.max(current_offset);
        if actual_start >= end {
            continue;
        }
        let actual_range = actual_start..end;

        if let Some(last) = normalized.last_mut() {
            if last.role == span.role && last.range.end == actual_range.start {
                last.range.end = actual_range.end;
            } else {
                normalized.push(SyntaxSpan {
                    range: actual_range,
                    role: span.role,
                });
            }
        } else {
            normalized.push(SyntaxSpan {
                range: actual_range,
                role: span.role,
            });
        }
        current_offset = end;
    }

    // Fill remaining tail with Plain if any
    if current_offset < code_len {
        let tail_range = current_offset..code_len;
        if let Some(last) = normalized.last_mut() {
            if last.role == SyntaxRole::Plain && last.range.end == tail_range.start {
                last.range.end = tail_range.end;
            } else {
                normalized.push(SyntaxSpan {
                    range: tail_range,
                    role: SyntaxRole::Plain,
                });
            }
        } else {
            normalized.push(SyntaxSpan {
                range: tail_range,
                role: SyntaxRole::Plain,
            });
        }
    }

    // Fallback if normalized is still empty
    if normalized.is_empty() {
        normalized.push(SyntaxSpan {
            range: 0..code_len,
            role: SyntaxRole::Plain,
        });
    }

    normalized
}

/// Fallback single plain span for the entire payload.
fn fallback_plain(code: &str) -> Vec<SyntaxSpan> {
    if code.is_empty() {
        Vec::new()
    } else {
        vec![SyntaxSpan {
            range: 0..code.len(),
            role: SyntaxRole::Plain,
        }]
    }
}

#[inline]
fn now_ms(start_instant: Instant) -> u64 {
    #[cfg(feature = "test-observation")]
    {
        let mock = TEST_MOCK_CLOCK_MS.load(Ordering::Relaxed);
        if mock > 0 {
            return mock;
        }
    }
    start_instant.elapsed().as_millis() as u64
}

/// Parses the code payload into semantic `SyntaxSpan`s using syntect with an optional cancellation flag.
///
/// Implements:
/// - Explicit size/threshold validation:
///   `> 64 KiB`, `> 2000 lines`, or any line `> 1000 unicode scalars` -> returns `HardLimit`.
/// - Multi-line state continuity via syntect `ParseState` and `ScopeStack`.
/// - 50ms soft budget across lines.
/// - Immediate cooperative cancellation between lines.
/// - Full byte coverage with contiguous, non-overlapping `SyntaxSpan`s.
pub fn parse_code_spans_bounded(
    code: &str,
    raw_lang: Option<&str>,
    cancel_flag: Option<&AtomicBool>,
) -> ParseResult {
    if code.is_empty() {
        return ParseResult::Ready(Arc::from([]));
    }

    if cancel_flag.map(|f| f.load(Ordering::Relaxed)).unwrap_or(false) {
        return ParseResult::Cancelled;
    }

    // Threshold check 1: byte length
    if code.len() > MAX_CODE_BLOCK_BYTES {
        return ParseResult::HardLimit;
    }

    // Pre-check line count and line unicode scalar count
    let mut line_count = 0;
    for line in code.split('\n') {
        let stripped = line.strip_suffix('\r').unwrap_or(line);
        if stripped.chars().count() > MAX_LINE_UNICODE_SCALARS {
            return ParseResult::HardLimit;
        }
        line_count += 1;
        if line_count > MAX_CODE_BLOCK_LINES {
            return ParseResult::HardLimit;
        }
    }

    // Canonical language resolution
    let canonical = match resolve_canonical_language(raw_lang) {
        Some(c) => c,
        None => return ParseResult::UnsupportedLanguage,
    };

    let ss = init_syntax_set();
    let syntax = match find_syntax_for_canonical(canonical) {
        Some(s) => s,
        None => return ParseResult::InitFailed,
    };

    let start_instant = Instant::now();
    let start_ms: u64 = now_ms(start_instant);

    #[cfg(feature = "test-observation")]
    {
        let init_adv = TEST_INITIAL_ADVANCE_MS.load(Ordering::Relaxed);
        if init_adv > 0 && TEST_MOCK_CLOCK_MS.load(Ordering::Relaxed) > 0 {
            TEST_MOCK_CLOCK_MS.fetch_add(init_adv, Ordering::Relaxed);
        }
    }

    let mut state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut raw_spans: Vec<SyntaxSpan> = Vec::new();

    let mut line_start_byte = 0;

    // Iterate through lines preserving line endings for accurate regex anchors ($)
    let mut remaining = code;
    while !remaining.is_empty() {
        // Cancellation check between lines
        if cancel_flag.map(|f| f.load(Ordering::Relaxed)).unwrap_or(false) {
            return ParseResult::Cancelled;
        }

        // Check cumulative elapsed time before each line
        let elapsed_ms = now_ms(start_instant).saturating_sub(start_ms);
        if elapsed_ms >= PARSE_SOFT_BUDGET_MS {
            return ParseResult::BudgetExceeded;
        }

        let next_newline = remaining.find('\n');
        let (line_with_end, next_remaining) = match next_newline {
            Some(idx) => (&remaining[..=idx], &remaining[idx + 1..]),
            None => (remaining, ""),
        };

        let ops = match state.parse_line(line_with_end, ss) {
            Ok(ops) => ops,
            Err(_) => return ParseResult::InitFailed,
        };

        #[cfg(feature = "test-observation")]
        {
            let idx = TEST_LINE_COUNT.fetch_add(1, Ordering::Relaxed);
            if idx < TEST_LINE_DELTAS_MS.len() {
                let delta = TEST_LINE_DELTAS_MS[idx].load(Ordering::Relaxed);
                if delta > 0 {
                    TEST_MOCK_CLOCK_MS.fetch_add(delta, Ordering::Relaxed);
                }
            }
        }

        // Check cumulative elapsed time immediately after regex return
        let elapsed_ms = now_ms(start_instant).saturating_sub(start_ms);
        if elapsed_ms >= PARSE_SOFT_BUDGET_MS {
            return ParseResult::BudgetExceeded;
        }

        let mut last_col = 0;
        let mut current_role = scope_stack_to_role(&stack);

        for (col, op) in ops {
            if col > last_col {
                let span_start = line_start_byte + last_col;
                let span_end = line_start_byte + col;
                raw_spans.push(SyntaxSpan {
                    range: span_start..span_end,
                    role: current_role,
                });
                last_col = col;
            }
            let _ = stack.apply(&op);
            current_role = scope_stack_to_role(&stack);
        }

        if last_col < line_with_end.len() {
            let span_start = line_start_byte + last_col;
            let span_end = line_start_byte + line_with_end.len();
            raw_spans.push(SyntaxSpan {
                range: span_start..span_end,
                role: current_role,
            });
        }

        line_start_byte += line_with_end.len();
        remaining = next_remaining;
    }

    ParseResult::Ready(Arc::from(normalize_and_fill_spans(code, raw_spans)))
}

/// Parses the code payload into semantic `SyntaxSpan`s using syntect.
pub fn parse_code_spans(code: &str, raw_lang: Option<&str>) -> Vec<SyntaxSpan> {
    match parse_code_spans_bounded(code, raw_lang, None) {
        ParseResult::Ready(spans) => spans.to_vec(),
        _ => fallback_plain(code),
    }
}

/// Returns the actual usable heap allocation size for a requested size on the current platform.
pub fn heap_usable_size(size: usize) -> usize {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn malloc_good_size(size: usize) -> usize;
        }
        unsafe { malloc_good_size(size) }
    }
    #[cfg(not(target_os = "macos"))]
    {
        (size + 15) & !15
    }
}

// -----------------------------------------------------------------------------
// SyntaxCache (Per-Workspace Instance, 4 MiB budget, LRU with Retained Keys)
// -----------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct CacheEntry {
    pub key: CodeKey,
    pub spans: Arc<[SyntaxSpan]>,
}

#[derive(Clone, Debug)]
pub struct SyntaxCache {
    pub entries: Vec<CacheEntry>,
    pub max_bytes: usize, // 4 * 1024 * 1024 (4 MiB)
}

impl SyntaxCache {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_bytes: MAX_CACHE_BYTES,
        }
    }

    pub fn key(code: &str, raw_lang: Option<&str>) -> CodeKey {
        CodeKey::from_raw(raw_lang, Arc::from(code))
    }

    pub fn heap_usable_size(size: usize) -> usize {
        heap_usable_size(size)
    }

    pub fn entry_heap_bytes(key: &CodeKey, spans: &Arc<[SyntaxSpan]>) -> usize {
        let payload_bytes = heap_usable_size(16 + key.payload.len());
        let spans_bytes = heap_usable_size(16 + spans.len() * std::mem::size_of::<SyntaxSpan>());
        payload_bytes + spans_bytes
    }

    pub fn current_bytes(&self) -> usize {
        let container_bytes = if self.entries.capacity() == 0 {
            0
        } else {
            heap_usable_size(self.entries.capacity() * std::mem::size_of::<CacheEntry>())
        };
        let heap_bytes: usize = self
            .entries
            .iter()
            .map(|e| Self::entry_heap_bytes(&e.key, &e.spans))
            .sum();
        container_bytes + heap_bytes
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    pub fn get(&mut self, key: &CodeKey) -> Option<Arc<[SyntaxSpan]>> {
        if let Some(pos) = self.entries.iter().position(|e| &e.key == key) {
            let entry = self.entries.remove(pos);
            let spans = entry.spans.clone();
            self.entries.push(entry);
            Some(spans)
        } else {
            None
        }
    }

    pub fn get_by_str(&mut self, code: &str, raw_lang: Option<&str>) -> Option<Arc<[SyntaxSpan]>> {
        let key = Self::key(code, raw_lang);
        self.get(&key)
    }

    pub fn insert(
        &mut self,
        key: CodeKey,
        spans: Arc<[SyntaxSpan]>,
        retained_keys: &std::collections::HashSet<CodeKey>,
    ) -> bool {
        let needed_entry_heap = Self::entry_heap_bytes(&key, &spans);
        let min_entry_size = needed_entry_heap + heap_usable_size(std::mem::size_of::<CacheEntry>());

        if min_entry_size > self.max_bytes {
            return false;
        }

        // If key already exists, remove it first
        if let Some(pos) = self.entries.iter().position(|e| e.key == key) {
            self.entries.remove(pos);
        }

        loop {
            let prospective_cap = if self.entries.len() == self.entries.capacity() {
                self.entries.len() + 1
            } else {
                self.entries.capacity()
            };

            let container_bytes = heap_usable_size(prospective_cap * std::mem::size_of::<CacheEntry>());
            let current_heap: usize = self
                .entries
                .iter()
                .map(|e| Self::entry_heap_bytes(&e.key, &e.spans))
                .sum();

            if container_bytes + current_heap + needed_entry_heap <= self.max_bytes {
                if self.entries.len() == self.entries.capacity() {
                    self.entries.reserve_exact(1);
                }
                self.entries.push(CacheEntry { key, spans });
                return true;
            }

            // Evict oldest (front) non-retained entry
            if let Some(evict_idx) = self
                .entries
                .iter()
                .position(|e| !retained_keys.contains(&e.key))
            {
                self.entries.remove(evict_idx);
            } else {
                // Cannot evict any more non-retained entries to fit
                return false;
            }
        }
    }

    pub fn clear(&mut self) {
        self.entries = Vec::new();
    }
}

impl Default for SyntaxCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_aliases_resolution() {
        assert_eq!(resolve_canonical_language(Some("xml")), Some("XML"));
        assert_eq!(resolve_canonical_language(Some("XML")), Some("XML"));
        assert_eq!(resolve_canonical_language(Some("html")), Some("HTML"));
        assert_eq!(resolve_canonical_language(Some("htm")), Some("HTML"));
        assert_eq!(resolve_canonical_language(Some("HTML")), Some("HTML"));
        assert_eq!(resolve_canonical_language(Some("java")), Some("Java"));
        assert_eq!(resolve_canonical_language(Some("Java")), Some("Java"));
        assert_eq!(resolve_canonical_language(Some("python")), Some("Python"));
        assert_eq!(resolve_canonical_language(Some("py")), Some("Python"));
        assert_eq!(resolve_canonical_language(Some("Python")), Some("Python"));
        assert_eq!(resolve_canonical_language(Some("javascript")), Some("JavaScript"));
        assert_eq!(resolve_canonical_language(Some("js")), Some("JavaScript"));
        assert_eq!(resolve_canonical_language(Some("JavaScript")), Some("JavaScript"));
        assert_eq!(resolve_canonical_language(Some("json")), Some("JSON"));
        assert_eq!(resolve_canonical_language(Some("JSON")), Some("JSON"));
        assert_eq!(resolve_canonical_language(Some("yaml")), Some("YAML"));
        assert_eq!(resolve_canonical_language(Some("yml")), Some("YAML"));
        assert_eq!(resolve_canonical_language(Some("YAML")), Some("YAML"));
        assert_eq!(resolve_canonical_language(Some("sql")), Some("SQL"));
        assert_eq!(resolve_canonical_language(Some("SQL")), Some("SQL"));
        assert_eq!(resolve_canonical_language(Some("shell")), Some("Bourne Again Shell (bash)"));
        assert_eq!(resolve_canonical_language(Some("sh")), Some("Bourne Again Shell (bash)"));
        assert_eq!(resolve_canonical_language(Some("bash")), Some("Bourne Again Shell (bash)"));
        assert_eq!(resolve_canonical_language(Some("Bourne Again Shell (bash)")), Some("Bourne Again Shell (bash)"));
        assert_eq!(resolve_canonical_language(Some("Bourne Again Shell (bash),editable")), Some("Bourne Again Shell (bash)"));
        assert_eq!(resolve_canonical_language(Some("Bourne Again Shell (bash) {1-3}")), Some("Bourne Again Shell (bash)"));
        assert_eq!(resolve_canonical_language(Some("rust")), Some("Rust"));
        assert_eq!(resolve_canonical_language(Some("rs")), Some("Rust"));
        assert_eq!(resolve_canonical_language(Some("Rust")), Some("Rust"));

        // With extra info tags: e.g. "rust,editable" or "python {1-3}"
        assert_eq!(resolve_canonical_language(Some("python {1-3}")), Some("Python"));
        assert_eq!(resolve_canonical_language(Some("  js  ")), Some("JavaScript"));

        // Fallbacks
        assert_eq!(resolve_canonical_language(None), None);
        assert_eq!(resolve_canonical_language(Some("")), None);
        assert_eq!(resolve_canonical_language(Some("text")), None);
        assert_eq!(resolve_canonical_language(Some("txt")), None);
        assert_eq!(resolve_canonical_language(Some("plain")), None);
        assert_eq!(resolve_canonical_language(Some("plaintext")), None);
        assert_eq!(resolve_canonical_language(Some("unknown_lang_xyz")), None);
        assert_eq!(resolve_canonical_language(Some("typescript")), None);
        assert_eq!(resolve_canonical_language(Some("toml")), None);
    }

    #[test]
    fn test_all_supported_languages_parse_without_panic() {
        let cases = [
            ("XML", "<root attr=\"val\">text</root>"),
            ("HTML", "<div class=\"foo\">hello</div>"),
            ("Java", "public class Main { public static void main(String[] args) {} }"),
            ("Python", "def hello(name: str) -> str:\n    return f'hi {name}'"),
            ("JavaScript", "function test() { const x = 42; return x; }"),
            ("JSON", "{\n  \"key\": 123,\n  \"arr\": [true, false, null]\n}"),
            ("YAML", "key: value\nlist:\n  - item1\n  - 42"),
            ("SQL", "SELECT id, name FROM users WHERE age > 18 ORDER BY id DESC;"),
            ("Bourne Again Shell (bash)", "#!/bin/bash\necho \"Hello $USER\"\nexit 0"),
            ("Rust", "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}"),
        ];

        for (lang, code) in cases {
            let spans = parse_code_spans(code, Some(lang));
            assert!(!spans.is_empty(), "spans for {} should not be empty", lang);

            // Invariant: contiguous coverage
            let mut curr = 0;
            for span in &spans {
                assert_eq!(span.range.start, curr, "gap or mismatch in {}", lang);
                assert!(span.range.end > span.range.start);
                curr = span.range.end;
            }
            assert_eq!(curr, code.len(), "did not reach end of code in {}", lang);

            // Reconstruct text
            let reconstructed: String = spans.iter().map(|s| &code[s.range.clone()]).collect();
            assert_eq!(reconstructed, code, "reconstructed mismatch for {}", lang);
        }
    }

    #[test]
    fn test_bash_canonical_never_downgraded_to_plain() {
        let bash_code = "echo 'hello' && exit 0";
        // Passing canonical name directly
        let spans_canonical = parse_code_spans(bash_code, Some("Bourne Again Shell (bash)"));
        // Passing raw alias
        let spans_alias = parse_code_spans(bash_code, Some("bash"));
        assert_eq!(spans_canonical, spans_alias);
        // Ensure not downgraded to single Plain span
        let has_non_plain = spans_canonical.iter().any(|s| s.role != SyntaxRole::Plain);
        assert!(has_non_plain, "Bourne Again Shell (bash) must have syntax highlighted tokens");
    }

    #[test]
    fn test_xml_maven_tag_attribute_string_comment_distinction() {
        let xml = r#"<!-- Project config -->
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <groupId>org.apache.maven</groupId>
    <version>3.9.0</version>
</project>"#;

        let spans = parse_code_spans(xml, Some("xml"));
        let has_comment = spans.iter().any(|s| s.role == SyntaxRole::Comment && xml[s.range.clone()].contains("Project config"));
        let has_keyword_tag = spans.iter().any(|s| s.role == SyntaxRole::Keyword && (&xml[s.range.clone()] == "project" || &xml[s.range.clone()] == "groupId"));
        let has_attribute = spans.iter().any(|s| s.role == SyntaxRole::Attribute && &xml[s.range.clone()] == "xmlns");
        let has_string = spans.iter().any(|s| s.role == SyntaxRole::String && xml[s.range.clone()].contains("http://maven.apache.org"));

        assert!(has_comment, "XML comments must be recognized as Comment");
        assert!(has_keyword_tag, "XML tag names must be recognized as Keyword");
        assert!(has_attribute, "XML attributes must be recognized as Attribute");
        assert!(has_string, "XML attribute values must be recognized as String");
    }

    #[test]
    fn test_multiline_comment_and_string_continuity() {
        let rust_code = "/* multi-line\n   comment\n   continues */\nfn main() {}";
        let spans = parse_code_spans(rust_code, Some("rust"));

        let comment_parts: Vec<&SyntaxSpan> = spans.iter().filter(|s| s.role == SyntaxRole::Comment).collect();
        assert!(!comment_parts.is_empty(), "must have comment spans");
        for cp in &comment_parts {
            assert!(rust_code[cp.range.clone()].contains("comment") || rust_code[cp.range.clone()].contains("multi-line") || rust_code[cp.range.clone()].contains("continues"));
        }

        let kw = spans.iter().find(|s| s.role == SyntaxRole::Keyword && &rust_code[s.range.clone()] == "fn");
        assert!(kw.is_some(), "fn must still be Keyword after multiline comment");
    }

    #[test]
    fn test_threshold_downgrades_to_plain() {
        // 1. > 64 KiB
        let huge_code = "let x = 1;\n".repeat(6000);
        assert!(huge_code.len() > MAX_CODE_BLOCK_BYTES);
        let spans_huge = parse_code_spans(&huge_code, Some("rust"));
        assert_eq!(spans_huge.len(), 1);
        assert_eq!(spans_huge[0].role, SyntaxRole::Plain);
        assert_eq!(spans_huge[0].range, 0..huge_code.len());

        // 2. > 2000 lines
        let many_lines = "x = 1\n".repeat(2005);
        let spans_lines = parse_code_spans(&many_lines, Some("python"));
        assert_eq!(spans_lines.len(), 1);
        assert_eq!(spans_lines[0].role, SyntaxRole::Plain);

        // 3. Single line > 1000 unicode scalars
        let long_line = format!("let x = \"{}\";\n", "a".repeat(1005));
        let spans_long = parse_code_spans(&long_line, Some("rust"));
        assert_eq!(spans_long.len(), 1);
        assert_eq!(spans_long[0].role, SyntaxRole::Plain);
    }

    #[test]
    fn test_syntax_cache_instance_methods() {
        let mut cache = SyntaxCache::new();
        let key = SyntaxCache::key("let x = 1;", Some("rust"));
        assert!(cache.get(&key).is_none());
        assert!(cache.get_by_str("let x = 1;", Some("rust")).is_none());

        let spans: Arc<[SyntaxSpan]> = parse_code_spans("let x = 1;", Some("rust")).into();
        let retained = std::collections::HashSet::new();
        let admitted = cache.insert(key.clone(), spans.clone(), &retained);
        assert!(admitted);

        let cached = cache.get(&key).expect("must be in cache");
        assert!(Arc::ptr_eq(&cached, &spans));

        let cached_str = cache.get_by_str("let x = 1;", Some("rust")).expect("get_by_str must work");
        assert!(Arc::ptr_eq(&cached_str, &spans));
    }

    #[test]
    fn test_parse_code_spans_bounded_cancellation() {
        let cancel = AtomicBool::new(true);
        let code = "fn cancel_me() { println!(\"test\"); }";
        let res = parse_code_spans_bounded(code, Some("rust"), Some(&cancel));
        assert_eq!(res, ParseResult::Cancelled);
        assert!(!res.is_ready());
        assert!(res.spans().is_none());
    }

    #[test]
    fn test_parse_code_spans_bounded_results() {
        // Empty
        assert_eq!(parse_code_spans_bounded("", Some("rust"), None), ParseResult::Ready(Arc::from([])));

        // HardLimit - byte size
        let huge = "a".repeat(MAX_CODE_BLOCK_BYTES + 1);
        assert_eq!(parse_code_spans_bounded(&huge, Some("rust"), None), ParseResult::HardLimit);

        // HardLimit - line length
        let long_line = "a".repeat(MAX_LINE_UNICODE_SCALARS + 1);
        assert_eq!(parse_code_spans_bounded(&long_line, Some("rust"), None), ParseResult::HardLimit);

        // UnsupportedLanguage
        assert_eq!(parse_code_spans_bounded("code", Some("unknown_xyz"), None), ParseResult::UnsupportedLanguage);
        assert_eq!(parse_code_spans_bounded("code", None, None), ParseResult::UnsupportedLanguage);

        // Ready
        let ready_res = parse_code_spans_bounded("fn test() {}", Some("rust"), None);
        assert!(ready_res.is_ready());
        assert!(ready_res.spans().is_some());
        assert!(!ready_res.is_empty());
    }

    #[test]
    fn test_syntax_cache_lru_and_retained_keys_eviction() {
        let mut cache = SyntaxCache::new();
        let sample_spans: Arc<[SyntaxSpan]> = Arc::new([SyntaxSpan {
            range: 0..10,
            role: SyntaxRole::Plain,
        }]);

        let key_a = SyntaxCache::key("code_a", Some("rust"));
        let key_b = SyntaxCache::key("code_b", Some("rust"));
        let key_c = SyntaxCache::key("code_c", Some("rust"));

        let heap_a = SyntaxCache::entry_heap_bytes(&key_a, &sample_spans);
        let heap_b = SyntaxCache::entry_heap_bytes(&key_b, &sample_spans);
        let entry_cap_bytes = heap_usable_size(2 * std::mem::size_of::<CacheEntry>());

        // Set max_bytes to hold exactly 2 entries
        cache.max_bytes = entry_cap_bytes + heap_a + heap_b;

        let mut retained = std::collections::HashSet::new();

        // Insert A and B
        assert!(cache.insert(key_a.clone(), sample_spans.clone(), &retained));
        assert!(cache.insert(key_b.clone(), sample_spans.clone(), &retained));
        assert_eq!(cache.entries.len(), 2);

        // Retain A, insert C -> B must be evicted (A is protected even though A was inserted first!)
        retained.insert(key_a.clone());

        assert!(cache.insert(key_c.clone(), sample_spans.clone(), &retained));
        assert_eq!(cache.entries.len(), 2);
        assert!(cache.entries.iter().any(|e| e.key == key_a), "A must be retained in cache");
        assert!(!cache.entries.iter().any(|e| e.key == key_b), "B must have been evicted");
        assert!(cache.entries.iter().any(|e| e.key == key_c), "C must be present");

        // Clear resets everything and zeroes bytes
        cache.clear();
        assert_eq!(cache.current_bytes(), 0);
        assert_eq!(cache.entries.len(), 0);
    }

    #[test]
    fn test_code_key_equality_and_hash_content_based() {
        let p1 = Arc::from("payload");
        let p2 = Arc::from(String::from("payload")); // distinct Arc pointer
        assert!(!Arc::ptr_eq(&p1, &p2));

        let k1 = CodeKey::new(Some("Rust"), p1);
        let k2 = CodeKey::new(Some("Rust"), p2);
        assert_eq!(k1, k2, "CodeKey equality must be content-based");

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut h1 = DefaultHasher::new();
        k1.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        k2.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish(), "CodeKey hash must be content-based");
    }
}
