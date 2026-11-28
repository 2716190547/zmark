#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statement_terminators_are_stripped_but_entities_stay() {
        assert_eq!(strip_statement_terminator("    classDef a fill:#fff,color:#000;"), "    classDef a fill:#fff,color:#000");
        assert_eq!(strip_statement_terminator("A --> B; ;  "), "A --> B");
        assert_eq!(strip_statement_terminator("A[\"x &amp;\"]"), "A[\"x &amp;\"]");
        assert_eq!(strip_statement_terminator("A[x &amp;];"), "A[x &amp;]");
        assert_eq!(strip_statement_terminator("A[x &amp;"), "A[x &amp;");
        assert_eq!(strip_statement_terminator("A --> B"), "A --> B");
    }

    #[test]
    fn trailing_semicolons_render_instead_of_failing_to_parse() {
        let code = "graph TB\n    classDef a fill:#E1F5FE,stroke:#0288D1,font-weight:bold;\n    A[\"x\"]:::a --> B;\n    style B fill:#fff;\n";
        render_to_svg(code).expect("semicolon-terminated statements should render");
    }

    #[test]
    fn test_render_mermaid_to_svg() {
        let code = r#"
graph TD
    Start(Start Process) --> Decision{Is Authorized?}
    Decision -->|Yes| DB[(Primary DB)]
    Decision -->|No| Reject[Reject Request]
"#;
        let svg = render_to_svg(code).expect("should render svg");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));

        let dark_svg = render_to_svg_themed(code, true).expect("should render dark svg");
        assert!(dark_svg.contains("<svg"));
    }

    #[test]
    fn test_render_complex_flowchart_with_subgraphs() {
        let code = r#"
flowchart TD
    subgraph S1["1. 前端交互层"]
        A[Vue 3 问答页] --> B[SSE 流接收器] --> C[状态灯与表格]
    end

    subgraph S2["2. 后端服务层"]
        D[FastAPI 路由] --> E[QueryService] --> F[LangGraph 状态机]
    end

    subgraph S3["3. 存储与模型底座"]
        G[(MySQL (dw + meta))]
        H[(Qdrant + ES 索引)]
        I[DeepSeek + BGE 模型]
    end

    F <-->|SSE 实时流| S1
    F <-->|检索 / 推理 / 查数| S3
"#;
        let svg = render_to_svg(code).expect("should render svg");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("1. 前端交互层"));
        assert!(svg.contains("MySQL (dw + meta)"));
        assert!(!svg.contains("&quot;1. 前端交互层&quot;"));

        let dark_svg = render_to_svg_themed(code, true).expect("should render dark svg");
        assert!(dark_svg.contains("<svg"));
        assert!(dark_svg.contains("#1e1e2e"));
    }

    #[test]
    fn test_normalize_svg_bounds_eliminates_empty_void() {
        let code = r#"
flowchart LR
    subgraph FE["1. 前端交互层"]
        direction TB
        UI["Vue 3 问答页"] --> SSE["SSE 流接收器"] --> Table["状态灯与表格"]
    end

    subgraph BE["2. 后端服务层"]
        direction TB
        API["FastAPI 路由"] --> Svc["QueryService"] --> Graph["LangGraph 状态机"]
    end

    subgraph Infra["3. 存储与模型底座"]
        direction TB
        MySQL[("MySQL (dw + meta)")]
        Vec[("Qdrant + ES 索引")]
        AI["DeepSeek + BGE 模型"]
    end

    FE <== "SSE 实时流" ==> BE
    BE <== "检索 / 推理 / 查数" ==> Infra
"#;
        let svg = render_to_svg(code).expect("should render svg");
        // Verify that the normalized SVG does NOT retain the bloated 1300+ px height
        assert!(svg.contains("<svg"));
        // Extract height attribute
        let height_attr = extract_attr(&svg, "height=").expect("height attr");
        let height: f64 = height_attr.parse().expect("parse height");
        assert!(height < 400.0, "SVG height should be compact (<400px), but was {}", height);
    }

    #[test]
    fn test_apply_texture_budget_clamps_dimensions() {
        // Test 1: Wide diagram (width > max_dimension)
        let raw_wide = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 3602.72 1539.60" width="3602.72" height="1539.60"><g></g></svg>"#;
        let budgeted_wide = apply_texture_budget(raw_wide, 1000.0);
        let w: f64 = extract_attr(&budgeted_wide, "width=").unwrap().parse().unwrap();
        let h: f64 = extract_attr(&budgeted_wide, "height=").unwrap().parse().unwrap();
        assert!((w - 1000.0).abs() < 0.1);
        assert!((h - 427.34).abs() < 0.5);
        // viewBox must be unchanged to maintain vector resolution
        assert!(budgeted_wide.contains(r#"viewBox="0 0 3602.72 1539.60""#));

        // Test 2: Tall diagram (height > max_dimension)
        let raw_tall = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600.00 2400.00" width="600.00" height="2400.00"><g></g></svg>"#;
        let budgeted_tall = apply_texture_budget(raw_tall, 1000.0);
        let w_tall: f64 = extract_attr(&budgeted_tall, "width=").unwrap().parse().unwrap();
        let h_tall: f64 = extract_attr(&budgeted_tall, "height=").unwrap().parse().unwrap();
        assert!((w_tall - 250.0).abs() < 0.1);
        assert!((h_tall - 1000.0).abs() < 0.1);
        assert!(budgeted_tall.contains(r#"viewBox="0 0 600.00 2400.00""#));

        // Test 3: Diagram within budget (both <= max_dimension)
        let raw_small = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400.00 300.00" width="400.00" height="300.00"><g></g></svg>"#;
        let budgeted_small = apply_texture_budget(raw_small, 1000.0);
        let w_small: f64 = extract_attr(&budgeted_small, "width=").unwrap().parse().unwrap();
        let h_small: f64 = extract_attr(&budgeted_small, "height=").unwrap().parse().unwrap();
        assert_eq!(w_small, 400.0);
        assert_eq!(h_small, 300.0);
        assert!(budgeted_small.contains(r#"viewBox="0 0 400.00 300.00""#));
    }

    #[test]
    fn test_non_ascii_state_diagram_renders_and_malformed_input_errors() {
        // rusty_mermaid 0.2.0 panicked on this (byte slicing inside a CJK char);
        // the vendored parser advances by chars, so it must render now.
        let state_diagram = r#"
stateDiagram-v2
    [*] --> 状态1
    状态1 --> [*]
"#;
        let svg = render_to_svg_themed(state_diagram, false).expect("CJK state ids render");
        assert!(svg.contains("<svg"));

        // Malformed diagram syntax returns error without panicking
        let invalid = "not_a_valid_mermaid_diagram\n123 -> 456";
        let res_invalid = render_to_svg(invalid);
        assert!(res_invalid.is_err());
    }

    #[test]
    fn test_scale_factor_to_bucket() {
        assert_eq!(scale_factor_to_bucket(1.0), 2);
        assert_eq!(scale_factor_to_bucket(1.25), 3);
        assert_eq!(scale_factor_to_bucket(1.5), 3);
        assert_eq!(scale_factor_to_bucket(2.0), 4);
        assert_eq!(scale_factor_to_bucket(2.5), 5);
        assert_eq!(scale_factor_to_bucket(3.0), 6);
        // Out of bounds / invalid fallback
        assert_eq!(scale_factor_to_bucket(0.0), 2);
        assert_eq!(scale_factor_to_bucket(-1.5), 2);
        assert_eq!(scale_factor_to_bucket(f32::NAN), 2);
        assert_eq!(scale_factor_to_bucket(f32::INFINITY), 2);
        assert_eq!(scale_factor_to_bucket(4.0), 6);
    }

    #[test]
    fn test_calculate_hidpi_dimensions_multi_scale_and_budget_guards() {
        let scales = [1.0f32, 1.25f32, 1.5f32, 2.0f32, 3.0f32];
        for &sf in &scales {
            let (tw, th) = calculate_hidpi_dimensions(1000.0, 600.0, sf);
            let pw = tw * GPUI_SVG_DECODER_SCALE;
            let ph = th * GPUI_SVG_DECODER_SCALE;
            assert!(pw * ph <= 16_000_000.0 + 1e-4, "pixel budget exceeded at {}x", sf);
            assert!(pw.max(ph) <= 4000.0 + 1e-4, "max side exceeded at {}x", sf);
            assert!(tw >= 1.0 && th >= 1.0);
        }
    }

    #[test]
    fn small_lightbox_targets_follow_dpi_without_filling_the_budget() {
        assert_eq!(calculate_hidpi_dimensions(500.0, 500.0, 1.0), (250.0, 250.0));
        assert_eq!(calculate_hidpi_dimensions(500.0, 500.0, 2.0), (500.0, 500.0));
        assert_eq!(calculate_hidpi_dimensions(500.0, 500.0, 3.0), (750.0, 750.0));
    }

    #[test]
    fn test_calculate_hidpi_dimensions_invalid_and_edge_inputs() {
        // Negative / zero / NaN dimensions fallback safely to (1000.0, 600.0)
        let (tw, th) = calculate_hidpi_dimensions(-100.0, 500.0, 2.0);
        assert!(tw > 0.0 && th > 0.0);
        let (tw0, th0) = calculate_hidpi_dimensions(0.0, 500.0, 2.0);
        assert_eq!((tw, th), (tw0, th0));
        let (twnan, thnan) = calculate_hidpi_dimensions(f64::NAN, 500.0, 2.0);
        assert_eq!((tw, th), (twnan, thnan));
        let (twinf, thinf) = calculate_hidpi_dimensions(f64::INFINITY, 500.0, 2.0);
        assert_eq!((tw, th), (twinf, thinf));

        // Negative / zero / NaN scale_factor safely fallback to 1.0
        let (tw_sf_neg, th_sf_neg) = calculate_hidpi_dimensions(800.0, 600.0, -1.0);
        let (tw_sf_1, th_sf_1) = calculate_hidpi_dimensions(800.0, 600.0, 1.0);
        assert_eq!((tw_sf_neg, th_sf_neg), (tw_sf_1, th_sf_1));

        let (tw_sf_nan, _) = calculate_hidpi_dimensions(800.0, 600.0, f32::NAN);
        assert_eq!(tw_sf_nan, tw_sf_1);
    }

    #[test]
    fn test_calculate_hidpi_dimensions_huge_diagrams() {
        // 10000x8000 diagram at scale 2.0
        let (tw, th) = calculate_hidpi_dimensions(10000.0, 8000.0, 2.0);
        let pw = tw * GPUI_SVG_DECODER_SCALE;
        let ph = th * GPUI_SVG_DECODER_SCALE;
        assert!(pw * ph <= 16_000_000.0 + 1e-4);
        assert!(pw.max(ph) <= 4000.0 + 1e-4);

        // Ultra wide diagram (30000x1000)
        let (uw_w, uw_h) = calculate_hidpi_dimensions(30000.0, 1000.0, 2.0);
        let upw = uw_w * GPUI_SVG_DECODER_SCALE;
        let uph = uw_h * GPUI_SVG_DECODER_SCALE;
        assert!(upw * uph <= 16_000_000.0 + 1e-4);
        assert!(upw <= 4000.0 + 1e-4);
    }

    #[test]
    fn test_extract_svg_dimensions() {
        // Standard width and height
        let svg1 = r#"<svg width="800" height="600" viewBox="0 0 800 600"><g></g></svg>"#;
        assert_eq!(extract_svg_dimensions(svg1), (800.0, 600.0));

        // Floating point and px unit
        let svg2 = r#"<svg width="1200.5px" height="800.25px"><g></g></svg>"#;
        assert_eq!(extract_svg_dimensions(svg2), (1200.5, 800.25));

        // Attributes with other prefixes like stroke-width
        let svg3 = r#"<svg stroke-width="2" width="500" height="300"><g></g></svg>"#;
        assert_eq!(extract_svg_dimensions(svg3), (500.0, 300.0));

        // Width and height with % fallback to viewBox
        let svg_pct = r#"<svg width="100%" height="100%" viewBox="0 0 1920 1080"><g></g></svg>"#;
        assert_eq!(extract_svg_dimensions(svg_pct), (1920.0, 1080.0));

        // ViewBox only (comma or whitespace separated)
        let svg_vb_space = r#"<svg viewBox="0 0 750 450"><g></g></svg>"#;
        assert_eq!(extract_svg_dimensions(svg_vb_space), (750.0, 450.0));
        let svg_vb_comma = r#"<svg viewBox="0, 0, 750, 450"><g></g></svg>"#;
        assert_eq!(extract_svg_dimensions(svg_vb_comma), (750.0, 450.0));

        // Fallback for missing or invalid
        let svg_empty = r#"<svg><g></g></svg>"#;
        assert_eq!(extract_svg_dimensions(svg_empty), (1000.0, 600.0));
        assert_eq!(extract_svg_dimensions("not an svg"), (1000.0, 600.0));
    }

    #[test]
    fn test_apply_texture_pixel_budget() {
        // Case 1: Standard SVG (replace width/height, preserve viewBox)
        let svg_std = r#"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600" viewBox="0 0 800 600"><path d="M0,0 L10,10"/></svg>"#;
        let res_std = apply_texture_pixel_budget(svg_std, 1600.0, 1200.0);
        assert!(res_std.contains(r#"width="1600.00""#));
        assert!(res_std.contains(r#"height="1200.00""#));
        assert!(res_std.contains(r#"viewBox="0 0 800 600""#));
        assert!(res_std.contains(r#"<path d="M0,0 L10,10"/>"#));

        // Case 2: width="100%" and height="100%"
        let svg_pct = r#"<svg width="100%" height="100%" viewBox="0 0 500 300"><circle cx="5" cy="5" r="5"/></svg>"#;
        let res_pct = apply_texture_pixel_budget(svg_pct, 1000.0, 600.0);
        assert!(res_pct.contains(r#"width="1000.00""#));
        assert!(res_pct.contains(r#"height="600.00""#));
        assert!(!res_pct.contains("100%"));
        assert!(res_pct.contains(r#"viewBox="0 0 500 300""#));

        // Case 3: Only viewBox (missing width & height)
        let svg_vb = r#"<svg viewBox="0 0 400 300"><rect x="0" y="0"/></svg>"#;
        let res_vb = apply_texture_pixel_budget(svg_vb, 800.0, 600.0);
        assert!(res_vb.contains(r#"width="800.00""#));
        assert!(res_vb.contains(r#"height="600.00""#));
        assert!(res_vb.contains(r#"viewBox="0 0 400 300""#));
        assert!(res_vb.contains(r#"<rect x="0" y="0"/>"#));

        // Case 4: Missing viewBox (synthesize viewBox from original dimensions)
        let svg_no_vb = r#"<svg width="500" height="250"><g></g></svg>"#;
        let res_no_vb = apply_texture_pixel_budget(svg_no_vb, 1000.0, 500.0);
        assert!(res_no_vb.contains(r#"viewBox="0 0 500.00 250.00""#));
        assert!(res_no_vb.contains(r#"width="1000.00""#));
        assert!(res_no_vb.contains(r#"height="500.00""#));

        // Case 5: Missing all (synthesize viewBox from target dimensions)
        let svg_none = r#"<svg><g></g></svg>"#;
        let res_none = apply_texture_pixel_budget(svg_none, 1200.0, 800.0);
        assert!(res_none.contains(r#"viewBox="0 0 1200.00 800.00""#));
        assert!(res_none.contains(r#"width="1200.00""#));
        assert!(res_none.contains(r#"height="800.00""#));
    }
}

struct EdgeMatch {
    start: usize,
    end: usize,
    edge_str: String,
}

/// Mermaid accepts `;` as a statement terminator (`classDef a fill:#fff;`, `A --> B;`),
/// but the parser rejects it. Drops trailing `;` and spaces from a line, keeping the `;`
/// of an HTML entity such as `&amp;` that a label may end with.
fn strip_statement_terminator(line: &str) -> String {
    let mut end = line.trim_end().len();
    while line[..end].ends_with(';') {
        let body = &line[..end - 1];
        let is_entity = body.rfind('&').is_some_and(|amp| {
            let name = &body[amp + 1..];
            !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '#')
        });
        if is_entity {
            break;
        }
        end = body.trim_end().len();
    }
    line[..end].to_string()
}

fn is_whitelisted(trimmed: &str) -> bool {
    trimmed.starts_with("%%")
        || trimmed.starts_with("classDef ")
        || trimmed == "classDef"
        || trimmed.starts_with("class ")
        || trimmed == "class"
        || trimmed.starts_with("style ")
        || trimmed.starts_with("linkStyle ")
        || trimmed.starts_with("direction ")
        || trimmed.starts_with("subgraph ")
        || trimmed == "subgraph"
        || trimmed.starts_with("flowchart ")
        || trimmed == "flowchart"
        || trimmed.starts_with("graph ")
        || trimmed == "graph"
        || trimmed.starts_with("sequenceDiagram")
        || trimmed.starts_with("classDiagram")
        || trimmed.starts_with("stateDiagram")
        || trimmed.starts_with("erDiagram")
        || trimmed == "end"
}

fn is_line_ending_with_edge_operator(s: &str) -> bool {
    let t = s.trim_end();
    let t = if let Some(stripped) = t.strip_suffix('|') {
        if let Some(open) = stripped.rfind('|') {
            stripped[..open].trim_end()
        } else {
            t
        }
    } else {
        t
    };

    t.ends_with("-->")
        || t.ends_with("---")
        || t.ends_with("-.->")
        || t.ends_with("-.-")
        || t.ends_with("==>")
        || t.ends_with("===")
        || t.ends_with("<-->")
        || t.ends_with("->")
        || t.ends_with("<-")
        || t.ends_with("<->")
}

fn stitch_lines(code: &str) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();

    for line in code.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            result.push(line.to_string());
            continue;
        }

        if is_whitelisted(trimmed) {
            result.push(line.to_string());
            continue;
        }

        let mut stitched = false;
        if let Some(prev_idx) = result.iter().rposition(|l| !l.trim().is_empty()) {
            let prev_trimmed = result[prev_idx].trim();
            if !is_whitelisted(prev_trimmed) {
                let starts_with_amp = trimmed.starts_with('&');
                let starts_with_edge = try_match_edge(trimmed).is_some();
                let prev_ends_with_arrow = is_line_ending_with_edge_operator(&result[prev_idx]);

                if starts_with_amp || starts_with_edge || prev_ends_with_arrow {
                    let prev_trimmed_end = result[prev_idx].trim_end();
                    result[prev_idx] = format!("{} {}", prev_trimmed_end, trimmed);
                    stitched = true;
                }
            }
        }

        if !stitched {
            result.push(line.to_string());
        }
    }

    result
}

fn handle_tag_bracket(c: char, rem_chars: &[char], out: &mut String) {
    if c == '<' {
        let rem: String = rem_chars.iter().take(6).collect();
        let rem_lower = rem.to_lowercase();
        if rem_lower.starts_with("<br>")
            || rem_lower.starts_with("<br/>")
            || rem_lower.starts_with("<br />")
        {
            out.push(c);
        } else {
            out.push_str("&lt;");
        }
    } else if c == '>' {
        if out.ends_with("<br") || out.ends_with("<br/") || out.ends_with("<br ") {
            out.push(c);
        } else {
            out.push_str("&gt;");
        }
    } else {
        out.push(c);
    }
}

fn strip_class_annotations(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_quotes = false;
    let mut in_pipe_label = false;
    let mut bracket_depth = 0usize;
    let mut escaped = false;

    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        let is_escaped = escaped;
        escaped = false;

        if c == '\\' {
            if !is_escaped {
                escaped = true;
            }
            out.push(c);
            i += 1;
            continue;
        }

        if c == '"' {
            if !escaped {
                in_quotes = !in_quotes;
            }
            out.push(c);
            i += 1;
            continue;
        }

        if in_quotes {
            handle_tag_bracket(c, &chars[i..], &mut out);
            i += 1;
            continue;
        }

        // Outside quotes: pipe label toggle ONLY when unescaped and at root bracket depth
        if c == '|' && bracket_depth == 0 && !is_escaped {
            in_pipe_label = !in_pipe_label;
            out.push(c);
            i += 1;
            continue;
        }

        if in_pipe_label {
            if is_escaped && c == '|' {
                if out.ends_with('\\') {
                    out.pop();
                    out.push_str("&#124;");
                } else {
                    out.push(c);
                }
                i += 1;
                continue;
            }
            handle_tag_bracket(c, &chars[i..], &mut out);
            i += 1;
            continue;
        }

        // Outside quotes and outside pipe labels
        match c {
            '[' | '(' | '{' => {
                bracket_depth += 1;
                out.push(c);
                i += 1;
            }
            ']' | ')' | '}' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                out.push(c);
                i += 1;
            }
            '<' | '>' if bracket_depth > 0 => {
                handle_tag_bracket(c, &chars[i..], &mut out);
                i += 1;
            }
            ':' if bracket_depth == 0 => {
                if i + 2 < chars.len() && chars[i + 1] == ':' && chars[i + 2] == ':' {
                    let mut id_len = 0;
                    while i + 3 + id_len < chars.len() {
                        let ch = chars[i + 3 + id_len];
                        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                            id_len += 1;
                        } else {
                            break;
                        }
                    }

                    if id_len > 0 {
                        out.push(' ');
                        i += 3 + id_len;
                        continue;
                    }
                }
                out.push(c);
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }

    out
}

fn attach_pipe_label(rem: &str, arrow_len: usize, norm_arrow: &str) -> Option<(usize, String)> {
    let after = &rem[arrow_len..];
    let trimmed_start = after.trim_start();
    if let Some(rest) = trimmed_start.strip_prefix('|') {
        let ws_len = after.len() - trimmed_start.len();
        let mut close_idx = None;
        let mut prev = '\0';
        for (i, c) in rest.char_indices() {
            if c == '|' && prev != '\\' {
                close_idx = Some(i);
                break;
            }
            prev = c;
        }

        if let Some(c_idx) = close_idx {
            let pipe_label_len = 1 + c_idx + 1;
            let pipe_label_str = &trimmed_start[..pipe_label_len];
            let total_len = arrow_len + ws_len + pipe_label_len;
            return Some((total_len, format!("{}{}", norm_arrow, pipe_label_str)));
        }
    }

    Some((arrow_len, norm_arrow.to_string()))
}

fn match_inline_label(rem: &str, prefix: &str, suffix: &str) -> Option<(usize, String)> {
    if !rem.starts_with(prefix) {
        return None;
    }
    let rest = &rem[prefix.len()..];
    let mut in_quotes = false;
    let mut bracket_depth = 0usize;
    let mut escaped = false;

    let mut i = 0;
    while i < rest.len() {
        let c = rest[i..].chars().next()?;
        let c_len = c.len_utf8();

        if c == '\\' {
            escaped = !escaped;
            i += c_len;
            continue;
        }

        if c == '"' {
            if !escaped {
                in_quotes = !in_quotes;
            }
            escaped = false;
            i += c_len;
            continue;
        }

        escaped = false;

        if !in_quotes {
            match c {
                '[' | '(' | '{' => {
                    bracket_depth += 1;
                }
                ']' | ')' | '}' => {
                    bracket_depth = bracket_depth.saturating_sub(1);
                }
                _ => {}
            }

            if bracket_depth == 0 && rest[i..].starts_with(suffix) {
                let total_len = prefix.len() + i + suffix.len();
                return Some((total_len, rem[..total_len].to_string()));
            }
        }

        i += c_len;
    }

    None
}

fn try_match_edge(rem: &str) -> Option<(usize, String)> {
    if rem.starts_with('<') {
        if rem.starts_with("<==>") {
            return attach_pipe_label(rem, 4, "<==>");
        }
        if rem.starts_with("<==") {
            if let Some((len, full)) = match_inline_label(rem, "<==", "==>") {
                return Some((len, full));
            }
        }
        if rem.starts_with("<-.->") {
            return attach_pipe_label(rem, 5, "<-.->");
        }
        if rem.starts_with("<-->") {
            return attach_pipe_label(rem, 4, "<-->");
        }
        if rem.starts_with("<--") {
            if let Some((len, full)) = match_inline_label(rem, "<--", "-->") {
                return Some((len, full));
            }
            return attach_pipe_label(rem, 3, "<--");
        }
        if rem.starts_with("<->") {
            return attach_pipe_label(rem, 3, "<-->");
        }
        if rem.starts_with("<-") {
            return attach_pipe_label(rem, 2, "<--");
        }
        return None;
    }

    if rem.starts_with('=') {
        if rem.starts_with("==>") {
            return attach_pipe_label(rem, 3, "==>");
        }
        if rem.starts_with("===") {
            return attach_pipe_label(rem, 3, "===");
        }
        if rem.starts_with("==") {
            if let Some((len, full)) = match_inline_label(rem, "==", "==>") {
                return Some((len, full));
            }
            if let Some((len, full)) = match_inline_label(rem, "==", "===") {
                return Some((len, full));
            }
        }
        return None;
    }

    if rem.starts_with("-.") {
        if rem.starts_with("-.->") {
            return attach_pipe_label(rem, 4, "-.->");
        }
        if rem.starts_with("-.-") {
            return attach_pipe_label(rem, 3, "-.-");
        }
        if let Some((len, full)) = match_inline_label(rem, "-.", ".->") {
            return Some((len, full));
        }
        return None;
    }

    if rem.starts_with("--") {
        if rem.starts_with("-->") {
            return attach_pipe_label(rem, 3, "-->");
        }
        if rem.starts_with("---") {
            return attach_pipe_label(rem, 3, "---");
        }
        if let Some((len, full)) = match_inline_label(rem, "--", "-->") {
            return Some((len, full));
        }
        if let Some((len, full)) = match_inline_label(rem, "--", "---") {
            return Some((len, full));
        }
        return None;
    }

    if rem.starts_with("->") {
        return attach_pipe_label(rem, 2, "-->");
    }

    None
}

fn split_node_group(group: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut bracket_depth = 0usize;
    let mut escaped = false;

    for c in group.chars() {
        if c == '\\' {
            escaped = !escaped;
            current.push(c);
            continue;
        }

        if c == '"' {
            if !escaped {
                in_quotes = !in_quotes;
            }
            escaped = false;
            current.push(c);
            continue;
        }

        escaped = false;

        if in_quotes {
            current.push(c);
            continue;
        }

        match c {
            '[' | '(' | '{' => {
                bracket_depth += 1;
                current.push(c);
            }
            ']' | ')' | '}' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                current.push(c);
            }
            '&' if bracket_depth == 0 => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
                current.clear();
            }
            _ => {
                current.push(c);
            }
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }

    if parts.is_empty() && !group.trim().is_empty() {
        parts.push(group.trim().to_string());
    }

    parts
}

fn desugar_edges(line: &str) -> Vec<String> {
    let indent_len = line.len() - line.trim_start().len();
    let indent = &line[..indent_len];
    let content = line[indent_len..].trim_end();

    if content.is_empty() {
        return vec![line.to_string()];
    }

    let mut matches: Vec<EdgeMatch> = Vec::new();
    let mut in_quotes = false;
    let mut bracket_depth = 0usize;
    let mut escaped = false;

    let mut pos = 0;
    while pos < content.len() {
        if !in_quotes && bracket_depth == 0 {
            if let Some((match_len, normalized_edge)) = try_match_edge(&content[pos..]) {
                matches.push(EdgeMatch {
                    start: pos,
                    end: pos + match_len,
                    edge_str: normalized_edge,
                });
                pos += match_len;
                continue;
            }
        }

        let c = match content[pos..].chars().next() {
            Some(ch) => ch,
            None => break,
        };
        let c_len = c.len_utf8();

        if c == '\\' {
            escaped = !escaped;
            pos += c_len;
            continue;
        }

        if c == '"' {
            if !escaped {
                in_quotes = !in_quotes;
            }
            escaped = false;
            pos += c_len;
            continue;
        }

        escaped = false;

        if !in_quotes {
            match c {
                '[' | '(' | '{' => bracket_depth += 1,
                ']' | ')' | '}' => bracket_depth = bracket_depth.saturating_sub(1),
                _ => {}
            }
        }

        pos += c_len;
    }

    if matches.is_empty() {
        return vec![line.trim_end().to_string()];
    }

    let mut raw_groups = Vec::new();
    let mut prev_end = 0;
    for m in &matches {
        raw_groups.push(&content[prev_end..m.start]);
        prev_end = m.end;
    }
    raw_groups.push(&content[prev_end..]);

    let node_groups: Vec<Vec<String>> = raw_groups
        .into_iter()
        .map(split_node_group)
        .collect();

    let mut result = Vec::new();
    let default_empty = vec!["".to_string()];

    for i in 0..matches.len() {
        let from_nodes = if node_groups[i].is_empty() {
            &default_empty
        } else {
            &node_groups[i]
        };

        let to_nodes = if node_groups[i + 1].is_empty() {
            &default_empty
        } else {
            &node_groups[i + 1]
        };

        let edge = &matches[i].edge_str;

        for from in from_nodes {
            for to in to_nodes {
                let line_str = match (from.is_empty(), to.is_empty()) {
                    (true, true) => format!("{}{}", indent, edge),
                    (true, false) => format!("{}{} {}", indent, edge, to),
                    (false, true) => format!("{}{} {}", indent, from, edge),
                    (false, false) => format!("{}{} {} {}", indent, from, edge, to),
                };
                result.push(line_str);
            }
        }
    }

    result
}

/// Desugars Mermaid diagram code into standard syntax accepted by rusty_mermaid.
///
/// Steps:
/// 1. Line stitching preprocessor (& prefix or trailing arrow continuation).
/// 2. Whitelist bypass for metadata/styles/subgraphs (with subgraph unquoting).
/// 3. Class annotation stripping (`:::className`).
/// 4. Edge extraction, arrow normalization, and Cartesian product expansion for `&`.
pub fn sanitize_mermaid(code: &str) -> String {
    let stitched = stitch_lines(code);
    let mut out = String::with_capacity(code.len());

    for line in stitched {
        let line = strip_statement_terminator(&line);
        let trimmed = line.trim();

        if trimmed.is_empty() {
            out.push_str(&line);
            out.push('\n');
            continue;
        }

        // Subgraph title unquoting (Requirement 1 & 3)
        if trimmed.starts_with("subgraph ") {
            if let Some(open) = line.find("[\"") {
                if let Some(close) = line.rfind("\"]") {
                    if close > open + 1 {
                        let prefix = &line[..open + 1];
                        let inner = &line[open + 2..close];
                        let suffix = &line[close + 1..];
                        out.push_str(prefix);
                        out.push_str(inner);
                        out.push_str(suffix);
                        out.push('\n');
                        continue;
                    }
                }
            }
            out.push_str(&line);
            out.push('\n');
            continue;
        }

        // Whitelist bypass (Requirement 3)
        if is_whitelisted(trimmed) {
            out.push_str(&line);
            out.push('\n');
            continue;
        }

        // Class annotation stripping (Requirement 4)
        let stripped = strip_class_annotations(&line);

        // Edge operator extraction & Cartesian Product Expansion (Requirement 5)
        let desugared_lines = desugar_edges(&stripped);
        for dl in desugared_lines {
            out.push_str(&dl);
            out.push('\n');
        }
    }

    out
}

/// Renders a Mermaid diagram source to a standard, high-quality SVG vector graphic in pure Rust.
pub fn render_to_svg(code: &str) -> Result<String, String> {
    render_to_svg_themed(code, false)
}

/// Renders a Mermaid diagram source to SVG tailored for light or dark backgrounds in 100% pure Rust.
pub fn render_to_svg_themed(code: &str, is_dark: bool) -> Result<String, String> {
    let sanitized = sanitize_mermaid(code);
    let theme = if is_dark {
        rusty_mermaid::Theme::dark()
    } else {
        rusty_mermaid::Theme::light()
    };

    let render_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rusty_mermaid::to_svg(&sanitized, &theme)
    }));

    let raw_svg = match render_result {
        Ok(Ok(svg)) => svg,
        Ok(Err(e)) => return Err(format!("Mermaid render error: {:?}", e)),
        Err(_) => {
            return Err("Mermaid render error: diagram parsing panicked or is unsupported".to_string());
        }
    };

    let normalized = normalize_svg_bounds(&raw_svg);
    Ok(apply_texture_budget(&normalized, 1000.0))
}

/// Clamps the default width and height of an SVG to a texture budget (max_dimension),
/// while preserving the full viewBox coordinate fidelity and aspect ratio.
pub fn apply_texture_budget(svg: &str, max_dimension: f64) -> String {
    let header_start = match svg.find("<svg") {
        Some(idx) => idx,
        None => return svg.to_string(),
    };
    let header_end = match svg[header_start..].find('>') {
        Some(idx) => header_start + idx + 1,
        None => return svg.to_string(),
    };
    let header = &svg[header_start..header_end];

    let w_str = extract_attr(header, "width=");
    let h_str = extract_attr(header, "height=");

    if let (Some(w_val), Some(h_val)) = (w_str, h_str) {
        if let (Ok(w), Ok(h)) = (w_val.parse::<f64>(), h_val.parse::<f64>()) {
            let max_side = w.max(h);
            if max_side > max_dimension && max_side > 0.0 {
                let ratio = max_dimension / max_side;
                let budgeted_w = w * ratio;
                let budgeted_h = h * ratio;

                let mut new_header = header.to_string();
                new_header = replace_attr(&new_header, "width=", &format!("{:.2}", budgeted_w));
                new_header = replace_attr(&new_header, "height=", &format!("{:.2}", budgeted_h));

                return format!("{}{}{}", &svg[..header_start], new_header, &svg[header_end..]);
            }
        }
    }

    svg.to_string()
}

fn replace_attr(header: &str, attr_prefix: &str, new_value: &str) -> String {
    if let Some(idx) = header.find(attr_prefix) {
        let quote_start = idx + attr_prefix.len();
        if let Some(quote_char) = header.as_bytes().get(quote_start).map(|&b| b as char) {
            if quote_char == '"' || quote_char == '\'' {
                let rest = &header[quote_start + 1..];
                if let Some(quote_end) = rest.find(quote_char) {
                    let end_pos = quote_start + 1 + quote_end + 1;
                    return format!(
                        "{}{}{}{}{}{}",
                        &header[..idx],
                        attr_prefix,
                        quote_char,
                        new_value,
                        quote_char,
                        &header[end_pos..]
                    );
                }
            }
        }
    }
    header.to_string()
}

pub fn normalize_svg_bounds(svg: &str) -> String {
    let g_start_tag = match svg.find("<g transform=\"translate(") {
        Some(idx) => idx,
        None => return svg.to_string(),
    };
    let g_tag_end = match svg[g_start_tag..].find('>') {
        Some(idx) => g_start_tag + idx + 1,
        None => return svg.to_string(),
    };
    let g_close = match svg.rfind("</g>") {
        Some(idx) => idx,
        None => return svg.to_string(),
    };
    if g_close <= g_tag_end {
        return svg.to_string();
    }

    let inner = &svg[g_tag_end..g_close];
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    // Scan rects: <rect x=".." y=".." width=".." height=".."
    for part in inner.split("<rect ") {
        if let (Some(x_val), Some(y_val), Some(w_val), Some(h_val)) = (
            extract_attr(part, "x="),
            extract_attr(part, "y="),
            extract_attr(part, "width="),
            extract_attr(part, "height="),
        ) {
            if !w_val.contains('%') {
                if let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
                    x_val.parse::<f64>(),
                    y_val.parse::<f64>(),
                    w_val.parse::<f64>(),
                    h_val.parse::<f64>(),
                ) {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x + w);
                    max_y = max_y.max(y + h);
                }
            }
        }
    }

    // Scan text: <text x=".." y=".."
    for part in inner.split("<text ") {
        if let (Some(x_val), Some(y_val)) = (extract_attr(part, "x="), extract_attr(part, "y=")) {
            if let (Ok(x), Ok(y)) = (x_val.parse::<f64>(), y_val.parse::<f64>()) {
                min_x = min_x.min(x - 20.0);
                min_y = min_y.min(y - 12.0);
                max_x = max_x.max(x + 20.0);
                max_y = max_y.max(y + 12.0);
            }
        }
    }

    if !min_x.is_finite() || !min_y.is_finite() || max_x <= min_x || max_y <= min_y {
        return svg.to_string();
    }

    let padding = 20.0;
    let content_w = max_x - min_x;
    let content_h = max_y - min_y;
    let new_w = content_w + padding * 2.0;
    let new_h = content_h + padding * 2.0;
    let new_tx = padding - min_x;
    let new_ty = padding - min_y;

    let header_end = match svg.find('>') {
        Some(idx) => idx + 1,
        None => return svg.to_string(),
    };
    let new_header = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {:.2} {:.2}" width="{:.2}" height="{:.2}">"#,
        new_w, new_h, new_w, new_h
    );

    let prefix_before_g = &svg[header_end..g_start_tag];
    let new_g_tag = format!(r#"<g transform="translate({:.2}, {:.2})">"#, new_tx, new_ty);
    let suffix = &svg[g_tag_end..];

    format!("{}{}{}{}", new_header, prefix_before_g, new_g_tag, suffix)
}

fn extract_attr<'a>(s: &'a str, attr_prefix: &str) -> Option<&'a str> {
    let idx = s.find(attr_prefix)?;
    let quote_start = idx + attr_prefix.len();
    let quote_char = *s.as_bytes().get(quote_start)? as char;
    if quote_char != '"' && quote_char != '\'' {
        return None;
    }
    let rest = &s[quote_start + 1..];
    let quote_end = rest.find(quote_char)?;
    Some(&rest[..quote_end])
}

/// Maps a window scale factor (e.g. 1.0, 1.25, 2.0) into discrete bucket IDs.
/// Bucketing avoids cache thrashing across minute resolution fluctuations.
pub fn scale_factor_to_bucket(scale_factor: f32) -> u8 {
    let sf = if scale_factor <= 0.0 || !scale_factor.is_finite() {
        1.0
    } else {
        scale_factor
    };
    (sf.clamp(1.0, 3.0) * 2.0).round() as u8
}

/// GPUI 0.2.2's `ImageAssetLoader` rasterizes SVGs via Resvg at `SMOOTH_SVG_SCALE_FACTOR = 2.0`.
pub const GPUI_SVG_DECODER_SCALE: f64 = 2.0;

/// Calculates HiDPI logical target dimensions subject to a strict 16MP / 4000px physical pixel budget
/// for the actual decoded `RenderImage` produced by GPUI's SVG decoder.
pub fn calculate_hidpi_dimensions(orig_w: f64, orig_h: f64, scale_factor: f32) -> (f64, f64) {
    let sf = if scale_factor <= 0.0 || !scale_factor.is_finite() {
        1.0
    } else {
        scale_factor
    };

    let (w, h) = if orig_w <= 0.0 || orig_h <= 0.0 || !orig_w.is_finite() || !orig_h.is_finite() {
        (1000.0, 600.0)
    } else {
        (orig_w, orig_h)
    };

    let effective_scale = scale_factor_to_bucket(sf) as f64 / 2.0;
    // The budget is a ceiling, not a target. GPUI decodes this SVG at a fixed 2x.
    let wanted_w = w * effective_scale;
    let wanted_h = h * effective_scale;
    const MAX_PHYSICAL_PIXELS: f64 = 16_000_000.0;
    const MAX_PHYSICAL_SIDE: f64 = 4000.0;
    let ratio = (MAX_PHYSICAL_PIXELS / (wanted_w * wanted_h)).sqrt()
        .min(MAX_PHYSICAL_SIDE / wanted_w.max(wanted_h)).min(1.0);
    let target_w = (wanted_w * ratio / GPUI_SVG_DECODER_SCALE).floor().max(1.0);
    let target_h = (wanted_h * ratio / GPUI_SVG_DECODER_SCALE).floor().max(1.0);
    let decoded_w = target_w * GPUI_SVG_DECODER_SCALE;
    let decoded_h = target_h * GPUI_SVG_DECODER_SCALE;

    assert!(
        decoded_w * decoded_h <= MAX_PHYSICAL_PIXELS + 1e-4,
        "decoded physical pixels {} exceed 16MP budget",
        decoded_w * decoded_h
    );
    assert!(
        decoded_w.max(decoded_h) <= MAX_PHYSICAL_SIDE + 1e-4,
        "decoded physical max side {} exceeds 4000 budget",
        decoded_w.max(decoded_h)
    );

    (target_w, target_h)
}

/// Inspects root `<svg ...>` tag and extracts width and height, falling back to viewBox or default (1000.0, 600.0).
pub fn extract_svg_dimensions(svg: &str) -> (f64, f64) {
    let root_tag = match find_root_svg_tag(svg) {
        Some((_, _, tag)) => tag,
        None => return (1000.0, 600.0),
    };

    let w_attr = get_tag_attr(root_tag, "width");
    let h_attr = get_tag_attr(root_tag, "height");

    if let (Some(w_str), Some(h_str)) = (w_attr, h_attr) {
        if !w_str.contains('%') && !h_str.contains('%') {
            let w_val = w_str.trim().trim_end_matches("px").trim().parse::<f64>();
            let h_val = h_str.trim().trim_end_matches("px").trim().parse::<f64>();
            if let (Ok(w), Ok(h)) = (w_val, h_val) {
                if w > 0.0 && h > 0.0 && w.is_finite() && h.is_finite() {
                    return (w, h);
                }
            }
        }
    }

    // Fallback to viewBox
    let vb_attr = get_tag_attr(root_tag, "viewBox").or_else(|| get_tag_attr(root_tag, "viewbox"));
    if let Some(vb) = vb_attr {
        let parts: Vec<&str> = vb
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() == 4 {
            let vb_w = parts[2].trim().parse::<f64>();
            let vb_h = parts[3].trim().parse::<f64>();
            if let (Ok(w), Ok(h)) = (vb_w, vb_h) {
                if w > 0.0 && h > 0.0 && w.is_finite() && h.is_finite() {
                    return (w, h);
                }
            }
        }
    }

    (1000.0, 600.0)
}

/// Rewrites root `<svg ...>` dimensions to target_w and target_h while strictly preserving
/// vector viewBox coordinate fidelity and inner vector graphics.
pub fn apply_texture_pixel_budget(svg: &str, target_w: f64, target_h: f64) -> String {
    let (tag_start, tag_end, root_tag) = match find_root_svg_tag(svg) {
        Some(res) => res,
        None => return svg.to_string(),
    };

    let orig_num_w = get_tag_attr(root_tag, "width").and_then(|val| {
        if val.contains('%') {
            None
        } else {
            val.trim().trim_end_matches("px").trim().parse::<f64>().ok()
        }
    });
    let orig_num_h = get_tag_attr(root_tag, "height").and_then(|val| {
        if val.contains('%') {
            None
        } else {
            val.trim().trim_end_matches("px").trim().parse::<f64>().ok()
        }
    });

    let has_viewbox = find_attr_range(root_tag, "viewBox").is_some()
        || find_attr_range(root_tag, "viewbox").is_some();

    let mut new_tag = root_tag.to_string();

    // Preserve viewBox intact if present; synthesize if missing
    if !has_viewbox {
        let vb_val = if let (Some(w), Some(h)) = (orig_num_w, orig_num_h) {
            if w > 0.0 && h > 0.0 && w.is_finite() && h.is_finite() {
                format!("0 0 {:.2} {:.2}", w, h)
            } else {
                format!("0 0 {:.2} {:.2}", target_w, target_h)
            }
        } else {
            format!("0 0 {:.2} {:.2}", target_w, target_h)
        };
        new_tag = set_or_insert_attr(&new_tag, "viewBox", &vb_val);
    }

    // Set or replace width and height
    new_tag = set_or_insert_attr(&new_tag, "width", &format!("{:.2}", target_w));
    new_tag = set_or_insert_attr(&new_tag, "height", &format!("{:.2}", target_h));

    format!("{}{}{}", &svg[..tag_start], new_tag, &svg[tag_end..])
}

fn find_root_svg_tag(svg: &str) -> Option<(usize, usize, &str)> {
    let start = svg.find("<svg").or_else(|| svg.find("<SVG"))?;
    let after = &svg[start..];
    let end_rel = find_tag_end(after)?;
    let end = start + end_rel + 1;
    Some((start, end, &svg[start..end]))
}

fn find_tag_end(s: &str) -> Option<usize> {
    let mut in_quote: Option<char> = None;
    for (i, c) in s.char_indices() {
        match in_quote {
            Some(q) => {
                if c == q {
                    in_quote = None;
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    in_quote = Some(c);
                } else if c == '>' {
                    return Some(i);
                }
            }
        }
    }
    None
}

fn get_tag_attr<'a>(tag: &'a str, attr_name: &str) -> Option<&'a str> {
    let (start, end) = find_attr_range(tag, attr_name)?;
    let span = &tag[start..end];
    let eq_idx = span.find('=')?;
    let after_eq = span[eq_idx + 1..].trim();
    if after_eq.len() >= 2 {
        let first = after_eq.as_bytes()[0] as char;
        let last = after_eq.as_bytes()[after_eq.len() - 1] as char;
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return Some(&after_eq[1..after_eq.len() - 1]);
        }
    }
    None
}

fn find_attr_range(tag: &str, attr_name: &str) -> Option<(usize, usize)> {
    let mut search_idx = 0;
    while let Some(rel_idx) = tag[search_idx..].find(attr_name) {
        let idx = search_idx + rel_idx;
        let valid_prefix = if idx == 0 {
            true
        } else {
            let prev = tag.as_bytes()[idx - 1];
            prev.is_ascii_whitespace() || prev == b'<'
        };

        if valid_prefix {
            let after_name = idx + attr_name.len();
            let rest = &tag[after_name..];
            let trimmed_rest = rest.trim_start();
            let ws_len = rest.len() - trimmed_rest.len();
            if let Some(after_eq) = trimmed_rest.strip_prefix('=') {
                let trimmed_after_eq = after_eq.trim_start();
                let ws_after_eq = after_eq.len() - trimmed_after_eq.len();
                if let Some(&quote) = trimmed_after_eq.as_bytes().first() {
                    if quote == b'"' || quote == b'\'' {
                        let val_start = 1;
                        if let Some(val_end) = trimmed_after_eq[val_start..].find(quote as char) {
                            let total_end = after_name
                                + ws_len
                                + 1
                                + ws_after_eq
                                + 1
                                + val_end
                                + 1;
                            return Some((idx, total_end));
                        }
                    }
                }
            }
        }
        search_idx = idx + attr_name.len();
    }
    None
}

fn set_or_insert_attr(tag: &str, attr_name: &str, new_value: &str) -> String {
    let range = find_attr_range(tag, attr_name)
        .or_else(|| find_attr_range(tag, &attr_name.to_lowercase()));
    if let Some((start, end)) = range {
        format!("{}{}=\"{}\"{}", &tag[..start], attr_name, new_value, &tag[end..])
    } else if tag.starts_with("<svg") || tag.starts_with("<SVG") {
        let prefix_len = 4;
        format!("<svg {}=\"{}\"{}", attr_name, new_value, &tag[prefix_len..])
    } else {
        tag.to_string()
    }
}

