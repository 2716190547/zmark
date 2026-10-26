use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MermaidShape {
    Rectangle,
    Rounded,
    Diamond,
    Database,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MermaidNode {
    pub id: String,
    pub label: String,
    pub shape: MermaidShape,
    pub class_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MermaidEdge {
    pub from: Vec<String>,
    pub to: Vec<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MermaidDiagram {
    pub direction: String,
    pub nodes: Vec<MermaidNode>,
    pub edges: Vec<MermaidEdge>,
    pub stages: Vec<Vec<MermaidNode>>,
}

impl MermaidDiagram {
    pub fn parse(code: &str) -> Self {
        let mut direction = "TD".to_string();
        let mut nodes: Vec<MermaidNode> = Vec::new();
        let mut edges: Vec<MermaidEdge> = Vec::new();

        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("%%") || trimmed.starts_with("classDef") {
                continue;
            }

            if trimmed.starts_with("flowchart") || trimmed.starts_with("graph") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() > 1 {
                    direction = parts[1].to_string();
                }
                continue;
            }

            // Parse edges (e.g. A --> B, A -->|Yes| B, A -- label --> B, A & B --> C, A --> D & E)
            if trimmed.contains("-->") || trimmed.contains("---") {
                let separator = if trimmed.contains("-->") { "-->" } else { "---" };
                let parts: Vec<&str> = trimmed.split(separator).collect();
                if parts.len() >= 2 {
                    let mut from_part = parts[0].trim();
                    let raw_to = parts[1].trim();

                    let mut edge_label = None;
                    let to_part = if raw_to.starts_with('|') {
                        if let Some(end) = raw_to[1..].find('|') {
                            edge_label = Some(raw_to[1..1 + end].trim().to_string());
                            raw_to[1 + end + 1..].trim()
                        } else {
                            raw_to
                        }
                    } else if from_part.contains("--") {
                        let sub: Vec<&str> = from_part.split("--").collect();
                        if sub.len() >= 2 {
                            from_part = sub[0].trim();
                            edge_label = Some(sub[1].trim().to_string());
                        }
                        raw_to
                    } else {
                        raw_to
                    };

                    let from_ids: Vec<String> = from_part
                        .split('&')
                        .map(|s| s.trim().to_string())
                        .collect();

                    // Parse any node definitions in from_part or to_part
                    for id in &from_ids {
                        extract_and_add_node(id, &mut nodes);
                    }

                    let to_ids: Vec<String> = to_part
                        .split('&')
                        .map(|s| s.trim().to_string())
                        .collect();

                    for id in &to_ids {
                        extract_and_add_node(id, &mut nodes);
                    }

                    edges.push(MermaidEdge {
                        from: from_ids
                            .into_iter()
                            .map(|s| clean_node_id(&s))
                            .collect(),
                        to: to_ids
                            .into_iter()
                            .map(|s| clean_node_id(&s))
                            .collect(),
                        label: edge_label,
                    });
                }
                continue;
            }

            // Standalone node definitions (e.g. S1["..."]:::step)
            extract_and_add_node(trimmed, &mut nodes);
        }

        // Build topological / sequential stages
        let stages = build_stages(&nodes, &edges);

        Self {
            direction,
            nodes,
            edges,
            stages,
        }
    }
}

fn clean_node_id(s: &str) -> String {
    let mut clean = s.trim().to_string();
    if let Some(idx) = clean.find('[') {
        clean = clean[..idx].trim().to_string();
    } else if let Some(idx) = clean.find('{') {
        clean = clean[..idx].trim().to_string();
    } else if let Some(idx) = clean.find('(') {
        clean = clean[..idx].trim().to_string();
    }
    if let Some(idx) = clean.find(":::") {
        clean = clean[..idx].trim().to_string();
    }
    clean
}

fn extract_and_add_node(raw: &str, nodes: &mut Vec<MermaidNode>) {
    let raw = raw.trim();
    if raw.is_empty() {
        return;
    }

    // Check class name (e.g. :::step)
    let (without_class, class_name) = if let Some(idx) = raw.find(":::") {
        (&raw[..idx], Some(raw[idx + 3..].trim().to_string()))
    } else {
        (raw, None)
    };

    let without_class = without_class.trim();

    // Check Database shape: [("label")] or [(label)]
    if let Some(start) = without_class.find("[(") {
        if let Some(end) = without_class.rfind(")]") {
            let id = without_class[..start].trim().to_string();
            let mut label = without_class[start + 2..end].to_string();
            if label.starts_with('"') && label.ends_with('"') && label.len() >= 2 {
                label = label[1..label.len() - 1].to_string();
            }
            upsert_node(nodes, id, label, MermaidShape::Database, class_name);
            return;
        }
    }
    // Check Diamond shape: {"label"} or {label}
    if let Some(start) = without_class.find('{') {
        if let Some(end) = without_class.rfind('}') {
            let id = without_class[..start].trim().to_string();
            let mut label = without_class[start + 1..end].to_string();
            if label.starts_with('"') && label.ends_with('"') && label.len() >= 2 {
                label = label[1..label.len() - 1].to_string();
            }
            upsert_node(nodes, id, label, MermaidShape::Diamond, class_name);
            return;
        }
    }
    // Check Rounded shape: ("label") or (label)
    if let Some(start) = without_class.find('(') {
        if let Some(end) = without_class.rfind(')') {
            let id = without_class[..start].trim().to_string();
            let mut label = without_class[start + 1..end].to_string();
            if label.starts_with('"') && label.ends_with('"') && label.len() >= 2 {
                label = label[1..label.len() - 1].to_string();
            }
            upsert_node(nodes, id, label, MermaidShape::Rounded, class_name);
            return;
        }
    }
    // Check Rectangle shape: ["label"] or [label]
    if let Some(start) = without_class.find('[') {
        if let Some(end) = without_class.rfind(']') {
            let id = without_class[..start].trim().to_string();
            let mut label = without_class[start + 1..end].to_string();
            if label.starts_with('"') && label.ends_with('"') && label.len() >= 2 {
                label = label[1..label.len() - 1].to_string();
            }
            upsert_node(nodes, id, label, MermaidShape::Rectangle, class_name);
            return;
        }
    }

    let clean_id = clean_node_id(without_class);
    if !clean_id.is_empty() && !nodes.iter().any(|n| n.id == clean_id) {
        nodes.push(MermaidNode {
            id: clean_id.clone(),
            label: clean_id,
            shape: MermaidShape::Rectangle,
            class_name,
        });
    }
}

fn upsert_node(
    nodes: &mut Vec<MermaidNode>,
    id: String,
    label: String,
    shape: MermaidShape,
    class_name: Option<String>,
) {
    let clean_id = clean_node_id(&id);
    let label = label.replace("<br/>", " \n ").replace("<br>", " \n ");

    if let Some(existing) = nodes.iter_mut().find(|n| n.id == clean_id) {
        existing.label = label;
        existing.shape = shape;
        if class_name.is_some() {
            existing.class_name = class_name;
        }
    } else {
        nodes.push(MermaidNode {
            id: clean_id,
            label,
            shape,
            class_name,
        });
    }
}

fn build_stages(nodes: &[MermaidNode], edges: &[MermaidEdge]) -> Vec<Vec<MermaidNode>> {
    if nodes.is_empty() {
        return Vec::new();
    }

    let mut stages: Vec<Vec<MermaidNode>> = Vec::new();
    let mut visited = std::collections::HashSet::new();

    // Group nodes based on edge sequence
    // First, find all start nodes (no incoming edges)
    let all_targets: std::collections::HashSet<&str> = edges
        .iter()
        .flat_map(|e| e.to.iter().map(|s| s.as_str()))
        .collect();

    let initial_stage: Vec<MermaidNode> = nodes
        .iter()
        .filter(|n| !all_targets.contains(n.id.as_str()))
        .cloned()
        .collect();

    if !initial_stage.is_empty() {
        for n in &initial_stage {
            visited.insert(n.id.clone());
        }
        stages.push(initial_stage);
    }

    // Follow edges layer by layer
    while visited.len() < nodes.len() {
        let mut next_stage: Vec<MermaidNode> = Vec::new();
        for edge in edges {
            let from_visited = edge.from.iter().all(|f| visited.contains(f));
            if from_visited {
                for to_id in &edge.to {
                    if !visited.contains(to_id) {
                        if let Some(node) = nodes.iter().find(|n| &n.id == to_id) {
                            if !next_stage.iter().any(|n| &n.id == to_id) {
                                next_stage.push(node.clone());
                            }
                        }
                    }
                }
            }
        }

        if next_stage.is_empty() {
            // Pick any remaining nodes to avoid infinite loop
            let remaining: Vec<MermaidNode> = nodes
                .iter()
                .filter(|n| !visited.contains(&n.id))
                .cloned()
                .collect();
            for n in &remaining {
                visited.insert(n.id.clone());
            }
            if !remaining.is_empty() {
                stages.push(remaining);
            }
            break;
        }

        for n in &next_stage {
            visited.insert(n.id.clone());
        }
        stages.push(next_stage);
    }

    stages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_realworld_flowchart() {
        let code = r#"
flowchart TD
    classDef step fill:#E3F2FD,stroke:#1E88E5,stroke-width:1.5px,color:#0D47A1
    classDef run fill:#E8F5E9,stroke:#43A047,stroke-width:1.5px,color:#1B5E20
    classDef check fill:#FFF3E0,stroke:#FB8C00,stroke-width:1.5px,color:#E65100

    S1["1. 启动基础设施: ./docker/start_all.sh"]:::step
    S1 --> D1["Docker 启动: MySQL 8.0, ES 8.x, Kibana, Qdrant"]:::step
    S1 --> D2["Metal 原生加速: 启动 BGE TEI 服务 (:8081)"]:::step

    D1 & D2 --> C1{"2. 自动初始化数据库<br/>(docker-entrypoint-initdb.d)"}:::check
    C1 --> M1[("dw 库: 建表并灌入业务数据<br/>dim_*, fact_order")]:::run
"#;

        let diagram = MermaidDiagram::parse(code);
        assert_eq!(diagram.direction, "TD");
        assert!(diagram.nodes.len() >= 5);

        let s1 = diagram.nodes.iter().find(|n| n.id == "S1").unwrap();
        assert_eq!(s1.shape, MermaidShape::Rectangle);
        assert_eq!(s1.class_name.as_deref(), Some("step"));

        let c1 = diagram.nodes.iter().find(|n| n.id == "C1").unwrap();
        assert_eq!(c1.shape, MermaidShape::Diamond);
        assert_eq!(c1.class_name.as_deref(), Some("check"));

        let m1 = diagram.nodes.iter().find(|n| n.id == "M1").unwrap();
        assert_eq!(m1.shape, MermaidShape::Database);
        assert_eq!(m1.class_name.as_deref(), Some("run"));

        assert!(!diagram.stages.is_empty());
    }

    #[test]
    fn test_parse_edge_labels_and_unquoted_shapes() {
        let code = r#"
graph TD
    Start(Start Process) --> Decision{Is Authorized?}
    Decision -->|Yes| DB[(Primary DB)]
    Decision -->|No| Reject[Reject Request]
"#;
        let diagram = MermaidDiagram::parse(code);
        let start = diagram.nodes.iter().find(|n| n.id == "Start").unwrap();
        assert_eq!(start.shape, MermaidShape::Rounded);

        let dec = diagram.nodes.iter().find(|n| n.id == "Decision").unwrap();
        assert_eq!(dec.shape, MermaidShape::Diamond);

        let db = diagram.nodes.iter().find(|n| n.id == "DB").unwrap();
        assert_eq!(db.shape, MermaidShape::Database);

        let yes_edge = diagram.edges.iter().find(|e| e.label.as_deref() == Some("Yes")).unwrap();
        assert_eq!(yes_edge.from, vec!["Decision".to_string()]);
        assert_eq!(yes_edge.to, vec!["DB".to_string()]);

        let no_edge = diagram.edges.iter().find(|e| e.label.as_deref() == Some("No")).unwrap();
        assert_eq!(no_edge.from, vec!["Decision".to_string()]);
        assert_eq!(no_edge.to, vec!["Reject".to_string()]);
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
    let raw_svg = rusty_mermaid::to_svg(&sanitized, &theme).map_err(|e| format!("{:?}", e))?;
    Ok(normalize_svg_bounds(&raw_svg))
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

fn sanitize_mermaid(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    for line in code.lines() {
        let trimmed = line.trim();
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
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

