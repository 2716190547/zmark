//! Reference implementation of the plan's flatten rule, to check that the
//! four pinned copy expectations and three boundary expectations follow from the stated rule.

#[derive(Debug, Clone)]
enum B {
    Para(String),
    Code(String),
    Quote(Vec<B>),
    List(Vec<Item>),
}

#[derive(Debug, Clone)]
struct Item {
    checked: Option<bool>,
    ordered: Option<usize>,
    body: Vec<B>,
}

#[derive(Default)]
struct Flat {
    text: String,
    // (child path, char range in text)
    segs: Vec<(String, std::ops::Range<usize>)>,
}

fn push_ensuring_single_lf(out: &mut String) {
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
}

fn indent_text(s: &str) -> String {
    // list continuation indent: every line of the child gets two spaces
    let mut o = String::new();
    let mut first = true;
    for line in s.split_inclusive('\n') {
        if !first {
            // split_inclusive keeps the LF in the previous piece
        }
        first = false;
        o.push_str("  ");
        o.push_str(line);
    }
    o
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

fn flatten(b: &B, path: &str) -> Flat {
    match b {
        B::Para(t) => Flat {
            text: t.clone(),
            segs: vec![(path.to_string(), 0..char_len(t))],
        },
        B::Code(t) => Flat {
            text: t.clone(),
            segs: vec![(path.to_string(), 0..char_len(t))],
        },
        B::Quote(children) => {
            let mut out = String::new();
            let mut segs = Vec::new();
            for (i, c) in children.iter().enumerate() {
                let child = flatten(c, &format!("{}.{}", path, i));
                if child.text.is_empty() {
                    segs.extend(child.segs);
                    continue;
                }
                push_ensuring_single_lf(&mut out);
                let start = char_len(&out);
                out.push_str(&child.text);
                let shift = start;
                segs.extend(
                    child
                        .segs
                        .into_iter()
                        .map(|(p, r)| (p, r.start + shift..r.end + shift)),
                );
            }
            Flat { text: out, segs }
        }
        B::List(items) => {
            let mut out = String::new();
            let mut segs = Vec::new();
            for (idx, item) in items.iter().enumerate() {
                push_ensuring_single_lf(&mut out);
                let marker = if item.checked.is_some() {
                    format!(
                        "- [{}] ",
                        if item.checked == Some(true) {
                            "x"
                        } else {
                            " "
                        }
                    )
                } else if let Some(n) = item.ordered {
                    format!("{}. ", n)
                } else {
                    "- ".to_string()
                };
                let _item_start = char_len(&out);
                out.push_str(&marker);
                for (ci, child) in item.body.iter().enumerate() {
                    let child_flat = flatten(child, &format!("{}.{}.{}", path, idx, ci));
                    if child_flat.text.is_empty() {
                        segs.extend(child_flat.segs.into_iter().map(|(p, _r)| {
                            let s = char_len(&out);
                            (p, s..s)
                        }));
                        continue;
                    }
                    if ci > 0 {
                        push_ensuring_single_lf(&mut out);
                    }
                    let start = char_len(&out);
                    if ci == 0 {
                        // first child: first line sits after the marker, continuation lines
                        // take the container indent ("续行")
                        let mut pieces = child_flat.text.split_inclusive('\n').peekable();
                        let mut first_piece = true;
                        while let Some(piece) = pieces.next() {
                            if first_piece {
                                out.push_str(piece);
                                first_piece = false;
                            } else {
                                out.push_str("  ");
                                out.push_str(piece);
                            }
                        }
                    } else {
                        out.push_str(&indent_text(&child_flat.text));
                    }
                    let total = char_len(&out) - start;
                    segs.push((format!("{}.{}.{}.all", path, idx, ci), start..start + total));
                    segs.extend(
                        child_flat
                            .segs
                            .into_iter()
                            .map(|(p, r)| (p, r.start + start..r.end + start)),
                    );
                }
            }
            Flat { text: out, segs }
        }
    }
}

#[test]
fn test_pinned_expectations_and_boundaries() {
    // 1. quote: two paragraphs
    let q = B::Quote(vec![B::Para("first".into()), B::Para("second".into())]);
    let f = flatten(&q, "0");
    assert_eq!(f.text, "first\nsecond");

    // 2. list: two paragraphs in one item
    let l2 = B::List(vec![Item {
        checked: None,
        ordered: None,
        body: vec![B::Para("first para".into()), B::Para("second para".into())],
    }]);
    let f2 = flatten(&l2, "0");
    assert_eq!(f2.text, "- first para\n  second para");

    // 3. quote: code then paragraph
    let q3 = B::Quote(vec![B::Code("fn a() {}\n".into()), B::Para("next".into())]);
    let f3 = flatten(&q3, "0");
    assert_eq!(f3.text, "fn a() {}\nnext");

    // 4. list: para -> code -> para
    let l4 = B::List(vec![Item {
        checked: None,
        ordered: None,
        body: vec![
            B::Para("before".into()),
            B::Code("fn a() {}\n".into()),
            B::Para("after".into()),
        ],
    }]);
    let f4 = flatten(&l4, "0");
    assert_eq!(f4.text, "- before\n  fn a() {}\n  after");

    // Edge A: code with an internal blank line inside a list item
    let la = B::List(vec![Item {
        checked: None,
        ordered: None,
        body: vec![B::Para("x".into()), B::Code("a\n\nb\n".into())],
    }]);
    assert_eq!(flatten(&la, "0").text, "- x\n  a\n  \n  b\n");

    // Edge B: code block as the FIRST child of a list item
    let lb = B::List(vec![Item {
        checked: None,
        ordered: None,
        body: vec![B::Code("fn a() {}\nlet b = 1;\n".into())],
    }]);
    assert_eq!(flatten(&lb, "0").text, "- fn a() {}\n  let b = 1;\n");

    // Edge C: two adjacent code blocks in a quote
    let qc = B::Quote(vec![
        B::Code("a\n".into()),
        B::Code("b\n".into()),
        B::Para("after".into()),
    ]);
    assert_eq!(flatten(&qc, "0").text, "a\nb\nafter");

    // Edge D: empty code block
    let ld = B::List(vec![Item {
        checked: None,
        ordered: None,
        body: vec![
            B::Para("before".into()),
            B::Code(String::new()),
            B::Para("after".into()),
        ],
    }]);
    assert_eq!(flatten(&ld, "0").text, "- before\n  after");
}
