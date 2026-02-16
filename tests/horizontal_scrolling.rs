//! Horizontal‑scrolling tests for the EMED core.
//!
//! These focus on:
//!   - `EditorState::ensure_cursor_visible` updates `col_offset` correctly
//!       when the cursor moves past the right edge of the visible window.
//!   - `EditorState::get_slice` returns exactly the characters that
//!       should be displayed for a given screen width.
//!   - `cx_to_screen_col` maps char indices to screen columns correctly.

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
/// Horizontal offset updates when the cursor moves right
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
/// `get_slice` returns the correct fragment for a given width
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
/// `get_slice` returns the whole line when the line is shorter than the screen
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
/// Horizontal scrolling works together with vertical scrolling
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

/// ---------------------------------------------------------------------------
/// cx_to_screen_col matches char index for pure ASCII
/// ---------------------------------------------------------------------------
#[test]
fn cx_to_screen_col_matches_char_index_for_ascii() {
    let mut state = EditorState::new((80, 24));
    state.load_document("hello world\n", Some("test.txt"));

    // For pure ASCII, screen col == char index
    assert_eq!(state.cx_to_screen_col(0, 0), 0);
    assert_eq!(state.cx_to_screen_col(0, 5), 5);
    assert_eq!(state.cx_to_screen_col(0, 11), 11);
}

/// ---------------------------------------------------------------------------
/// Typing past screen width scrolls the viewport
/// ---------------------------------------------------------------------------
#[test]
fn typing_past_screen_width_scrolls_viewport() {
    let mut state = EditorState::new((10, 24));

    // Type 12 characters on a 10-column screen
    for c in "abcdefghijkl".chars() {
        state.insert_char(c);
    }

    // cx = 12, screen_col = 12
    assert_eq!(state.cursor_pos().0, 12);
    assert_eq!(state.cx_to_screen_col(0, 12), 12);

    // col_offset should have advanced: 12 + 1 - 10 = 3
    assert_eq!(state.col_offset(), 3);

    // Visible slice should show characters from offset 3, width 10
    let slice = state.get_slice(0, 10);
    assert_eq!(slice, "defghijkl");
}

/// ---------------------------------------------------------------------------
/// col_offset snaps back to 0 when cursor moves left
/// ---------------------------------------------------------------------------
#[test]
fn col_offset_snaps_back_when_cursor_moves_left() {
    let mut state = EditorState::new((5, 24));
    state.load_document("abcdefghijklmnopqrstuvwxyz\n", Some("test.txt"));

    // Move right to trigger scroll
    for _ in 0..10 {
        apply_key(&mut state, InputKey::Right, &mut false);
    }
    assert!(state.col_offset() > 0);

    // Move all the way back left
    for _ in 0..10 {
        apply_key(&mut state, InputKey::Left, &mut false);
    }

    assert_eq!(state.cursor_pos(), (0, 0));
    assert_eq!(state.col_offset(), 0);
}

// make sure tab calculations work

/// ---------------------------------------------------------------------------
/// cx_to_screen_col accounts for tab width
/// ---------------------------------------------------------------------------
#[test]
fn cx_to_screen_col_accounts_for_tabs() {
    let mut state = EditorState::new((80, 24));
    // Line: \t a b \n  →  chars: ['\t', 'a', 'b', '\n']
    state.load_document("\tab\n", Some("tab.txt"));

    // cx=0 → before the tab → screen col 0
    assert_eq!(state.cx_to_screen_col(0, 0), 0);
    // cx=1 → after the tab → screen col state.tab_width (4)
    assert_eq!(state.cx_to_screen_col(0, 1), state.tab_width);
    // cx=2 → after 'a' → state.tab_width + 1
    assert_eq!(state.cx_to_screen_col(0, 2), state.tab_width + 1);
    // cx=3 → after 'b' → state.tab_width + 2
    assert_eq!(state.cx_to_screen_col(0, 3), state.tab_width + 2);
}

/// ---------------------------------------------------------------------------
/// display_width_of_line counts tabs as state.tab_width columns
/// ---------------------------------------------------------------------------
#[test]
fn display_width_of_line_with_tabs() {
    let mut state = EditorState::new((80, 24));
    // Two tabs + "hi" + newline → 2*state.tab_width + 2 visible columns
    state.load_document("\t\thi\n", Some("tab.txt"));

    assert_eq!(state.display_width_of_line(0), 2 * state.tab_width + 2);
}

