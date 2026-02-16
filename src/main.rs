use crossterm::event::{KeyEventKind, KeyModifiers};
use crossterm::{
    event::{Event, KeyCode, read},
    terminal,
};
use emed_core::{
    DEFAULT_HELP_MESSAGE, EditorCommand, EditorState, InputKey, QUIT_CONFIRM_COUNT,
    command_from_key,
};
use std::io::{self};
mod settings;
mod theme;
mod ui;
use crate::theme::Theme;
use clap::Parser;
use std::path::PathBuf;
use ui::EditorUi;

const VERSION: &str = "0.0.1";

#[derive(Parser, Debug)]
#[command(name = "emed", version = VERSION)]
struct Args {
    /// File to open
    file: Option<PathBuf>,
}

/// Handle a keypress while the editor is in prompt mode (e.g. "Save as").
///
/// Returns `true` if the prompt is finished (confirmed or cancelled),
/// so the caller knows to return to normal event routing.
fn handle_prompt_key(
    key: InputKey,
    ui: &mut EditorUi,
    state: &mut EditorState,
) -> io::Result<bool> {
    match key {
        InputKey::Enter => {
            // Take the prompt buffer and use it as the filename.
            if let Some(input) = state.prompt_buffer.take() {
                let input = input.trim().to_string();
                if input.is_empty() {
                    state.help_message = "Save cancelled (empty filename)".to_string();
                } else {
                    let path = std::path::Path::new(&input);
                    match write_to_file(path, state) {
                        Ok(()) => {
                            state.filename = input;
                            state.clear_dirty();
                            state.help_message = "File saved".to_string();
                        }
                        Err(e) => {
                            state.help_message = format!("Save failed: {}", e);
                        }
                    }
                }
            }
            ui.draw_screen(state)?;
            Ok(true)
        }
        InputKey::Ctrl('g') => {
            // Cancel prompt (Emacs-style C-g).
            state.prompt_buffer = None;
            state.help_message = "Save cancelled".to_string();
            ui.draw_screen(state)?;
            Ok(true)
        }
        InputKey::Char(c) => {
            if let Some(ref mut buf) = state.prompt_buffer {
                buf.push(c);
            }
            ui.draw_screen(state)?;
            Ok(false)
        }
        InputKey::Backspace => {
            if let Some(ref mut buf) = state.prompt_buffer {
                buf.pop();
            }
            ui.draw_screen(state)?;
            Ok(false)
        }
        _ => {
            // Ignore other keys while in prompt mode.
            Ok(false)
        }
    }
}

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
        EditorCommand::Quit => {
            if state.is_dirty() {
                state.quit_count += 1;
                if state.quit_count >= QUIT_CONFIRM_COUNT {
                    return Ok(true); // actually quit
                }
                let remaining = QUIT_CONFIRM_COUNT - state.quit_count;
                state.help_message = format!(
                    "WARNING: Unsaved changes! Quit {} more time(s), or C-x C-s to save.",
                    remaining
                );
                ui.draw_screen(state)?;
                return Ok(false);
            }
            return Ok(true);
        }
        // Any non-Quit command resets the quit counter.
        _ => {
            if state.quit_count > 0 {
                state.reset_quit_count();
                state.help_message = DEFAULT_HELP_MESSAGE.to_string();
            }
        }
    }
    match cmd {
        EditorCommand::Quit => unreachable!(), // handled separately above
        EditorCommand::SaveFile => {
            if state.filename != "-" {
                let path = std::path::Path::new(&state.filename);
                match write_to_file(path, state) {
                    Ok(()) => {
                        state.help_message = "File saved".to_string();
                        state.clear_dirty();
                    }
                    Err(e) => {
                        state.help_message = format!("Save failed: {}", e);
                    }
                }
            } else {
                // No filename known — enter prompt mode.
                state.prompt_buffer = Some(String::new());
            }
            ui.draw_screen(state)?;
        }
        EditorCommand::PromptSaveAs => {
            // Always enter prompt mode, even if we already have a filename.
            state.prompt_buffer = Some(String::new());
            ui.draw_screen(state)?;
        }
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

/// Write the editor buffer to a file.
///
/// This is the operation done as a result of "Save" or "Save as".
/// Caller is responsible for determining the path (from the known filename
/// or from the "Save as" prompt).
fn write_to_file(path: &std::path::Path, state: &EditorState) -> io::Result<()> {
    std::fs::write(path, state.save_to_string())
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let stdout = io::stdout();

    // get user configuration from ./settings.toml, if it exists
    let toml_content = std::fs::read_to_string("settings.toml").unwrap_or_default();
    let settings = settings::load_settings(&toml_content);
    let user_defined_theme = settings.get("theme").unwrap();
    let user_defined_tab_width = settings.get("tab_width").unwrap();
    let mut ui = EditorUi::new(stdout, Theme::from_name(user_defined_theme));

    terminal::enable_raw_mode()?;

    let screen_size = terminal::size()?;

    ui.print_editor_version(screen_size.0, screen_size.1)?;
    ui.initialise_editing()?;

    let mut state = EditorState::new(screen_size);
    state.tab_width = user_defined_tab_width.parse::<usize>().unwrap();

    // If we have an argument, load the file.
    if let Some(path) = args.file.as_deref() {
        let contents = std::fs::read_to_string(path)?;
        state.load_document(&contents, path.to_str());
    }

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

        // If we're in prompt mode, route keypresses to the prompt handler
        // instead of the normal command pipeline.
        if state.prompt_buffer.is_some() {
            if let Some(key) = to_input_key(event) {
                handle_prompt_key(key, &mut ui, &mut state)?;
            }
            continue;
        }

        let cmd = command_from_event(event, &mut saw_ctrl_x);
        let should_quit = apply_command(cmd, &mut ui, &mut state)?;
        if should_quit {
            break;
        }
    }

    ui.clean_up()?;
    Ok(())
}
