/// Test suite for Drawer visual polish and noise reduction (Ticket D1 / ADR 0033)
///
/// Verifies:
/// 1. "Workspace" group label is removed from drawer.
/// 2. 5px gray open-file dot is completely eradicated.
/// 3. Standardized 13px font size and 28px row height across drawer items.
/// 4. Directory nodes use medium weight (`FontWeight::MEDIUM`).
/// 5. Drawer items hover uses `theme.warm_pill_hover()` instead of harsh white `theme.surface`.

#[test]
fn test_drawer_workspace_group_label_removed() {
    let src = include_str!("../src/views/drawer.rs");

    // Assert that strings.workspace_group is no longer rendered as a child element in the drawer
    assert!(
        !src.contains("child(strings.workspace_group)"),
        "drawer.rs must not render 'strings.workspace_group' to avoid visual noise"
    );
}

#[test]
fn test_drawer_5px_gray_dot_removed() {
    let src = include_str!("../src/views/drawer.rs");

    // Assert that the 5px gray dot indicator for open files is completely removed
    assert!(
        !src.contains("rounded_full().bg(theme.text_muted)"),
        "drawer.rs must not contain the 5px gray dot ('rounded_full().bg(theme.text_muted)') for open files"
    );
    assert!(
        !src.contains("div().w(px(5.)).h(px(5.))"),
        "drawer.rs must not contain 5px spacer placeholder ('div().w(px(5.)).h(px(5.))')"
    );
}

#[test]
fn test_drawer_item_typography_and_row_height() {
    let src = include_str!("../src/views/drawer.rs");

    // 1. Draft items: h(px(28.)) and text_size(px(13.))
    let draft_pos = src
        .find("drawer_draft_item")
        .expect("drawer.rs must define drawer_draft_item");
    let draft_end = (draft_pos + 2000).min(src.len());
    let draft_section = &src[draft_pos..draft_end];
    assert!(
        draft_section.contains(".h(px(28.))"),
        "drawer_draft_item must use 28px row height .h(px(28.))"
    );
    assert!(
        draft_section.contains(".text_size(px(13.))"),
        "drawer_draft_item must use 13px font size .text_size(px(13.))"
    );

    // 2. Loose docs: h(px(28.)) and text_size(px(13.))
    let loose_pos = src
        .find("drawer_loose_file_")
        .expect("drawer.rs must define drawer_loose_file");
    let loose_end = (loose_pos + 2000).min(src.len());
    let loose_section = &src[loose_pos..loose_end];
    assert!(
        loose_section.contains(".h(px(28.))"),
        "loose docs must use 28px row height .h(px(28.))"
    );
    assert!(
        loose_section.contains(".text_size(px(13.))"),
        "loose docs must use 13px font size .text_size(px(13.))"
    );

    // 3. Tree directory nodes: h(px(28.)), text_size(px(13.)), and FontWeight::MEDIUM
    let dir_pos = src
        .find("drawer_tree_dir_")
        .expect("drawer.rs must define drawer_tree_dir");
    let dir_end = (dir_pos + 2000).min(src.len());
    let dir_section = &src[dir_pos..dir_end];
    assert!(
        dir_section.contains(".h(px(28.))"),
        "tree directory nodes must use 28px row height .h(px(28.))"
    );
    assert!(
        dir_section.contains(".text_size(px(13.))"),
        "tree directory nodes must use 13px font size .text_size(px(13.))"
    );
    assert!(
        dir_section.contains("FontWeight::MEDIUM"),
        "tree directory nodes must use FontWeight::MEDIUM"
    );

    // 4. Tree file nodes: h(px(28.)) and text_size(px(13.))
    let file_pos = src
        .find("drawer_tree_file_")
        .expect("drawer.rs must define drawer_tree_file");
    let file_end = (file_pos + 2000).min(src.len());
    let file_section = &src[file_pos..file_end];
    assert!(
        file_section.contains(".h(px(28.))"),
        "tree file nodes must use 28px row height .h(px(28.))"
    );
    assert!(
        file_section.contains(".text_size(px(13.))"),
        "tree file nodes must use 13px font size .text_size(px(13.))"
    );
}

#[test]
fn test_drawer_items_warm_pill_hover_styling() {
    let src = include_str!("../src/views/drawer.rs");

    // Draft items hover
    let draft_pos = src
        .find("drawer_draft_item")
        .expect("drawer.rs must define drawer_draft_item");
    let draft_end = (draft_pos + 2000).min(src.len());
    let draft_section = &src[draft_pos..draft_end];
    assert!(
        draft_section.contains("theme.warm_pill_hover()"),
        "drawer_draft_item must use theme.warm_pill_hover() for hover"
    );

    // Tree directory nodes hover
    let dir_pos = src
        .find("drawer_tree_dir_")
        .expect("drawer.rs must define drawer_tree_dir");
    let dir_end = (dir_pos + 2000).min(src.len());
    let dir_section = &src[dir_pos..dir_end];
    assert!(
        dir_section.contains("theme.warm_pill_hover()"),
        "drawer_tree_dir must use theme.warm_pill_hover() for hover"
    );

    // Tree file nodes hover
    let file_pos = src
        .find("drawer_tree_file_")
        .expect("drawer.rs must define drawer_tree_file");
    let file_end = (file_pos + 2000).min(src.len());
    let file_section = &src[file_pos..file_end];
    assert!(
        file_section.contains("theme.warm_pill_hover()"),
        "drawer_tree_file must use theme.warm_pill_hover() for hover"
    );

    // Loose doc items hover
    let loose_pos = src
        .find("drawer_loose_file_")
        .expect("drawer.rs must define drawer_loose_file");
    let loose_end = (loose_pos + 2000).min(src.len());
    let loose_section = &src[loose_pos..loose_end];
    assert!(
        loose_section.contains("theme.warm_pill_hover()"),
        "loose doc items must use theme.warm_pill_hover() for hover"
    );
}
