//! Integration tests for incremental search, driven entirely through
//! `EditorState` — no terminal, no keybindings (those come later).
//!
//! Commit 4a scope only: `search_start`, `search_push_char`,
//! `search_backspace`, `is_searching`, `search_query`. No `search_repeat`
//! (needs `last_match`, added in 4b) and no `search_accept`/`search_cancel`
//! (4c) yet.

use emed_core::EditorState;

#[test]
fn typing_query_jumps_cursor_to_first_match() {
    let mut state = EditorState::new((80, 24));
    state.load_document("one two three\n", Some("test.txt"));

    state.search_start();
    state.search_push_char('t');
    state.search_push_char('w');
    state.search_push_char('o');

    // "two" starts at char index 4.
    assert_eq!(state.cursor_pos(), (4, 0));
}

#[test]
fn cursor_refines_as_query_grows() {
    let mut state = EditorState::new((80, 24));
    state.load_document("ax bat\n", Some("test.txt"));

    state.search_start(); // origin = (0, 0)

    state.search_push_char('a');
    // "a" matches the 'a' in "ax", at index 0.
    assert_eq!(state.cursor_pos(), (0, 0));

    state.search_push_char('t');
    // "at" doesn't match "ax", but does match "bat" at index 4 — the match
    // moves as the query grows, it isn't stuck wherever "a" first matched.
    assert_eq!(state.cursor_pos(), (4, 0));
}

#[test]
fn backspace_shrinks_query_and_rematches() {
    let mut state = EditorState::new((80, 24));
    state.load_document("ax bat\n", Some("test.txt"));

    state.search_start();
    state.search_push_char('a');
    state.search_push_char('t');
    assert_eq!(state.cursor_pos(), (4, 0)); // "at" -> "bat"

    state.search_backspace();
    // Back to "a" -> matches "ax" again, at index 0.
    assert_eq!(state.cursor_pos(), (0, 0));
}

#[test]
fn no_match_leaves_cursor_in_place() {
    let mut state = EditorState::new((80, 24));
    state.load_document("ax bat\n", Some("test.txt"));

    state.search_start();
    state.search_push_char('a');
    assert_eq!(state.cursor_pos(), (0, 0));

    state.search_push_char('z'); // "az" appears nowhere in the document
    assert_eq!(state.cursor_pos(), (0, 0)); // stays at the last real match
}

#[test]
fn is_searching_and_search_query_reflect_state() {
    let mut state = EditorState::new((80, 24));
    state.load_document("abc\n", Some("test.txt"));

    assert!(!state.is_searching());
    assert_eq!(state.search_query(), None);

    state.search_start();
    assert!(state.is_searching());
    assert_eq!(state.search_query(), Some(""));

    state.search_push_char('a');
    assert_eq!(state.search_query(), Some("a"));
}

#[test]
fn search_repeat_advances_through_matches_and_wraps() {
    let mut state = EditorState::new((80, 24));
    state.load_document("cat cat cat\n", Some("test.txt"));

    state.search_start();
    state.search_push_char('c');
    state.search_push_char('a');
    state.search_push_char('t');
    assert_eq!(state.cursor_pos(), (0, 0)); // first "cat", found by typing

    state.search_repeat();
    assert_eq!(state.cursor_pos(), (4, 0)); // second "cat"

    state.search_repeat();
    assert_eq!(state.cursor_pos(), (8, 0)); // third "cat"

    state.search_repeat();
    assert_eq!(state.cursor_pos(), (0, 0)); // wraps back to the first
}

#[test]
fn search_repeat_does_nothing_without_an_active_search() {
    let mut state = EditorState::new((80, 24));
    state.load_document("cat cat cat\n", Some("test.txt"));

    state.search_repeat(); // no active session — must not panic or move
    assert_eq!(state.cursor_pos(), (0, 0));
}

#[test]
fn search_cancel_restores_original_cursor_and_ends_session() {
    let mut state = EditorState::new((80, 24));
    state.load_document("one two three\n", Some("test.txt"));
    state.set_cursor(3, 0); // start searching from the space before "two"

    state.search_start();
    state.search_push_char('t');
    state.search_push_char('w');
    state.search_push_char('o');
    assert_eq!(state.cursor_pos(), (4, 0)); // jumped forward to "two"

    state.search_cancel();
    // Restored to where the search began (3), not left at the match (4).
    assert_eq!(state.cursor_pos(), (3, 0));
    assert!(!state.is_searching());
}

#[test]
fn search_accept_keeps_cursor_at_match_and_ends_session() {
    let mut state = EditorState::new((80, 24));
    state.load_document("one two three\n", Some("test.txt"));

    state.search_start();
    state.search_push_char('t');
    state.search_push_char('w');
    state.search_push_char('o');
    assert_eq!(state.cursor_pos(), (4, 0));

    state.search_accept();
    assert_eq!(state.cursor_pos(), (4, 0)); // stays at the match
    assert!(!state.is_searching());
}

#[test]
fn loading_a_new_document_clears_any_active_search() {
    let mut state = EditorState::new((80, 24));
    state.load_document("abc\n", Some("test.txt"));
    state.search_start();
    assert!(state.is_searching());

    state.load_document("xyz\n", Some("test2.txt"));
    assert!(!state.is_searching());
}
