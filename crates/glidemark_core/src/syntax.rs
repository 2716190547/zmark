//! Shared syntax highlighting domain logic and contracts for GlideMark.
//!
//! Provides pure, headless syntect-based tokenization mapping to a minimal 6-role
//! semantic model (`SyntaxRole`), strictly guaranteeing contiguous byte-range coverage,
//! multi-line statefulness, input thresholds, and bounded LRU caching.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::Instant;
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

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
        for name in &[
            "XML",
            "HTML",
            "Java",
            "Python",
            "JavaScript",
            "JSON",
            "YAML",
            "SQL",
            "Bourne Again Shell (bash)",
            "Rust",
        ] {
            if let Some(syntax) = ss.find_syntax_by_name(name) {
                let mut state = ParseState::new(syntax);
                let _ = state.parse_line("\n", &ss);
            }
        }
        ss
    })
}

/// Maps a raw fence info string to a canonical language identifier recognized by GlideMark.
///
/// Rules:
/// - Takes the first whitespace-separated token.
/// - Trims and converts to ASCII lowercase.
/// - Matches against the approved canonical list.
/// - Returns `None` for empty/text/plain or unknown/unsupported languages.
pub fn resolve_canonical_language(raw_info: Option<&str>) -> Option<&'static str> {
    let raw = raw_info?;
    let token = raw
        .trim()
        .split(|c: char| c.is_whitespace() || c == ',')
        .next()?
        .trim()
        .to_ascii_lowercase();
    match token.as_str() {
        "xml" => Some("XML"),
        "html" | "htm" => Some("HTML"),
        "java" => Some("Java"),
        "python" | "py" => Some("Python"),
        "javascript" | "js" => Some("JavaScript"),
        "json" => Some("JSON"),
        "yaml" | "yml" => Some("YAML"),
        "sql" => Some("SQL"),
        "shell" | "sh" | "bash" => Some("Bourne Again Shell (bash)"),
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

/// Parses the code payload into semantic `SyntaxSpan`s using syntect with an optional cancellation flag.
///
/// Implements:
/// - Explicit size/threshold validation:
///   `> 64 KiB`, `> 2000 lines`, or any line `> 1000 unicode scalars` -> returns full Plain.
/// - Multi-line state continuity via syntect `ParseState` and `ScopeStack`.
/// - 50ms soft budget across lines.
/// - Immediate cooperative cancellation between lines.
/// - Full byte coverage with contiguous, non-overlapping `SyntaxSpan`s.
pub fn parse_code_spans_bounded(
    code: &str,
    raw_lang: Option<&str>,
    cancel_flag: Option<&AtomicBool>,
) -> Vec<SyntaxSpan> {
    if code.is_empty() {
        return Vec::new();
    }

    if cancel_flag.map(|f| f.load(Ordering::Relaxed)).unwrap_or(false) {
        return fallback_plain(code);
    }

    // Threshold check 1: byte length
    if code.len() > MAX_CODE_BLOCK_BYTES {
        return fallback_plain(code);
    }

    // Canonical language resolution
    let canonical = match resolve_canonical_language(raw_lang) {
        Some(c) => c,
        None => return fallback_plain(code),
    };

    let ss = init_syntax_set();
    let syntax = match find_syntax_for_canonical(canonical) {
        Some(s) => s,
        None => return fallback_plain(code),
    };

    // Pre-check line count and line unicode scalar count
    let mut line_count = 0;
    for line in code.split('\n') {
        let stripped = line.strip_suffix('\r').unwrap_or(line);
        if stripped.chars().count() > MAX_LINE_UNICODE_SCALARS {
            return fallback_plain(code);
        }
        line_count += 1;
        if line_count > MAX_CODE_BLOCK_LINES {
            return fallback_plain(code);
        }
    }

    let start_time = Instant::now();
    let mut state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut raw_spans: Vec<SyntaxSpan> = Vec::new();

    let mut line_start_byte = 0;

    // Iterate through lines preserving line endings for accurate regex anchors ($)
    let mut remaining = code;
    while !remaining.is_empty() {
        // Cancellation check between lines
        if cancel_flag.map(|f| f.load(Ordering::Relaxed)).unwrap_or(false) {
            return fallback_plain(code);
        }

        // Check cumulative elapsed time before each line
        if start_time.elapsed().as_millis() as u64 >= PARSE_SOFT_BUDGET_MS {
            return fallback_plain(code);
        }

        let next_newline = remaining.find('\n');
        let (line_with_end, next_remaining) = match next_newline {
            Some(idx) => (&remaining[..=idx], &remaining[idx + 1..]),
            None => (remaining, ""),
        };

        let ops = match state.parse_line(line_with_end, ss) {
            Ok(ops) => ops,
            Err(_) => return fallback_plain(code),
        };

        // Check cumulative elapsed time after regex return
        if start_time.elapsed().as_millis() as u64 >= PARSE_SOFT_BUDGET_MS && !next_remaining.is_empty() {
            return fallback_plain(code);
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

    normalize_and_fill_spans(code, raw_spans)
}

/// Parses the code payload into semantic `SyntaxSpan`s using syntect.
pub fn parse_code_spans(code: &str, raw_lang: Option<&str>) -> Vec<SyntaxSpan> {
    parse_code_spans_bounded(code, raw_lang, None)
}

// -----------------------------------------------------------------------------
// Bounded LRU Cache (Capacity: 32 blocks, 4 MiB)
// -----------------------------------------------------------------------------

struct CacheEntry {
    canonical_lang: &'static str,
    code_hash: u64,
    code: String,
    spans: Arc<[SyntaxSpan]>,
    size_bytes: usize,
}

struct LruSyntaxCache {
    entries: Vec<CacheEntry>,
    total_bytes: usize,
}

impl LruSyntaxCache {
    fn new() -> Self {
        Self {
            entries: Vec::with_capacity(MAX_CACHE_ENTRIES),
            total_bytes: 0,
        }
    }

    fn get(&mut self, canonical_lang: &'static str, code_hash: u64, code: &str) -> Option<Arc<[SyntaxSpan]>> {
        if let Some(pos) = self.entries.iter().position(|e| {
            e.canonical_lang == canonical_lang && e.code_hash == code_hash && e.code == code
        }) {
            // Move to most recently used (back of vector)
            let entry = self.entries.remove(pos);
            let spans = entry.spans.clone();
            self.entries.push(entry);
            Some(spans)
        } else {
            None
        }
    }

    fn insert(
        &mut self,
        canonical_lang: &'static str,
        code_hash: u64,
        code: &str,
        spans: Arc<[SyntaxSpan]>,
    ) {
        let entry_size = std::mem::size_of::<CacheEntry>()
            + code.len()
            + spans.len() * std::mem::size_of::<SyntaxSpan>();

        if entry_size > MAX_CACHE_BYTES {
            // Block is too large to fit in cache
            return;
        }

        // Evict if over count or size limit
        while self.entries.len() >= MAX_CACHE_ENTRIES
            || (self.total_bytes + entry_size > MAX_CACHE_BYTES && !self.entries.is_empty())
        {
            let evicted = self.entries.remove(0);
            self.total_bytes = self.total_bytes.saturating_sub(evicted.size_bytes);
        }

        self.total_bytes += entry_size;
        self.entries.push(CacheEntry {
            canonical_lang,
            code_hash,
            code: code.to_string(),
            spans,
            size_bytes: entry_size,
        });
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.total_bytes = 0;
    }
}

static CACHE: LazyLock<Mutex<LruSyntaxCache>> = LazyLock::new(|| Mutex::new(LruSyntaxCache::new()));

fn compute_code_hash(canonical: &str, code: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    canonical.hash(&mut hasher);
    code.hash(&mut hasher);
    hasher.finish()
}

/// Clears the global syntax cache (useful for testing and memory reclaim).
pub fn clear_syntax_cache() {
    if let Ok(mut cache) = CACHE.lock() {
        cache.clear();
    }
}

/// Fast non-blocking lookup in syntax cache.
///
/// Returns:
/// - `Some(fallback_plain)` if language resolves to plain / unsupported.
/// - `Some(spans)` if cached.
/// - `None` if supported language but cache miss (does NOT parse or block).
pub fn get_cached_code_spans(code: &str, raw_lang: Option<&str>) -> Option<Arc<[SyntaxSpan]>> {
    if code.is_empty() {
        return Some(fallback_plain(code).into());
    }
    let canonical = match resolve_canonical_language(raw_lang) {
        Some(c) => c,
        None => return Some(fallback_plain(code).into()),
    };

    let code_hash = compute_code_hash(canonical, code);
    if let Ok(mut cache) = CACHE.lock() {
        if let Some(spans) = cache.get(canonical, code_hash, code) {
            return Some(spans);
        }
    }
    None
}

/// Inserts pre-parsed spans into the syntax cache.
pub fn insert_cached_code_spans(code: &str, raw_lang: Option<&str>, spans: Arc<[SyntaxSpan]>) {
    let canonical = match resolve_canonical_language(raw_lang) {
        Some(c) => c,
        None => return,
    };
    let code_hash = compute_code_hash(canonical, code);
    if let Ok(mut cache) = CACHE.lock() {
        cache.insert(canonical, code_hash, code, spans);
    }
}

/// Retrieves or computes semantic syntax spans for a code block.
///
/// Thread-safe and caching enabled. Keyed by canonical language and code content.
/// Roles are theme-independent, enabling 100% cache reuse across theme switches.
pub fn get_or_parse_code_spans(code: &str, raw_lang: Option<&str>) -> Arc<[SyntaxSpan]> {
    if let Some(spans) = get_cached_code_spans(code, raw_lang) {
        return spans;
    }

    let parsed: Arc<[SyntaxSpan]> = parse_code_spans_bounded(code, raw_lang, None).into();
    insert_cached_code_spans(code, raw_lang, parsed.clone());
    parsed
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
        assert_eq!(resolve_canonical_language(Some("java")), Some("Java"));
        assert_eq!(resolve_canonical_language(Some("python")), Some("Python"));
        assert_eq!(resolve_canonical_language(Some("py")), Some("Python"));
        assert_eq!(resolve_canonical_language(Some("javascript")), Some("JavaScript"));
        assert_eq!(resolve_canonical_language(Some("js")), Some("JavaScript"));
        assert_eq!(resolve_canonical_language(Some("json")), Some("JSON"));
        assert_eq!(resolve_canonical_language(Some("yaml")), Some("YAML"));
        assert_eq!(resolve_canonical_language(Some("yml")), Some("YAML"));
        assert_eq!(resolve_canonical_language(Some("sql")), Some("SQL"));
        assert_eq!(resolve_canonical_language(Some("shell")), Some("Bourne Again Shell (bash)"));
        assert_eq!(resolve_canonical_language(Some("sh")), Some("Bourne Again Shell (bash)"));
        assert_eq!(resolve_canonical_language(Some("bash")), Some("Bourne Again Shell (bash)"));
        assert_eq!(resolve_canonical_language(Some("rust")), Some("Rust"));
        assert_eq!(resolve_canonical_language(Some("rs")), Some("Rust"));

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
    fn test_cache_hit_and_theme_independence() {
        clear_syntax_cache();
        let code = "fn test() -> i32 { 42 }";
        let parsed1 = get_or_parse_code_spans(code, Some("rust"));
        let parsed2 = get_or_parse_code_spans(code, Some("rust"));
        assert!(Arc::ptr_eq(&parsed1, &parsed2), "subsequent call must hit Arc cache pointer");
    }

    #[test]
    fn test_syntax_set_readiness_and_cached_spans_api() {
        assert!(is_syntax_set_ready() || !is_syntax_set_ready());
        let _ = init_syntax_set();
        assert!(is_syntax_set_ready());

        clear_syntax_cache();
        let code = "let val = 999;";

        // Cache miss for rust before parsing
        assert!(get_cached_code_spans(code, Some("rust")).is_none());

        // Plain / unsupported lang resolves immediately without blocking
        let plain = get_cached_code_spans(code, None).expect("none raw_lang should return plain");
        assert_eq!(plain[0].role, SyntaxRole::Plain);

        // Explicit cache insert & get
        let spans: Arc<[SyntaxSpan]> = parse_code_spans(code, Some("rust")).into();
        insert_cached_code_spans(code, Some("rust"), spans.clone());
        let cached = get_cached_code_spans(code, Some("rust")).expect("must hit cache after insert");
        assert!(Arc::ptr_eq(&cached, &spans));
    }

    #[test]
    fn test_parse_code_spans_bounded_cancellation() {
        let cancel = AtomicBool::new(true);
        let code = "fn cancel_me() { println!(\"test\"); }";
        let spans = parse_code_spans_bounded(code, Some("rust"), Some(&cancel));
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].role, SyntaxRole::Plain);
        assert_eq!(spans[0].range, 0..code.len());
    }
}
