//! The About page must reuse the shared settings row/section components, and
//! the header must no longer carry red-dot markers.

fn settings_src() -> &'static str {
    include_str!("../../src/views/settings.rs")
}

fn workspace_src() -> &'static str {
    include_str!("../../src/views/workspace.rs")
}

fn fn_body<'a>(src: &'a str, name: &str) -> &'a str {
    let start = src.find(name).unwrap_or_else(|| panic!("{name} missing"));
    let rest = &src[start..];
    let end = rest[1..]
        .find("\nfn ")
        .or_else(|| rest[1..].find("\npub fn "))
        .map(|i| i + 1)
        .unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn about_page_reuses_shared_setting_components() {
    let about = fn_body(settings_src(), "fn render_about_tab(");
    // About's section titles come from titled setting groups, which draw them
    // through `render_section` -> `render_section_title`.
    assert!(
        about.contains("render_setting_group(\n            Some("),
        "About must use titled shared setting groups"
    );
    let group = fn_body(settings_src(), "fn render_setting_group(");
    assert!(group.contains("render_section("), "setting groups must use the shared section");
    let section = fn_body(settings_src(), "fn render_section(");
    assert!(
        section.contains("render_section_title("),
        "sections must draw their title with the shared section title"
    );
    assert!(about.contains("render_setting_row("), "About must use the shared setting row");
    assert!(about.contains("render_secondary_button("), "About must use the shared secondary button");
    assert!(
        !about.contains("theme.accent"),
        "About must not paint accent color (version, buttons and dots stay neutral)"
    );
}

#[test]
fn toggle_rows_use_the_shared_row_component() {
    let editor = fn_body(settings_src(), "fn render_editor_save_tab(");
    assert!(editor.matches("render_setting_row(").count() >= 3);
    let appearance = fn_body(settings_src(), "fn render_appearance_tab(");
    assert!(appearance.contains("render_setting_row("));
}

#[test]
fn header_has_no_dirty_or_update_dot() {
    let ws = workspace_src();
    // `dirty_pulse_generation` remains the state counter; only the dot element id is gone.
    assert!(
        !ws.contains("format!(\"dirty_pulse_{"),
        "the red dirty dot element must be gone"
    );
    let title_pos = ws
        .find("canvas_document_title")
        .expect("workspace.rs must contain canvas_document_title");
    let title_section = &ws[title_pos..title_pos + 1200.min(ws.len() - title_pos)];
    assert!(
        !title_section.contains("rounded_full().bg(theme.accent)"),
        "no accent dot may be attached to canvas_document_title"
    );
    assert!(
        ws.contains("dirty_title_"),
        "dirty state is expressed as a one-shot title fade instead"
    );
    let update_icon = ws
        .split("btn_update_ready")
        .nth(1)
        .expect("update icon missing");
    assert!(
        !update_icon[..update_icon.len().min(1800)].contains("rounded_full()"),
        "the update icon must not carry a badge dot"
    );
}
