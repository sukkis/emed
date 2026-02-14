use crossterm::event::{KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::{
    event::{Event, KeyCode, read},
    terminal,
};
use std::io::{self};

use emed_core::EditorState;
mod ui;
use ui::EditorUi;

const VERSION: &str = "0.0.1";

/// High-level actions the editor understands.
///
/// Intent:
/// - Keep terminal input (`crossterm::Event`) out of the editor core logic.
/// - Make the main loop a simple "read event -> translate -> apply" pipeline.
///
/// How it fits together:
/// - `command_from_event()` translates raw input events into one of these commands.
/// - `apply_command()` performs the command by mutating `EditorState` and/or redrawing via `EditorUi`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorCommand {
    Quit,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    InsertChar(char),
    InsertNewline,
    //   DeleteChar,
    NoOp,
}

/// - Prevent "control chords" (Ctrl+something, Alt+something) from being inserted as text.
/// - Only treat actual character keys as text when the user is typing normally.
fn is_plain_text_key(k: &KeyEvent) -> bool {
    k.kind == KeyEventKind::Press
        && !k.modifiers.contains(KeyModifiers::CONTROL)
        && !k.modifiers.contains(KeyModifiers::ALT)
}

/// Converts a raw terminal `Event` into an `EditorCommand`.
///
/// Intent:
/// - Centralize all keybinding decisions in one place.
/// - Keep the rest of the program from caring about `crossterm` details.
/// - Implement multi-key "chords" such as Emacs-style `Ctrl+X` then `Ctrl+C`.
///
/// How it fits together:
/// - The main loop calls this for each incoming event.
/// - The `saw_ctrl_x` flag is *state across events*: it remembers whether the previous keypress
///   was `Ctrl+X`, so we can interpret the *next* keypress accordingly.
/// - The returned `EditorCommand` is then handed to `apply_command()`.
fn command_from_event(event: Event, saw_ctrl_x: &mut bool) -> EditorCommand {
    let Event::Key(k) = event else {
        return EditorCommand::NoOp;
    };

    if k.kind != KeyEventKind::Press {
        return EditorCommand::NoOp;
    }

    // Quit on Ctrl-Q. Alternative to C-x C-c.
    if k.kind == KeyEventKind::Press
        && k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('q') {
        return EditorCommand::Quit;
    }

    // Ctrl-x prefix handling (Emacs-style chord starter).
    // If we see Ctrl+X, we "arm" the prefix and consume this keypress.
    let is_ctrl_x = k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('x');
    if is_ctrl_x {
        *saw_ctrl_x = true;
        return EditorCommand::NoOp;
    }

    // If a Ctrl+X prefix was armed by the previous keypress, interpret this key as the second half
    // of the chord and then clear the prefix.
    if *saw_ctrl_x {
        *saw_ctrl_x = false;
        let is_ctrl_c = k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c');
        if is_ctrl_c {
            return EditorCommand::Quit;
        } else {
            return EditorCommand::NoOp;
        }
    }

    // Normal (non-chord) bindings: movement + plain text insertion.
    match k.code {
        KeyCode::Left => EditorCommand::MoveLeft,
        KeyCode::Right => EditorCommand::MoveRight,
        KeyCode::Up => EditorCommand::MoveUp,
        KeyCode::Down => EditorCommand::MoveDown,
        KeyCode::Enter => EditorCommand::InsertNewline,
        KeyCode::Char(c) if is_plain_text_key(&k) => EditorCommand::InsertChar(c),
        _ => EditorCommand::NoOp,
    }
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
        //        EditorCommand::DeleteChar => ,
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
