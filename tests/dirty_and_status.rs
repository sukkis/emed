// tests to see if setting the "(modified)" in status bar works,
// and is cleared appropriately

use emed_core::{ApplyResult, EditorState, InputKey, command_from_key};

/// Helper – run a single key through the command pipeline.
fn apply_key(state: &mut EditorState, key: InputKey, saw_ctrl_x: &mut bool) -> ApplyResult {
    let cmd = command_from_key(key, saw_ctrl_x);
    state.apply_command(cmd)
}
fn fit_to_width(s: &str, width: usize) -> String {
    let mut out: String = s.chars().take(width).collect();
    let len = out.chars().count();
    if len < width {
        out.extend(std::iter::repeat(' ').take(width - len));
    }
    out
}

/*==========================================================================*
 * Dirty‑flag basics
 *==========================================================================*/
#[test]
fn dirty_flag_flips_on_edit_and_resets_on_load() {
    let mut state = EditorState::new((80, 24));

    // Fresh editor → clean.
    assert!(!state.is_dirty(), "new editor should start clean");

    // Any mutating command makes it dirty.
    apply_key(&mut state, InputKey::Char('a'), &mut false);
    assert!(state.is_dirty(), "insert_char must set dirty");

    // Loading a new document must clear the flag, even if it was set.
    state.load_document("some text\n", Some("tmp.txt"));
    assert!(!state.is_dirty(), "load_document must reset dirty flag");
}

/*==========================================================================*
 * Helper that builds the exact status line string
 *==========================================================================*/
fn build_status_line(state: &EditorState, cols: u16, rows: u16) -> String {
    // This reproduces the logic from `EditorUi::queue_status_information`
    // but returns the final string instead of sending it to the terminal.
    let filetype = state.file_type.as_str();

    // cursor coordinates (1‑based for the user)
    let (cx, cy) = state.cursor_pos();
    let col_disp = cx + 1;
    let row_disp = cy + 1;

    // left‑hand part (file info + optional dirty flag)
    let mut left = format!(
        "{}: {} lines, {} chars",
        filetype,
        state.index_of_last_line() + 1,
        state.char_count()
    );
    if state.is_dirty() {
        left.push_str(" (modified)");
    }

    // right‑hand part (coordinates)
    let right = format!("col: {}, row: {}", col_disp, row_disp);

    // combine and pad to the full terminal width
    let combined = format!("{} {}", left, right);
    fit_to_width(&combined, cols as usize)
}

/*==========================================================================*
 * Status line contains coordinates and dirty marker
 *==========================================================================*/
#[test]
fn status_line_shows_coords_and_dirty_marker_correctly() {
    // Small terminal – 40 columns, 5 rows (status line is at row 3).
    let cols = 80u16;
    let rows = 5u16;

    let mut state = EditorState::new((cols, rows));
    state.load_document("first line\nsecond line\n", Some("demo.txt"));

    // Move cursor to column 3 (zero‑based → user sees col 4) on line 1.
    state.set_cursor(3, 1);

    // ---- clean buffer ----------------------------------------------------
    let clean = build_status_line(&state, cols, rows);
    assert!(
        clean.contains("col: 4, row: 2"),
        "clean status line must contain the coordinates"
    );
    assert!(
        !clean.contains("(modified)"),
        "clean buffer must not show the modified flag"
    );

    // ---- make it dirty ---------------------------------------------------
    apply_key(&mut state, InputKey::Char('x'), &mut false);
    let dirty = build_status_line(&state, cols, rows);
    assert!(
        dirty.contains("(modified)"),
        "dirty buffer must show the modified flag"
    );
    // The insertion moved the cursor one column to the right.
    assert!(
        dirty.contains("col: 5, row: 2"),
        "coordinates must update after the edit"
    );
}
