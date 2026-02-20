use emed_core::EditorState;
use emed_core::lexer::TokenKind;

#[test]
fn tokens_for_line_returns_tokens_after_load_document() {
    let mut state = EditorState::new((80, 24));
    state.load_document("let x = 42;\n", Some("test.rs"));

    let tokens = state.tokens_for_line(0).to_vec();
    assert!(!tokens.is_empty(), "Rust file should produce tokens");

    // "42" should be highlighted as a number.
    let number_token = tokens.iter().find(|t| t.kind == TokenKind::Number);
    assert!(number_token.is_some(), "should find a Number token for 42");

    let nt = number_token.unwrap();
    assert_eq!(nt.start, 8);
    assert_eq!(nt.len, 2);
}

#[test]
fn tokens_for_line_returns_empty_when_no_file_loaded() {
    let mut state = EditorState::new((80, 24));
    // No load_document called — fresh empty buffer.
    // Should not panic, and tokens should be usable.
    let tokens = state.tokens_for_line(0);
    // PlainLexer or empty — either way, no crash.
    assert!(
        tokens
            .iter()
            .all(|t| t.kind == TokenKind::Normal || t.kind == TokenKind::Number)
    );
}

#[test]
fn token_cache_is_invalidated_after_edit() {
    let mut state = EditorState::new((80, 24));
    state.load_document("hello\n", Some("test.rs"));

    // Prime the cache.
    let _ = state.tokens_for_line(0).to_vec();

    // Edit the buffer — cache should be invalidated.
    state.insert_char('9');

    // After edit, tokens_for_line should re-tokenize and find the '9'.
    let tokens = state.tokens_for_line(0).to_vec();
    assert!(
        tokens.iter().any(|t| t.kind == TokenKind::Number),
        "after inserting '9', should find a Number token"
    );
}

#[test]
fn u16_type_is_not_highlighted_as_number_in_loaded_file() {
    let mut state = EditorState::new((80, 24));
    state.load_document("let x: u16 = 0;\n", Some("test.rs"));

    let tokens = state.tokens_for_line(0).to_vec();
    // The "16" inside "u16" must NOT be a Number.
    // "u16" starts at position 7, so positions 8 and 9 are "16".
    let bad = tokens
        .iter()
        .find(|t| t.kind == TokenKind::Number && t.start == 8);
    assert!(
        bad.is_none(),
        "digits inside 'u16' must not be highlighted as Number"
    );
}
