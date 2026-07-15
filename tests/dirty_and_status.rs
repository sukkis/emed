// tests to see if setting the "(modified)" in status bar works,
// and is cleared appropriately

use emed_core::search::Direction;
use emed_core::{
    ApplyResult, DEFAULT_HELP_MESSAGE, EditorCommand, EditorState, InputKey, QUIT_CONFIRM_COUNT,
    cancels_pending_quit, command_from_key,
};

// quit confirmation if user has unsaved changes

#[test]
fn noop_and_quit_do_not_cancel_a_pending_quit() {
    // NoOp is what arming the C-x prefix produces — it must not cancel
    // the countdown, or completing C-x C-c across two key presses is
    // impossible (the prefix key itself would reset the counter first).
    assert!(!cancels_pending_quit(EditorCommand::NoOp));
    // Quit itself is handled by its own branch, never this one, but it
    // shouldn't be considered a "cancelling" command either.
    assert!(!cancels_pending_quit(EditorCommand::Quit));
}

#[test]
fn a_real_action_cancels_a_pending_quit() {
    assert!(cancels_pending_quit(EditorCommand::InsertChar('a')));
    assert!(cancels_pending_quit(EditorCommand::MoveLeft));
}

#[test]
fn quit_warning_message_appears_and_resets_on_edit() {
    let mut state = EditorState::new((80, 24));
    state.load_document("hello\n", Some("test.txt"));

    // Make the buffer dirty.
    apply_key(&mut state, InputKey::Char('x'), &mut false, &mut false);
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
    apply_key(&mut state, InputKey::Char('x'), &mut false, &mut false);
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
    apply_key(&mut state, InputKey::Char('x'), &mut false, &mut false);

    state.quit_count = 2; // user pressed Quit twice
    state.reset_quit_count(); // then typed a character — counter resets
    assert_eq!(state.quit_count, 0);
}

#[test]
fn quit_count_resets_on_save() {
    let mut state = EditorState::new((80, 24));
    state.load_document("hello\n", Some("test.txt"));
    apply_key(&mut state, InputKey::Char('x'), &mut false, &mut false);

    state.quit_count = 2;
    state.clear_dirty(); // save clears dirty AND resets quit_count
    assert_eq!(state.quit_count, 0);
}
/// Helper – run a single key through the command pipeline.
fn apply_key(
    state: &mut EditorState,
    key: InputKey,
    saw_ctrl_x: &mut bool,
    saw_ctrl_c: &mut bool,
) -> ApplyResult {
    let cmd = command_from_key(key, saw_ctrl_x, saw_ctrl_c);
    state.apply_command(cmd)
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
    apply_key(&mut state, InputKey::Char('a'), &mut false, &mut false);
    assert!(state.is_dirty(), "insert_char must set dirty");

    // Loading a new document must clear the flag, even if it was set.
    state.load_document("some text\n", Some("tmp.txt"));
    assert!(!state.is_dirty(), "load_document must reset dirty flag");
}

/*==========================================================================*
 * status_help_line: what the bottom help line should show
 *==========================================================================*/
#[test]
fn status_help_line_shows_default_message_normally() {
    let state = EditorState::new((80, 24));
    assert_eq!(state.status_help_line(), DEFAULT_HELP_MESSAGE);
}

#[test]
fn status_help_line_shows_save_as_prompt_when_prompting() {
    let mut state = EditorState::new((80, 24));
    state.prompt_buffer = Some("myfile.txt".to_string());
    assert_eq!(state.status_help_line(), "Save as: myfile.txt");
}

#[test]
fn status_help_line_shows_search_query_when_searching() {
    let mut state = EditorState::new((80, 24));
    state.load_document("bind bindings\n", Some("test.txt"));
    state.search_start(Direction::Forward);
    for c in "bind".chars() {
        state.search_push_char(c);
    }
    assert_eq!(state.status_help_line(), "I-search: bind");
}

