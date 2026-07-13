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

/// `screen_rows_before_line` is the row/Y half of mapping a buffer
/// position to a screen position: how many wrapped screen rows do the
/// buffer lines from `row_offset` up to (not including) `line_index`
/// occupy? This is the piece both the cursor-placement fix and (later)
/// visual-row Up/Down movement need.

/// With no wrapping happening, each buffer line is exactly one screen
/// row, so this behaves like plain line counting.
#[test]
fn screen_rows_before_line_counts_unwrapped_lines_as_one_row_each() {
    let mut state = EditorState::new((80, 24));
    state.load_document("one\ntwo\nthree", Some("dummy.txt"));

    // Lines 0 and 1 ("one", "two") come before line 2 — 2 rows.
    assert_eq!(state.screen_rows_before_line(2, 10), 2);
}

/// A line that wraps into multiple chunks contributes that many rows,
/// not just one.
#[test]
fn screen_rows_before_line_counts_wrapped_lines_as_multiple_rows() {
    let mut state = EditorState::new((80, 24));
    state.load_document("the quick brown fox\nnext", Some("dummy.txt"));

    // Line 0 wraps into 2 chunks ("the quick ", "brown fox") before
    // line 1 ("next") begins.
    assert_eq!(state.screen_rows_before_line(1, 10), 2);
}

/// A blank line still counts as exactly one row, matching the same rule
/// `wrapped_screen_rows` uses.
#[test]
fn screen_rows_before_line_counts_blank_line_as_one_row() {
    let mut state = EditorState::new((80, 24));
    state.load_document("one\n\ntwo", Some("dummy.txt"));

    // Line 0 ("one") + line 1 (blank) = 2 rows before line 2 ("two").
    assert_eq!(state.screen_rows_before_line(2, 10), 2);
}

/// Counting starts at `row_offset`, not always from buffer line 0 — once
/// the viewport has scrolled, only the lines actually still on screen
/// above `line_index` should count.
#[test]
fn screen_rows_before_line_respects_row_offset() {
    // 5 rows tall screen → text area height = 5 - 2 = 3.
    let mut state = EditorState::new((10, 5));
    state.load_document("one\ntwo\nthree\nfour\nfive\nsix", Some("dummy.txt"));

    // Move down to buffer line 4 ("five"). With a 3-row text area this
    // scrolls row_offset to 2 (see the identical math in the existing
    // cursor_up_and_down_clamp_cx_to_line_length test in lib.rs).
    for _ in 0..4 {
        state.cursor_down();
    }
    assert_eq!(state.row_offset(), 2);

    // Only lines 2 and 3 ("three", "four") are between row_offset and
    // line 4 now — not lines 0..4, since 0 and 1 have scrolled off.
    assert_eq!(state.screen_rows_before_line(4, 10), 2);
}

/// `wrapped_cursor_offset` is the within-the-current-line half of mapping
/// a buffer position to a screen position: given `cx` on `line_index`,
/// which wrapped chunk does it fall in, and what column within that
/// chunk? Combined with `screen_rows_before_line`, this is everything
/// `draw_screen` needs to place the cursor correctly under wrapping.

/// A cursor on a line short enough not to wrap is always in chunk 0, at
/// its own character offset.
#[test]
fn wrapped_cursor_offset_within_a_single_chunk() {
    let mut state = EditorState::new((80, 24));
    state.load_document("short\n", Some("dummy.txt"));

    // cx = 2 is the cursor sitting between 'h' and 'o'.
    assert_eq!(state.wrapped_cursor_offset(0, 2, 10), (0, 2));
}

/// A cursor inside a later wrapped chunk reports that chunk's index and
/// an offset relative to *that chunk's* start, not the whole line.
#[test]
fn wrapped_cursor_offset_within_a_later_chunk() {
    let mut state = EditorState::new((80, 24));
    state.load_document("the quick brown fox\n", Some("dummy.txt"));

    // cx = 12 is between 'r' and 'o' in "brown", which is 2 characters
    // into the second chunk ("brown fox").
    assert_eq!(state.wrapped_cursor_offset(0, 12, 10), (1, 2));
}

/// A cursor at the very end of the line sits at the end of the last
/// chunk, not off the end of a nonexistent next chunk.
#[test]
fn wrapped_cursor_offset_at_end_of_line() {
    let mut state = EditorState::new((80, 24));
    state.load_document("short\n", Some("dummy.txt"));

    assert_eq!(state.wrapped_cursor_offset(0, 5, 10), (0, 5));
}

/// A cursor sitting exactly at a *mid-line* chunk boundary belongs to the
/// start of the next row, not the end of the previous one — the same
/// "every visual line starts at a real character" rule from
/// `wrapped_lines` applies to the cursor too.
#[test]
fn wrapped_cursor_offset_at_chunk_boundary_lands_at_start_of_next_row() {
    let mut state = EditorState::new((80, 24));
    state.load_document("the quick brown fox\n", Some("dummy.txt"));

    // cx = 10 is exactly the boundary between "the quick " and "brown fox".
    assert_eq!(state.wrapped_cursor_offset(0, 10, 10), (1, 0));
}

/// A blank line has no chunks to look through — the cursor is simply at
/// its one (empty) row, column 0.
#[test]
fn wrapped_cursor_offset_on_blank_line() {
    let mut state = EditorState::new((80, 24));
    state.load_document("one\n\ntwo", Some("dummy.txt"));

    assert_eq!(state.wrapped_cursor_offset(1, 0, 10), (0, 0));
}
