use zmark_core::mermaid::render_to_svg;

#[test]
fn test_edge_variants_render_to_svg() {
    // 1. Test -- label -->, <--> and -.->|label|
    let diagram = r#"
flowchart TD
    A -- label1 --> B
    B <--> C
    C -.->|label2| D
"#;
    let res = render_to_svg(diagram);
    assert!(
        res.is_ok(),
        "Expected render_to_svg to succeed for edge variants, got: {:?}",
        res.err()
    );
    let svg = res.unwrap();
    assert!(svg.contains("<svg"), "Output does not contain <svg");
    assert!(svg.contains("</svg>"), "Output does not contain </svg>");
}

#[test]
fn test_individual_edge_variants() {
    // Isolated tests for each edge variant
    let d1 = "flowchart TD\n  A -- label --> B";
    let res1 = render_to_svg(d1);
    assert!(res1.is_ok(), "Failed on -- label -->: {:?}", res1.err());
    let svg1 = res1.unwrap();
    assert!(svg1.contains("<svg") && svg1.contains("</svg>"));

    let d2 = "flowchart TD\n  A <--> B";
    let res2 = render_to_svg(d2);
    assert!(res2.is_ok(), "Failed on <-->: {:?}", res2.err());
    let svg2 = res2.unwrap();
    assert!(svg2.contains("<svg") && svg2.contains("</svg>"));

    let d3 = "flowchart TD\n  A -.->|label| B";
    let res3 = render_to_svg(d3);
    assert!(res3.is_ok(), "Failed on -.->|label|: {:?}", res3.err());
    let svg3 = res3.unwrap();
    assert!(svg3.contains("<svg") && svg3.contains("</svg>"));
}

#[test]
fn test_line_stitching_leading_ampersand_and_arrow() {
    // 2. Line stitching with leading & and -> operator
    let diagram = r#"
flowchart TD
    N2_1
    & N2_2 & N2_3 -> N3["3. merge_recall ("]
"#;
    let res = render_to_svg(diagram);
    assert!(
        res.is_ok(),
        "Expected render_to_svg to succeed for line stitching with leading &, got: {:?}",
        res.err()
    );
    let svg = res.unwrap();
    assert!(svg.contains("<svg"), "Output does not contain <svg");
    assert!(svg.contains("</svg>"), "Output does not contain </svg>");
    assert!(
        svg.contains("merge_recall"),
        "SVG should retain label text 'merge_recall': {}",
        svg
    );
}

#[test]
fn test_classdef_and_class_name_with_brackets() {
    // 3. classDef and :::className mixed with ["std::vector<int>"]
    let diagram = r#"
flowchart TD
    classDef code fill:#f9f,stroke:#333
    classDef primary fill:#bbf,stroke:#333
    A["std::vector<int>"]:::code --> B["Normal Node"]:::primary
    B --> C
    class C code
"#;
    let res = render_to_svg(diagram);
    assert!(
        res.is_ok(),
        "Expected render_to_svg to succeed for classDef and class annotations, got: {:?}",
        res.err()
    );
    let svg = res.unwrap();
    assert!(svg.contains("<svg"), "Output does not contain <svg");
    assert!(svg.contains("</svg>"), "Output does not contain </svg>");
    // Node text should retain std::vector<int> (raw or escaped)
    assert!(
        svg.contains("std::vector<int>") || svg.contains("std::vector&lt;int&gt;"),
        "SVG should retain std::vector<int> in node text: {}",
        svg
    );
}

#[test]
fn test_cartesian_product_expansion_with_labels() {
    // 4. Cartesian product expansion (many-to-many and compound with labels)
    let diagram = r#"
flowchart LR
    A & B -->|step 1| C & D
    C & D ==> E
"#;
    let res = render_to_svg(diagram);
    assert!(
        res.is_ok(),
        "Expected render_to_svg to succeed for Cartesian product expansion, got: {:?}",
        res.err()
    );
    let svg = res.unwrap();
    assert!(svg.contains("<svg"), "Output does not contain <svg");
    assert!(svg.contains("</svg>"), "Output does not contain </svg>");
}

#[test]
fn test_labels_containing_ampersand_in_quotes_or_brackets() {
    // 5. Labels containing & inside quotes or brackets
    let diagram = r#"
flowchart TD
    A["User & Password"] & B("Logic (A & B)") --> C{Decision & Result}
"#;
    let res = render_to_svg(diagram);
    assert!(
        res.is_ok(),
        "Expected render_to_svg to succeed for labels containing &, got: {:?}",
        res.err()
    );
    let svg = res.unwrap();
    assert!(svg.contains("<svg"), "Output does not contain <svg");
    assert!(svg.contains("</svg>"), "Output does not contain </svg>");
    assert!(
        svg.contains("User & Password") || svg.contains("User &amp; Password"),
        "SVG should contain 'User & Password': {}",
        svg
    );
    assert!(
        svg.contains("Logic (A & B)") || svg.contains("Logic (A &amp; B)"),
        "SVG should contain 'Logic (A & B)': {}",
        svg
    );
    assert!(
        svg.contains("Decision & Result") || svg.contains("Decision &amp; Result"),
        "SVG should contain 'Decision & Result': {}",
        svg
    );
}

