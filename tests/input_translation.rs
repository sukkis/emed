use emed_core::{EditorState, EditorCommand, InputKey};

#[test]
fn ctrl_q_quits_immediately() {
    let mut saw_ctrl_x = false;
    let cmd = EditorState::command_from_key(InputKey::Ctrl('q'), &mut saw_ctrl_x);
    assert_eq!(cmd, EditorCommand::Quit);
    assert!(!saw_ctrl_x);
}

#[test]
fn ctrl_x_arms_prefix_and_returns_noop() {
    let mut saw_ctrl_x = false;
    let cmd = EditorState::command_from_key(InputKey::Ctrl('x'), &mut saw_ctrl_x);
    assert_eq!(cmd, EditorCommand::NoOp);
    assert!(saw_ctrl_x);
}

#[test]
fn ctrl_x_then_ctrl_c_quits() {
    let mut saw_ctrl_x = false;

    let cmd1 = EditorState::command_from_key(InputKey::Ctrl('x'), &mut saw_ctrl_x);
    assert_eq!(cmd1, EditorCommand::NoOp);
    assert!(saw_ctrl_x);

    let cmd2 = EditorState::command_from_key(InputKey::Ctrl('c'), &mut saw_ctrl_x);
    assert_eq!(cmd2, EditorCommand::Quit);
    assert!(!saw_ctrl_x);
}

#[test]
fn ctrl_x_then_other_key_cancels_prefix() {
    let mut saw_ctrl_x = false;

    let _ = EditorState::command_from_key(InputKey::Ctrl('x'), &mut saw_ctrl_x);
    assert!(saw_ctrl_x);

    let cmd = EditorState::command_from_key(InputKey::Char('a'), &mut saw_ctrl_x);
    assert_eq!(cmd, EditorCommand::NoOp);
    assert!(!saw_ctrl_x);
}

#[test]
fn ctrl_c_alone_does_not_quit() {
    let mut saw_ctrl_x = false;
    let cmd = EditorState::command_from_key(InputKey::Ctrl('c'), &mut saw_ctrl_x);
    assert_eq!(cmd, EditorCommand::NoOp);
}