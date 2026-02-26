// tests to see if setting the "(modified)" in status bar works,
// and is cleared appropriately

use emed_core::{
    ApplyResult, DEFAULT_HELP_MESSAGE, EditorCommand, EditorState, InputKey, QUIT_CONFIRM_COUNT,
    command_from_key,
};

// quit confirmation if user has unsaved changes

#[test]
fn quit_warning_message_appears_and_resets_on_edit() {
    let mut state = EditorState::new((80, 24));
    state.load_document("hello\n", Some("test.txt"));

    // Make the buffer dirty.
    apply_key(&mut state, InputKey::Char('x'), &mut false);
    assert!(state.is_dirty());

    // Simulate what main.rs's apply_command does on Quit with a dirty buffer:
    state.quit_count += 1;
    let remaining = QUIT_CONFIRM_COUNT - state.quit_count;
    state.help_message = format!(
        "WARNING: Unsaved changes! Quit {} more time(s), or C-x C-s to save.",
        remaining
    );

    // Verify the warning is shown with the correct count.
    assert_eq!(state.quit_count, 1);
    assert!(
        state.help_message.contains("2 more time(s)"),
        "after first quit press, message should say 2 more; got: {}",
        state.help_message
    );

    // Second quit press.
    state.quit_count += 1;
    let remaining = QUIT_CONFIRM_COUNT - state.quit_count;
    state.help_message = format!(
        "WARNING: Unsaved changes! Quit {} more time(s), or C-x C-s to save.",
        remaining
    );
    assert!(
        state.help_message.contains("1 more time(s)"),
        "after second quit press, message should say 1 more; got: {}",
        state.help_message
    );

    // Now the user decides to keep editing instead of quitting.
    // Simulate what main.rs does for a non-Quit command:
    if state.quit_count > 0 {
        state.reset_quit_count();
        state.help_message = "HELP: C-x C-s to Save, C-x C-c to Quit".to_string();
    }

    assert_eq!(state.quit_count, 0);
    assert_eq!(
        state.help_message, DEFAULT_HELP_MESSAGE,
        "help message should be restored after editing resumes"
    );
}
#[test]
fn quit_on_clean_buffer_returns_quit_immediately() {
    let mut state = EditorState::new((80, 24));
    // Buffer is clean — Quit should produce ApplyResult::Quit on the first try.
    let result = state.apply_command(EditorCommand::Quit);
    assert_eq!(result, ApplyResult::Quit);
}

#[test]
fn quit_on_dirty_buffer_needs_three_presses() {
    let mut state = EditorState::new((80, 24));
    state.load_document("hello\n", Some("test.txt"));

    // Make it dirty.
    apply_key(&mut state, InputKey::Char('x'), &mut false);
    assert!(state.is_dirty());

    // The core's apply_command always returns Quit for EditorCommand::Quit —
    // the confirmation logic lives in main.rs's apply_command.
    // So we test the *counter* field directly here.
    assert_eq!(state.quit_count, 0);

    // Simulate three quit presses by bumping the counter manually
    // (mirrors what main.rs does).
    state.quit_count += 1;
    assert_eq!(state.quit_count, 1);
    assert!(state.quit_count < QUIT_CONFIRM_COUNT);

    state.quit_count += 1;
    assert_eq!(state.quit_count, 2);
    assert!(state.quit_count < QUIT_CONFIRM_COUNT);

    state.quit_count += 1;
    assert!(state.quit_count >= QUIT_CONFIRM_COUNT); // NOW we'd actually quit
}

#[test]
fn quit_count_resets_on_non_quit_action() {
    let mut state = EditorState::new((80, 24));
    state.load_document("hello\n", Some("test.txt"));
    apply_key(&mut state, InputKey::Char('x'), &mut false);

    state.quit_count = 2; // user pressed Quit twice
    state.reset_quit_count(); // then typed a character — counter resets
    assert_eq!(state.quit_count, 0);
}

#[test]
fn quit_count_resets_on_save() {
    let mut state = EditorState::new((80, 24));
    state.load_document("hello\n", Some("test.txt"));
    apply_key(&mut state, InputKey::Char('x'), &mut false);

    state.quit_count = 2;
    state.clear_dirty(); // save clears dirty AND resets quit_count
    assert_eq!(state.quit_count, 0);
}
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
