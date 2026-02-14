use crossterm::event::{KeyEventKind, KeyModifiers};
use crossterm::{
    event::{Event, KeyCode, read},
    terminal,
};
use std::io::{self};

use emed_core::{EditorCommand, EditorState, InputKey, command_from_key};
mod ui;
use ui::EditorUi;

const VERSION: &str = "0.0.1";

// Convert crossterm events into a simplified, editor-owned input representation.
// This keeps `crossterm` types out of the core editor logic and makes keybinding logic testable.
fn to_input_key(event: Event) -> Option<InputKey> {
    let Event::Key(k) = event else {
        return None;
    };

    if k.kind != KeyEventKind::Press {
        return None;
    }

    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let alt = k.modifiers.contains(KeyModifiers::ALT);

    match k.code {
        KeyCode::Left => Some(InputKey::Left),
        KeyCode::Right => Some(InputKey::Right),
        KeyCode::Up => Some(InputKey::Up),
        KeyCode::Down => Some(InputKey::Down),
        KeyCode::Enter => Some(InputKey::Enter),
        KeyCode::Backspace => Some(InputKey::Backspace),
        KeyCode::Delete => Some(InputKey::Delete),

        // Characters: distinguish plain typing from control chords.
        KeyCode::Char(c) if ctrl => Some(InputKey::Ctrl(c)),

        // Ignore Alt-modified chars for now (often Meta / compose / terminal shortcuts).
        KeyCode::Char(_c) if alt => None,

        KeyCode::Char(c) => Some(InputKey::Char(c)),

        _ => None,
    }
}

/// Converts a raw terminal `Event` into an `EditorCommand`.
///
/// This is now a thin adapter:
/// `crossterm::Event` → `InputKey` → `EditorCommand` (via emed_core).
fn command_from_event(event: Event, saw_ctrl_x: &mut bool) -> EditorCommand {
    let Some(key) = to_input_key(event) else {
        return EditorCommand::NoOp;
    };

    command_from_key(key, saw_ctrl_x)
}

/// Executes an `EditorCommand`.
///
/// Intent:
/// - Keep side-effects (mutating `EditorState`,
///   drawing to the terminal via `EditorUi`) in one place.
/// - Make it explicit which commands cause a redraw.
///
/// How it fits together:
/// - The main loop reads input,
///   uses `command_from_event()` to translate it, then calls this.
/// - Returns `Ok(true)` when the command requests program termination,
///   so the caller can `break`.
fn apply_command(
    cmd: EditorCommand,
    ui: &mut EditorUi,
    state: &mut EditorState,
) -> io::Result<bool> {
    match cmd {
        EditorCommand::Quit => return Ok(true),
        EditorCommand::MoveLeft => ui.left(state)?,
        EditorCommand::MoveRight => ui.right(state)?,
        EditorCommand::MoveUp => ui.up(state)?,
        EditorCommand::MoveDown => ui.down(state)?,
        EditorCommand::InsertChar(c) => {
            state.insert_char(c);
            ui.draw_screen(state)?;
        }
        EditorCommand::InsertNewline => {
            state.insert_newline();
            ui.draw_screen(state)?;
        }
        EditorCommand::DeleteChar => {
            state.delete_char();
            ui.draw_screen(state)?;
        }
        EditorCommand::Backspace => {
            state.backspace();
            ui.draw_screen(state)?;
        }
        EditorCommand::NoOp => {}
    }
    Ok(false)
}

fn main() -> io::Result<()> {
    let stdout = io::stdout();
    let mut ui = EditorUi::new(stdout);

    terminal::enable_raw_mode()?;

    ui.print_editor_version()?;
    ui.clear_screen()?;
    ui.initialise_editing()?;

    let screen_size = terminal::size()?;
    let mut state = EditorState::new(screen_size);
    ui.draw_screen(&state)?;

    let mut saw_ctrl_x = false;

    // Main event loop architecture ("read → translate → apply").
    //
    // We keep the interactive part of the editor deliberately split into three steps:
    //
    // 1) Read: `crossterm::event::read()` blocks until the terminal produces an `Event`.
    // 2) Translate: `command_from_event(event, &mut saw_ctrl_x)` turns that low-level event into an
    //    `EditorCommand` (our small, editor-centric vocabulary). This is also where multi-key
    //    chords live: `saw_ctrl_x` remembers whether the previous keypress was `Ctrl+X`, so the
    //    next key can complete `Ctrl+X` then `Ctrl+C` to quit.
    // 3) Apply: `apply_command(cmd, &mut ui, &mut state)` performs the command by mutating the
    //    `EditorState` and redrawing via `EditorUi` when needed. It returns `true` when we should
    //    exit the loop.
    //
    // This structure keeps terminal-specific details at the edges, and concentrates editor
    // behavior (keybindings + effects) into small, readable functions.
    loop {
        let event = read()?;
        let cmd = command_from_event(event, &mut saw_ctrl_x);
        let should_quit = apply_command(cmd, &mut ui, &mut state)?;
        if should_quit {
            break;
        }
    }

    ui.clean_up()?;
    Ok(())
}
