//! Tests for the pure word-wrap algorithm behind `visual_line_mode`.
//!
//! These focus solely on `EditorState::wrapped_lines`, the display-time
//! calculation that turns one buffer line into a list of screen-width-sized
//! chunks, broken at word boundaries where possible. No rendering, no
//! keybinding, no settings wiring is exercised here yet — those are later
//! increments.
//!
//! Convention: when a line breaks at a space, that space stays at the end
//! of the earlier chunk rather than being dropped or becoming a leading
//! character of the next chunk. This means every visual line starts at a
//! real character (never a space) and every buffer character — including
//! spaces — belongs to exactly one chunk. That invariant is what a future
//! increment (mapping a buffer column to a visual row/column, for
//! visual-row cursor movement) will need to build on.

use emed_core::EditorState;

/// A line that already fits within the given width should come back as a
/// single, untouched chunk.
#[test]
fn wrapped_lines_returns_single_chunk_when_line_fits() {
    let mut state = EditorState::new((80, 24));
    state.load_document("short\n", Some("dummy.txt"));

    assert_eq!(state.wrapped_lines(0, 10), vec!["short"]);
}

/// A line longer than the width wraps at the nearest space, so words are
/// never split when a space is available to break at. The break space
/// itself stays attached to the end of the earlier chunk.
#[test]
fn wrapped_lines_breaks_at_nearest_space() {
    let mut state = EditorState::new((80, 24));
    state.load_document("the quick brown fox\n", Some("dummy.txt"));

    // width = 10: "the quick " (including the trailing space) is exactly
    // 10 columns, and "brown" would push past the limit, so the break
    // lands right after that space. The next chunk starts clean at "brown".
    assert_eq!(state.wrapped_lines(0, 10), vec!["the quick ", "brown fox"]);
}

/// A single word with no spaces that is itself longer than the width has no
/// break point to back up to, so it falls back to a hard break exactly at
/// the width — matching Emacs' own `visual-line-mode` fallback behaviour.
#[test]
fn wrapped_lines_hard_breaks_when_word_exceeds_width() {
    let mut state = EditorState::new((80, 24));
    state.load_document("abcdefghijklmno\n", Some("dummy.txt"));

    assert_eq!(state.wrapped_lines(0, 10), vec!["abcdefghij", "klmno"]);
}

/// Concatenating the chunks (no extra separator needed) must reconstruct
/// the original line exactly. Every character of the line — space or
/// not — belongs to exactly one chunk.
#[test]
fn wrapped_lines_chunks_reconstruct_original_line_exactly() {
    let mut state = EditorState::new((80, 24));
    state.load_document("the quick brown fox\n", Some("dummy.txt"));

    let chunks = state.wrapped_lines(0, 10);
    assert_eq!(chunks.concat(), "the quick brown fox");
}

/// Wrapping is a display-time-only transformation: computing wrap chunks
/// must never mutate the underlying buffer, even when it has to hard-break
/// a word. The full, unbroken line must still be there afterwards.
#[test]
fn wrapped_lines_does_not_mutate_buffer() {
    let mut state = EditorState::new((80, 24));
    state.load_document("abcdefghijklmno\n", Some("dummy.txt"));

    let _ = state.wrapped_lines(0, 10);

    assert_eq!(state.line_as_string(0), "abcdefghijklmno\n");
}

/// A width of 0 means nothing can ever fit, so there is nothing meaningful
/// to wrap. This mirrors the existing behaviour for an empty buffer line
/// (which already returns an empty `Vec` with no special-casing), rather
/// than inventing a new "one empty-string chunk" convention.
#[test]
fn wrapped_lines_returns_no_chunks_for_zero_width() {
    let mut state = EditorState::new((80, 24));
    state.load_document("abcdefghijklmno\n", Some("dummy.txt"));

    let chunks: Vec<String> = state.wrapped_lines(0, 0);

    assert!(chunks.is_empty());
}

/// `wrapped_screen_rows` composes `wrapped_lines` across the whole buffer
/// (starting at `row_offset`) into the flat list of screen rows
/// `draw_screen` will paint. Rows past the end of the buffer come back as
/// `None`, so the caller can print `~` for them exactly as it does today.
#[test]
fn wrapped_screen_rows_pads_with_none_past_buffer_end() {
    let mut state = EditorState::new((80, 24));
    state.load_document("one\ntwo\nthree", Some("dummy.txt"));

    let rows = state.wrapped_screen_rows(5, 10);

    assert_eq!(
        rows,
        vec![
            Some("one".to_string()),
            Some("two".to_string()),
            Some("three".to_string()),
            None,
            None,
        ]
    );
}

/// A blank buffer line must still occupy exactly one screen row (an empty
/// string), not zero — `wrapped_lines` returns an empty `Vec` for a blank
/// line, which is correct in isolation, but composing screen rows must not
/// let that collapse the blank line out of existence and shift every line
/// below it up by one row.
#[test]
fn wrapped_screen_rows_gives_blank_line_exactly_one_row() {
    let mut state = EditorState::new((80, 24));
    state.load_document("one\n\ntwo", Some("dummy.txt"));

    let rows = state.wrapped_screen_rows(3, 10);

    assert_eq!(
        rows,
        vec![
            Some("one".to_string()),
            Some(String::new()),
            Some("two".to_string()),
        ]
    );
}

/// A buffer line that wraps into several chunks consumes that many screen
/// rows, and layout continues correctly with the buffer lines after it.
#[test]
fn wrapped_screen_rows_lets_one_buffer_line_span_multiple_rows() {
    let mut state = EditorState::new((80, 24));
    state.load_document("the quick brown fox\nnext", Some("dummy.txt"));

    let rows = state.wrapped_screen_rows(5, 10);

    assert_eq!(
        rows,
        vec![
            Some("the quick ".to_string()),
            Some("brown fox".to_string()),
            Some("next".to_string()),
            None,
            None,
        ]
    );
}

/// Known limitation, accepted for this increment: `row_offset` is still a
/// buffer-line index, not a visual-row index, so there's no way yet to know
/// in advance that a wrapped line won't fully fit in the remaining rows.
/// When the screen runs out of rows mid-line, the rest of that line's
/// chunks are simply dropped rather than being scrolled to. This test pins
/// down that exact (imperfect) behaviour so a future fix — scrolling by
/// visual row instead of buffer line — has a clear regression signal.
#[test]
fn wrapped_screen_rows_clips_a_line_that_does_not_fully_fit() {
    let mut state = EditorState::new((80, 24));
    state.load_document("short\nthe quick brown fox", Some("dummy.txt"));

    // Only 2 rows available: "short" takes row 1, leaving just 1 row for
    // "the quick brown fox", which would normally need 2 rows to wrap into.
    let rows = state.wrapped_screen_rows(2, 10);

    assert_eq!(
        rows,
        vec![Some("short".to_string()), Some("the quick ".to_string())]
    );
}