#[test]
fn test_pipe_edge_labels_containing_colons() {
    // 6. Test pipe edge labels containing ::: and :: (e.g. A -->|step:::code| B, A -->|std::vector:::tag| B)
    let diagram = r#"
flowchart TD
    A -->|step:::code| B
    B -->|std::vector:::tag| C
"#;
    let res = render_to_svg(diagram);
    assert!(
        res.is_ok(),
        "Expected render_to_svg to succeed for pipe labels containing colons, got: {:?}",
        res.err()
    );
    let svg = res.unwrap();
    assert!(svg.contains("<svg") && svg.contains("</svg>"));
    assert!(
        svg.contains("step:::code"),
        "SVG should retain pipe label 'step:::code': {}",
        svg
    );
    assert!(
        svg.contains("std::vector:::tag"),
        "SVG should retain pipe label 'std::vector:::tag': {}",
        svg
    );
}

#[test]
fn test_html_br_and_generic_brackets_in_node_and_edge_labels() {
    // 7. Test <br>/<br/> preserved and generic <T> safely escaped in both nodes and pipe labels
    let diagram = r#"
flowchart TD
    A["Node 1<br>Line 2<br/>Generic<T>"] -->|Edge<br/>Text with <int>| B["Node 2"]
"#;
    let res = render_to_svg(diagram);
    assert!(
        res.is_ok(),
        "Expected render_to_svg to succeed with <br> and <T> in node/edge labels, got: {:?}",
        res.err()
    );
    let svg = res.unwrap();
    assert!(svg.contains("<svg") && svg.contains("</svg>"));
    assert!(
        svg.contains("Line 2"),
        "SVG should contain Line 2: {}",
        svg
    );
    assert!(
        svg.contains("&lt;T&gt;") || svg.contains("Generic"),
        "SVG should contain Generic<T>: {}",
        svg
    );
    assert!(
        svg.contains("&lt;int&gt;") || svg.contains("Text with"),
        "SVG should contain Text with <int>: {}",
        svg
    );
}

#[test]
fn test_node_brackets_containing_pipe_and_class_annotation() {
    // 8. Test A[foo|bar]:::code --> B: verify :::code is stripped even when node text contains '|'
    let diagram = r#"
flowchart TD
    A[foo|bar]:::code --> B
"#;
    let res = render_to_svg(diagram);
    assert!(
        res.is_ok(),
        "Expected render_to_svg to succeed for A[foo|bar]:::code, got: {:?}",
        res.err()
    );
    let svg = res.unwrap();
    assert!(svg.contains("<svg") && svg.contains("</svg>"));
    assert!(svg.contains("foo") && svg.contains("bar"));
}

#[test]
fn test_escaped_pipe_in_diagram() {
    // 9. Test diagram with escaped pipe \|
    let diagram = r#"
flowchart TD
    A["Escaped \| Pipe"] -->|pipe with \| inside| B
"#;
    let res = render_to_svg(diagram);
    assert!(
        res.is_ok(),
        "Expected render_to_svg to succeed with escaped pipe, got: {:?}",
        res.err()
    );
    let svg = res.unwrap();
    assert!(svg.contains("<svg") && svg.contains("</svg>"));
}

/// Regression for issue #25: rusty-mermaid-diagrams 0.2.0 sliced strings at byte
/// offsets and panicked on non-ASCII text. The vendored copy must render these.
#[test]
fn non_ascii_sequence_and_state_diagrams_render_instead_of_panicking() {
    let cases = [
        "sequenceDiagram\n  participant 用户\n  用户->>服务: 请求\n  Note over 用户,服务: 备注\n",
        "sequenceDiagram\r\n  participant 用户\r\n  用户->>服务: 请求\r\n",
        "sequenceDiagram\n  Note left of 用户: 说明\n",
        "stateDiagram-v2\n  [*] --> 空闲\n  空闲 --> 运行 : 启动\n",
    ];
    for source in cases {
        let svg = zmark_core::mermaid::render_to_svg(source)
            .unwrap_or_else(|error| panic!("{source:?} failed: {error}"));
        assert!(svg.contains("<svg"));
    }
}
