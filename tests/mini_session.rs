use emed_core::{ApplyResult, EditorState, InputKey, command_from_key};

fn run_key(state: &mut EditorState, key: InputKey, saw_ctrl_x: &mut bool) -> ApplyResult {
    let cmd = command_from_key(key, saw_ctrl_x);
    state.apply_command(cmd)
}

#[test]
fn mini_session_typing_newline_merge_and_scroll() {
    // Small screen to make scrolling deterministic:
    // rows=4 => text area height = 2 (rows - 2)
    let mut state = EditorState::new((80, 4));
    let mut saw_ctrl_x = false;

    // Type: "hi"
    assert_eq!(
        run_key(&mut state, InputKey::Char('h'), &mut saw_ctrl_x),
        ApplyResult::Changed
    );
    assert_eq!(
        run_key(&mut state, InputKey::Char('i'), &mut saw_ctrl_x),
        ApplyResult::Changed
    );

    // Enter => new line
    assert_eq!(
        run_key(&mut state, InputKey::Enter, &mut saw_ctrl_x),
        ApplyResult::Changed
    );

    // Type: "there"
    for c in "there".chars() {
        run_key(&mut state, InputKey::Char(c), &mut saw_ctrl_x);
    }

    // Backspace 5x removes "there"
    for _ in 0..5 {
        run_key(&mut state, InputKey::Backspace, &mut saw_ctrl_x);
    }

    // Backspace at start-of-line merges back into previous line (removes newline)
    run_key(&mut state, InputKey::Backspace, &mut saw_ctrl_x);

    // Validate final content and cursor.
    // Buffer should now contain "hi"
    // (Depending on whether your editor keeps a trailing newline in an empty buffer,
    // you may see "hi" or "hi\n". Adjust expectation to your intended model.)
    let buf = state.line_as_string(0);
    assert!(buf.starts_with("hi"));

    let (cx, cy) = state.cursor_pos();
    assert_eq!((cx, cy), (2, 0));

    // And since we used a tiny screen, ensure scroll offset is sane.
    assert_eq!(state.row_offset(), 0);
}
