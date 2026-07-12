use emed_core::{EditorCommand, InputKey, command_from_key, escapes_search};

#[test]
fn ctrl_q_quits_immediately() {
    let mut saw_ctrl_x = false;
    let cmd = command_from_key(InputKey::Ctrl('q'), &mut saw_ctrl_x);
    assert_eq!(cmd, EditorCommand::Quit);
    assert!(!saw_ctrl_x);
}

#[test]
fn ctrl_x_arms_prefix_and_returns_noop() {
    let mut saw_ctrl_x = false;
    let cmd = command_from_key(InputKey::Ctrl('x'), &mut saw_ctrl_x);
    assert_eq!(cmd, EditorCommand::NoOp);
    assert!(saw_ctrl_x);
}

#[test]
fn ctrl_x_then_ctrl_c_quits() {
    let mut saw_ctrl_x = false;

    let cmd1 = command_from_key(InputKey::Ctrl('x'), &mut saw_ctrl_x);
    assert_eq!(cmd1, EditorCommand::NoOp);
    assert!(saw_ctrl_x);

    let cmd2 = command_from_key(InputKey::Ctrl('c'), &mut saw_ctrl_x);
    assert_eq!(cmd2, EditorCommand::Quit);
    assert!(!saw_ctrl_x);
}

#[test]
fn ctrl_x_then_other_key_cancels_prefix() {
    let mut saw_ctrl_x = false;

    let _ = command_from_key(InputKey::Ctrl('x'), &mut saw_ctrl_x);
    assert!(saw_ctrl_x);

    let cmd = command_from_key(InputKey::Char('a'), &mut saw_ctrl_x);
    assert_eq!(cmd, EditorCommand::NoOp);
    assert!(!saw_ctrl_x);
}

#[test]
fn ctrl_c_alone_does_not_quit() {
    let mut saw_ctrl_x = false;
    let cmd = command_from_key(InputKey::Ctrl('c'), &mut saw_ctrl_x);
    assert_eq!(cmd, EditorCommand::NoOp);
}

#[test]
fn ctrl_x_then_ctrl_s_saves_file() {
    let mut saw_ctrl_x = false;

    let cmd1 = command_from_key(InputKey::Ctrl('x'), &mut saw_ctrl_x);
    assert_eq!(cmd1, EditorCommand::NoOp);
    assert!(saw_ctrl_x);

    let cmd2 = command_from_key(InputKey::Ctrl('s'), &mut saw_ctrl_x);
    assert_eq!(cmd2, EditorCommand::SaveFile);
    assert!(!saw_ctrl_x);
}

#[test]
fn plain_ctrl_s_starts_search() {
    // Regression guard: this must NOT collide with C-x C-s (save), which is
    // covered by `ctrl_x_then_ctrl_s_saves_file` above — that test staying
    // green alongside this one proves the prefix check still separates them.
    let mut saw_ctrl_x = false;
    let cmd = command_from_key(InputKey::Ctrl('s'), &mut saw_ctrl_x);
    assert_eq!(cmd, EditorCommand::StartSearch);
    assert!(!saw_ctrl_x);
}

#[test]
fn ctrl_q_and_ctrl_x_escape_a_search() {
    // These are the keys that should cancel an active search and fall
    // through to normal handling, so quitting is never unreachable.
    assert!(escapes_search(InputKey::Ctrl('q')));
    assert!(escapes_search(InputKey::Ctrl('x')));
}

#[test]
fn ctrl_g_does_not_escape_a_search() {
    // C-g has its own meaning while searching (cancel back to origin,
    // but stay in the editor) — it must not be treated as an escape key,
    // or handle_search_key's own C-g handling would never run.
    assert!(!escapes_search(InputKey::Ctrl('g')));
}

#[test]
fn typing_a_character_does_not_escape_a_search() {
    assert!(!escapes_search(InputKey::Char('a')));
}

#[test]
fn ctrl_x_then_ctrl_s_does_not_interfere_with_subsequent_ctrl_x_ctrl_c() {
    let mut saw_ctrl_x = false;

    // First: C-x C-s → SaveFile
    let _ = command_from_key(InputKey::Ctrl('x'), &mut saw_ctrl_x);
    let cmd = command_from_key(InputKey::Ctrl('s'), &mut saw_ctrl_x);
    assert_eq!(cmd, EditorCommand::SaveFile);
    assert!(!saw_ctrl_x);

    // Then: C-x C-c should still work → Quit
    let _ = command_from_key(InputKey::Ctrl('x'), &mut saw_ctrl_x);
    let cmd = command_from_key(InputKey::Ctrl('c'), &mut saw_ctrl_x);
    assert_eq!(cmd, EditorCommand::Quit);
    assert!(!saw_ctrl_x);
}
