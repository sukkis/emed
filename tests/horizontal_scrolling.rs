//! Horizontal‑scrolling tests for the EMED core.
//!
//! These focus on two things:
//!   - `EditorState::ensure_cursor_visible` updates `col_offset` correctly
//!       when the cursor moves past the right edge of the visible window.
//!   - `EditorState::get_slice` returns exactly the characters that
//!       should be displayed for a given screen width.

use emed_core::{EditorState, InputKey, command_from_key};

/// Helper that feeds a single key into the core and returns the resulting
/// `ApplyResult`.  Mirrors the `run_key` helper used in `mini_session.rs`.
fn apply_key(
    state: &mut EditorState,
    key: InputKey,
    saw_ctrl_x: &mut bool,
) -> emed_core::ApplyResult {
    let cmd = command_from_key(key, saw_ctrl_x);
    state.apply_command(cmd)
}

/// ---------------------------------------------------------------------------
/// 1️⃣  Horizontal offset updates when the cursor moves right
/// ---------------------------------------------------------------------------
#[test]
fn col_offset_advances_when_cursor_exceeds_screen_width() {
    // Simulate a tiny terminal: 5 columns of visible text.
    let mut state = EditorState::new((5, 24));

    // Load a single long line so we have plenty of characters to the right.
    state.load_document(
        "abcdefghijklmnopqrstuvwxyz\n", // 26 chars + newline
        Some("dummy.txt"),
    );

    // Initially the cursor is at (0,0) and col_offset == 0.
    assert_eq!(state.cursor_pos(), (0, 0));
    assert_eq!(state.col_offset(), 0);

    // Move the cursor right 6 times – that is one column beyond the 5‑col window.
    for _ in 0..6 {
        apply_key(&mut state, InputKey::Right, &mut false);
    }

    // After the sixth move the cursor column should be 6,
    // and the visible window should have slid right by 2 columns:
    //   screen width = 5 → visible start = cursor + 1 - width = 6 + 1 - 5 = 2
    assert_eq!(state.cursor_pos(), (6, 0));
    assert_eq!(state.col_offset(), 2);
}

/// ---------------------------------------------------------------------------
/// 2️⃣  `get_slice` returns the correct fragment for a given width
/// ---------------------------------------------------------------------------
#[test]
fn get_slice_respects_col_offset_and_width() {
    // 5‑column screen again.
    let mut state = EditorState::new((5, 24));
    state.load_document("abcdefghijklmnopqrstuvwxyz\n", Some("alpha.txt"));

    // Put the cursor far to the right so that `col_offset` becomes non‑zero.
    // We'll move the cursor to column 20.
    for _ in 0..20 {
        apply_key(&mut state, InputKey::Right, &mut false);
    }

    // At this point:
    //   col_offset = cursor + 1 - width = 20 + 1 - 5 = 16
    assert_eq!(state.cursor_pos(), (20, 0));
    assert_eq!(state.col_offset(), 16);

    // Ask the core for the visible fragment of line 0.
    let slice = state.get_slice(0, 5);

    // Characters 16..21 of the alphabet are "qrstu".
    assert_eq!(slice, "qrstu");
}

/// ---------------------------------------------------------------------------
/// 3️⃣  `get_slice` returns the whole line when the line is shorter than the screen
/// ---------------------------------------------------------------------------
#[test]
fn get_slice_returns_full_line_when_line_shorter_than_screen() {
    let mut state = EditorState::new((5, 24));
    state.load_document("abc\n", Some("short.txt")); // line length = 3 chars

    // No horizontal scrolling needed – col_offset stays 0.
    assert_eq!(state.col_offset(), 0);

    // The slice for line 0 should be the whole line (without the trailing newline).
    let slice = state.get_slice(0, 5);
    assert_eq!(slice, "abc");
}

/// ---------------------------------------------------------------------------
/// 4️⃣  Horizontal scrolling works together with vertical scrolling
/// ---------------------------------------------------------------------------
#[test]
fn horizontal_and_vertical_scrolling_combined() {
    // Very small screen: 5 columns × 4 rows → text area height = 2 rows.
    let mut state = EditorState::new((5, 4));
    // Two long lines, each longer than the screen width.
    state.load_document(
        "aaaaaaaaaaaaaaaaaaaa\nbbbbbbbbbbbbbbbbbbbb\n",
        Some("both.txt"),
    );

    // Move cursor to the far‑right of the first line (col 20).
    // The line has 20 ‘a’s, so after 20 Right presses the cursor sits at the end.
    for _ in 0..20 {
        apply_key(&mut state, InputKey::Right, &mut false);
    }

    // At this point the calculated offset is:
    //   col_offset = cursor + 1 - width = 20 + 1 - 5 = 16
    assert_eq!(state.col_offset(), 16);

    // Now move the cursor down one line – this forces a vertical scroll.
    apply_key(&mut state, InputKey::Down, &mut false);

    // Verify both offsets are non‑zero (vertical scroll happened, horizontal stayed).
    assert_eq!(state.row_offset(), 0);
    assert!(state.col_offset() > 0);

    // The visible fragment of the second line should start at column 16.
    // The second line is all 'b's, so we expect "bbbb".
    let slice = state.get_slice(1, 5);
    assert_eq!(slice, "bbbb");
}
