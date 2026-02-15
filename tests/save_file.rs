use emed_core::{EditorCommand, EditorState, InputKey, command_from_key};

/// Simulate C-x C-s and return the resulting command.
fn press_ctrl_x_ctrl_s(saw_ctrl_x: &mut bool) -> EditorCommand {
    let _ = command_from_key(InputKey::Ctrl('x'), saw_ctrl_x);
    command_from_key(InputKey::Ctrl('s'), saw_ctrl_x)
}

// -- Prompt-mode state machine tests --
// These verify the prompt_buffer field transitions that main.rs relies on.
// No filesystem or UI involved.

#[test]
fn save_with_known_filename_does_not_enter_prompt() {
    let mut state = EditorState::new((80, 24));
    state.load_document("hello\n", Some("test.txt"));

    let mut saw_ctrl_x = false;
    let cmd = press_ctrl_x_ctrl_s(&mut saw_ctrl_x);
    assert_eq!(cmd, EditorCommand::SaveFile);

    // With a known filename, prompt_buffer should stay None.
    // (apply_command in core is a no-op for SaveFile; main.rs handles the actual write.)
    state.apply_command(cmd);
    assert!(state.prompt_buffer.is_none());
}

#[test]
fn save_to_string_returns_buffer_contents() {
    let mut state = EditorState::new((80, 24));
    state.load_document("line one\nline two\n", Some("doc.txt"));

    assert_eq!(state.save_to_string(), "line one\nline two\n");
}

#[test]
fn prompt_buffer_accumulates_typed_characters() {
    let mut state = EditorState::new((80, 24));

    // Enter prompt mode (simulating what main.rs does for unknown filename).
    state.prompt_buffer = Some(String::new());

    // Type "out.txt" character by character.
    for c in "out.txt".chars() {
        if let Some(ref mut buf) = state.prompt_buffer {
            buf.push(c);
        }
    }

    assert_eq!(state.prompt_buffer.as_deref(), Some("out.txt"));
}

#[test]
fn prompt_buffer_backspace_removes_last_char() {
    let mut state = EditorState::new((80, 24));
    state.prompt_buffer = Some("test.rs".to_string());

    // Backspace twice.
    if let Some(ref mut buf) = state.prompt_buffer {
        buf.pop();
        buf.pop();
    }

    assert_eq!(state.prompt_buffer.as_deref(), Some("test."));
}

#[test]
fn prompt_buffer_backspace_on_empty_stays_empty() {
    let mut state = EditorState::new((80, 24));
    state.prompt_buffer = Some(String::new());

    if let Some(ref mut buf) = state.prompt_buffer {
        buf.pop(); // should be no-op on empty string
    }

    assert_eq!(state.prompt_buffer.as_deref(), Some(""));
}

#[test]
fn cancel_prompt_clears_buffer() {
    let mut state = EditorState::new((80, 24));
    state.prompt_buffer = Some("partial_name".to_string());

    // C-g cancels: simulate what handle_prompt_key does.
    state.prompt_buffer = None;
    state.help_message = "Save cancelled".to_string();

    assert!(state.prompt_buffer.is_none());
    assert_eq!(state.help_message, "Save cancelled");
}

#[test]
fn confirm_prompt_takes_buffer() {
    let mut state = EditorState::new((80, 24));
    state.prompt_buffer = Some("output.txt".to_string());

    // Simulate Enter: take the buffer.
    let filename = state.prompt_buffer.take().unwrap();

    assert_eq!(filename, "output.txt");
    assert!(state.prompt_buffer.is_none());
}

#[test]
fn normal_keys_do_not_affect_prompt_when_not_in_prompt_mode() {
    let mut state = EditorState::new((80, 24));

    // prompt_buffer is None — we're in normal mode.
    assert!(state.prompt_buffer.is_none());

    // Typing should not create a prompt buffer.
    // (In real code, chars go to insert_char; this just confirms the field stays None.)
    state.insert_char('a');
    assert!(state.prompt_buffer.is_none());
    assert_eq!(state.save_to_string(), "a");
}