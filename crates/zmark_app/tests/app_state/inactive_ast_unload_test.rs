use zmark_app::state::{AppState, DocKey};
use zmark_core::document::Document;

#[test]
fn inactive_documents_release_their_ast_and_restore_it_on_switch() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.md");
    let second = dir.path().join("second.md");
    std::fs::write(&first, "# First\n\nFirst body.\n").unwrap();
    std::fs::write(&second, "# Second\n\nSecond body.\n").unwrap();
    let first = first.canonicalize().unwrap();
    let second = second.canonicalize().unwrap();

    let mut state = AppState::new_isolated(Document::from_file(first.clone()).unwrap());
    state.open_file(second.clone()).unwrap();

    let first_key = DocKey::Path(first.clone());
    let second_key = DocKey::Path(second.clone());
    let first_doc = state
        .open_documents()
        .iter()
        .find(|doc| doc.key == first_key)
        .unwrap();
    assert!(
        first_doc.doc.parsed.blocks.is_empty(),
        "inactive tab must release its AST"
    );
    assert!(first_doc.doc.is_parse_stale());
    assert!(!state.doc().parsed.blocks.is_empty());

    state.switch_document(&first_key);
    assert!(
        !state.doc().parsed.blocks.is_empty(),
        "switching back must rebuild the AST"
    );
    assert!(!state.doc().is_parse_stale());
    let second_doc = state
        .open_documents()
        .iter()
        .find(|doc| doc.key == second_key)
        .unwrap();
    assert!(second_doc.doc.parsed.blocks.is_empty());
    assert!(second_doc.doc.is_parse_stale());
}