/// ---------------------------------------------------------------------------
/// get_slice expands tabs to spaces
/// ---------------------------------------------------------------------------
#[test]
fn get_slice_expands_tabs_to_spaces() {
    let mut state = EditorState::new((80, 24));
    state.load_document("\tab\n", Some("tab.txt"));

    let slice = state.get_slice(0, 80);
    // Tab should be expanded to state.tab_width spaces, followed by "ab"
    let expected = format!("{}ab", " ".repeat(state.tab_width));
    assert_eq!(slice, expected);
}

/// ---------------------------------------------------------------------------
/// Horizontal scrolling with tabs: a tab that doesn't fit truncates the line
/// ---------------------------------------------------------------------------
#[test]
fn horizontal_scroll_with_tabs() {
    // Screen width of 10 columns.
    // Line: "\tHello\n" → screen: [4 spaces]Hello = 9 cols
    let mut state = EditorState::new((10, 24));
    state.load_document("\tHello\n", Some("tab.txt"));

    // Move right past the tab and all of "Hello" (5 chars) → cx=6
    for _ in 0..6 {
        apply_key(&mut state, InputKey::Right, &mut false);
    }

    // cx=6, screen_col = state.tab_width + 5 = 9, fits in 10-col screen → no scroll
    assert_eq!(state.cursor_pos().0, 6);
    assert_eq!(state.cx_to_screen_col(0, 6), state.tab_width + 5);
    assert_eq!(state.col_offset(), 0);

    // With no scroll, get_slice should show the full rendered line.
    let slice = state.get_slice(0, 10);
    assert_eq!(slice, format!("{}Hello", " ".repeat(state.tab_width)));

    // Now use a narrower screen where the tab fits but scrolling is needed.
    // Screen width 6, line: "\tab\n" → [4 spaces]ab = 6 cols exactly.
    let mut state2 = EditorState::new((6, 24));
    state2.load_document("\tab\n", Some("tab2.txt"));

    // Move right twice: past tab and 'a' → cx=2, screen_col = state.tab_width + 1 = 5
    apply_key(&mut state2, InputKey::Right, &mut false);
    apply_key(&mut state2, InputKey::Right, &mut false);
    assert_eq!(state2.cursor_pos().0, 2);
    assert_eq!(state2.cx_to_screen_col(0, 2), state.tab_width + 1);
    // 5 < 6, so no scroll yet
    assert_eq!(state2.col_offset(), 0);

    // Full line fits exactly in 6 columns.
    let slice2 = state2.get_slice(0, 6);
    assert_eq!(slice2, format!("{}ab", " ".repeat(state.tab_width)));

    // Now: screen width 6, line "\t\tab\n" → 4+4+1+1 = 10 screen cols.
    // Move past both tabs → cx=2, screen_col=8, col_offset = 8+1-6 = 3.
    let mut state3 = EditorState::new((6, 24));
    state3.load_document("\t\tab\n", Some("tab3.txt"));

    apply_key(&mut state3, InputKey::Right, &mut false);
    apply_key(&mut state3, InputKey::Right, &mut false);
    assert_eq!(state3.col_offset(), 3);

    // get_slice with col_offset=3, width=6:
    //   skip_while: first \t has w=4, skip_cols(0)+4 <= 3? No → stop.
    //   render_to_width sees: \t(4), \t(4), 'a', 'b'
    //     \t → 4 cols used, fits in 6 → "    "
    //     \t → 4+4=8 > 6 → doesn't fit, stop.
    //   Result: 4 spaces only. The second tab truncates the visible line.
    let slice3 = state3.get_slice(0, 6);
    assert_eq!(slice3, " ".repeat(state.tab_width));
}

/// Changing tab_width is respected by display width calculations
#[test]
fn custom_tab_width_is_respected() {
    let mut state = EditorState::new((80, 24));
    state.tab_width = 8;
    state.load_document("\thi\n", Some("tab.txt"));

    // One tab at width 8 + "hi" (2 chars) = 10 columns
    assert_eq!(state.display_width_of_line(0), 10);

    // And with a narrow tab
    state.tab_width = 2;
    assert_eq!(state.display_width_of_line(0), 4);
}
