//! Tests for `EditorState::char_index_to_cursor` — converting a plain char
//! index (as produced by search) into a `(cx, cy)` cursor position.
//!
//! Kept in its own file rather than lib.rs's in-file `mod tests`, since this
//! helper is a step toward search, not a permanent home — lib.rs already
//! carries a fair amount of test weight and this is a natural place to draw
//! the line for future thinning.

use emed_core::EditorState;

/// Document used by every test here:
///
/// ```text
/// idx:  0 1 2  3 4 5  6 7 8
/// char: a b \n c d \n e f \n
/// line: 0 0 0  1 1 1  2 2 2
/// ```
///
/// Lines: "ab\n" (0..3), "cd\n" (3..6), "ef\n" (6..9), then an empty trailing
/// line 3 starting at char index 9 (ropey's convention for text ending in a
/// newline: there is always one more, empty, line after the last `\n`).
const DOC: &str = "ab\ncd\nef\n";

fn state_with_doc(text: &str) -> EditorState {
    let mut state = EditorState::new((80, 24));
    state.load_document(text, Some("test.txt"));
    state
}

#[test]
fn index_zero_is_start_of_buffer() {
    let state = state_with_doc(DOC);
    assert_eq!(state.char_index_to_cursor(0), (0, 0));
}

#[test]
fn index_in_middle_of_a_line() {
    let state = state_with_doc(DOC);
    // idx 4 sits just before 'd' — the second line, second column.
    assert_eq!(state.char_index_to_cursor(4), (1, 1));
}

#[test]
fn index_exactly_at_a_line_boundary_starts_the_new_line() {
    let state = state_with_doc(DOC);
    // idx 3 is the position right after the first '\n' — start of line 1,
    // not the end of line 0.
    assert_eq!(state.char_index_to_cursor(3), (0, 1));
}

#[test]
fn index_past_end_clamps_to_the_trailing_empty_line() {
    let state = state_with_doc(DOC);
    // The document has 9 chars. An index at or past the end clamps to
    // len_chars() (9), which ropey places on line 3 — the empty line that
    // follows the final '\n'. Sitting "on" that last '\n' means you're
    // actually on the next (empty) line, per ropey's own convention.
    assert_eq!(state.char_index_to_cursor(9), (0, 3));
    assert_eq!(state.char_index_to_cursor(100), (0, 3));
}

#[test]
fn index_is_unaffected_by_multibyte_characters_earlier_in_the_document() {
    // "é" is one char but two bytes in UTF-8. This guards that the
    // conversion works in char space (like the rest of the editor already
    // does), not byte space.
    let doc = "héllo\ncafé\n";
    let state = state_with_doc(doc);
    // idx 9 is the position right before the final 'é' on line 1: after
    // "caf" (3 chars into line 1).
    assert_eq!(state.char_index_to_cursor(9), (3, 1));
}
