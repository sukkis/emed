use emed_core::{EditorState, FileType};

#[test]
fn load_document_replaces_buffer_and_resets_cursor_and_scroll() {
    let mut state = EditorState::new((80, 4));

    // Move away from origin and force some scroll state.
    state.load_document("0\n1\n2\n3\n4\n", Some("a.txt"));
    state.set_cursor(0, 4);
    state.ensure_cursor_visible();
    assert!(state.row_offset() > 0);

    state.load_document("one\ntwo\n", Some("b.txt"));

    assert_eq!(state.cursor_pos(), (0, 0));
    assert_eq!(state.row_offset(), 0);

    assert_eq!(state.line_as_string(0), "one\n");
    assert_eq!(state.line_as_string(1), "two\n");
}

#[test]
fn load_document_sets_filename_and_detects_rust_filetype() {
    let mut state = EditorState::new((80, 24));

    state.load_document("fn main() {}\n", Some("main.rs"));

    assert_eq!(state.filename, "main.rs");
    assert_eq!(state.file_type.as_str(), "Rust file");

    // Also sanity-check the enum variant is what we expect.
    match state.file_type {
        FileType::Rust => {}
        _ => panic!("expected FileType::Rust"),
    }
}

#[test]
fn load_document_with_unknown_extension_defaults_to_text_or_unknown() {
    let mut state = EditorState::new((80, 24));

    state.load_document("hello\n", Some("notes.txt"));
    assert_eq!(state.file_type.as_str(), "text");

    state.load_document("hello\n", Some("README"));
    assert_eq!(state.file_type.as_str(), "unknown");
}

#[test]
fn load_document_with_none_filename_resets_filename_and_filetype() {
    let mut state = EditorState::new((80, 24));

    state.load_document("fn main() {}\n", Some("main.rs"));
    assert_eq!(state.file_type.as_str(), "Rust file");

    state.load_document("just text\n", None);
    assert_eq!(state.filename, "-");
    assert_eq!(state.file_type.as_str(), "unknown");
}

#[test]
fn load_document_detects_c_filetype_for_dot_c() {
    let mut state = EditorState::new((80, 24));

    state.load_document("#include <stdio.h>\n", Some("kilo.c"));

    assert_eq!(state.filename, "kilo.c");
    assert_eq!(state.file_type.as_str(), "C file");

    match state.file_type {
        FileType::C => {}
        _ => panic!("expected FileType::C"),
    }
}

#[test]
fn load_document_detects_c_filetype_for_dot_h() {
    let mut state = EditorState::new((80, 24));

    state.load_document(
        "#ifndef HEADER_H\n#define HEADER_H\n#endif\n",
        Some("editor.h"),
    );

    assert_eq!(state.filename, "editor.h");
    assert_eq!(state.file_type.as_str(), "C file");

    match state.file_type {
        FileType::C => {}
        _ => panic!("expected FileType::C"),
    }
}