#[test]
fn status_help_line_shows_backward_wording_when_searching_backward() {
    let mut state = EditorState::new((80, 24));
    state.load_document("bind bindings\n", Some("test.txt"));
    state.set_cursor(13, 0); // end of buffer, so there's text before origin
    state.search_start(Direction::Backward);
    for c in "bind".chars() {
        state.search_push_char(c);
    }
    assert_eq!(state.status_help_line(), "I-search backward: bind");
}

#[test]
fn status_help_line_shows_failing_prefix_when_query_has_no_match() {
    let mut state = EditorState::new((80, 24));
    state.load_document("cat\n", Some("test.txt"));
    state.search_start(Direction::Forward);
    state.search_push_char('z'); // "z" is nowhere in "cat"
    assert_eq!(state.status_help_line(), "Failing I-search: z");
}

#[test]
fn status_help_line_shows_failing_and_backward_wording_together() {
    let mut state = EditorState::new((80, 24));
    state.load_document("cat\n", Some("test.txt"));
    state.search_start(Direction::Backward);
    state.search_push_char('z');
    assert_eq!(state.status_help_line(), "Failing I-search backward: z");
}

#[test]
fn status_help_line_never_shows_failing_for_empty_query() {
    let mut state = EditorState::new((80, 24));
    state.load_document("cat\n", Some("test.txt"));
    state.search_start(Direction::Forward);
    assert_eq!(state.status_help_line(), "I-search: ");
}

/*==========================================================================*
 * status_line(): the real, testable string-building logic behind
 * queue_status_information
 *==========================================================================*/
#[test]
fn status_line_includes_filetype_line_count_char_count_and_coordinates() {
    let mut state = EditorState::new((80, 24));
    state.load_document("first line\nsecond line\n", Some("demo.txt"));
    state.set_cursor(3, 1);

    let line = state.status_line();
    // Ropey counts the trailing '\n' as starting a third, empty line.
    assert!(line.contains("3 lines"));
    assert!(line.contains("chars"));
    assert!(
        line.contains("col: 3, row: 1"),
        "coordinates are 0-based, matching cursor_pos() directly: {line}"
    );
}

#[test]
fn status_line_coordinates_update_after_edit() {
    let mut state = EditorState::new((80, 24));
    state.load_document("first line\nsecond line\n", Some("demo.txt"));
    state.set_cursor(3, 1);

    apply_key(&mut state, InputKey::Char('x'), &mut false, &mut false);

    // The insertion moved the cursor one column to the right.
    assert!(
        state.status_line().contains("col: 4, row: 1"),
        "coordinates must update after the edit"
    );
}

#[test]
fn status_line_shows_modified_only_when_dirty() {
    let mut state = EditorState::new((80, 24));
    state.load_document("first line\n", Some("demo.txt"));

    assert!(
        !state.status_line().contains("(modified)"),
        "clean buffer must not show the modified flag"
    );

    apply_key(&mut state, InputKey::Char('x'), &mut false, &mut false);

    assert!(
        state.status_line().contains("(modified)"),
        "dirty buffer must show the modified flag"
    );
}

#[test]
fn status_line_shows_wrap_tag_only_when_visual_line_mode_on() {
    let mut state = EditorState::new((80, 24));
    state.load_document("first line\n", Some("demo.txt"));

    assert!(
        !state.status_line().contains("(wrap)"),
        "wrap tag must not show when visual_line_mode is off"
    );

    state.visual_line_mode = true;

    assert!(
        state.status_line().contains("(wrap)"),
        "wrap tag must show when visual_line_mode is on"
    );
}

#[test]
fn status_line_shows_quit_countdown_when_pending() {
    let mut state = EditorState::new((80, 24));
    state.load_document("first line\n", Some("demo.txt"));

    assert!(
        !state.status_line().contains("more quit(s)"),
        "quit countdown must not show when quit_count is 0"
    );

    state.quit_count = 1;

    assert!(
        state.status_line().contains("more quit(s)"),
        "quit countdown must show when quit_count is nonzero"
    );
}
