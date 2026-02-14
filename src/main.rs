use crossterm::event::{KeyEventKind, KeyModifiers};
use crossterm::{
    event::{Event, KeyCode, read},
    terminal,
};
use std::io::{self};

use emed_core::EditorState;
mod ui;
use ui::EditorUi;
const VERSION: &str = "0.0.1";

fn main() -> io::Result<()> {


    let stdout = io::stdout();
    let mut ui = EditorUi::new(stdout);

    terminal::enable_raw_mode()?;
    ui.clean_up()?;
    ui.print_editor_version()?;
    ui.initialise_screen()?;
    ui.initialise_editing()?;

    let screen_size = terminal::size()?;
    let mut state = EditorState::new(screen_size);
    ui.draw_screen(&state)?;

    let mut saw_ctrl_x = false;
    loop {
        match read()? {
            Event::Key(key_event)
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && key_event.code == KeyCode::Char('x') =>
            {
                saw_ctrl_x = true;
            }

            Event::Key(key_event)
                if saw_ctrl_x
                    && key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && key_event.code == KeyCode::Char('c') =>
            {
                break;
            }

            // // all normal characters
            // Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
            //     if let KeyCode::Char(c) = key_event.code {
            //         if c.is_alphabetic() {}
            //     }
            // }

            //left
            Event::Key(key_event)
                if key_event.kind == KeyEventKind::Press && key_event.code == KeyCode::Left =>
            {
                ui.left(&mut state)?;
                ui.draw_screen(&state)?;
            }

            //right
            Event::Key(key_event)
                if key_event.kind == KeyEventKind::Press && key_event.code == KeyCode::Right =>
            {
                ui.right(&mut state)?;
                ui.draw_screen(&state)?;
            }

            // up
            Event::Key(key_event)
                if key_event.kind == KeyEventKind::Press && key_event.code == KeyCode::Up =>
            {
                ui.up(&mut state)?;
                ui.draw_screen(&state)?;
            }

            // down
            Event::Key(key_event)
                if key_event.kind == KeyEventKind::Press && key_event.code == KeyCode::Down =>
            {
                ui.down(&mut state)?;
                ui.draw_screen(&state)?;
            }

            Event::Key(_) => {
                saw_ctrl_x = false;
            }

            _ => {}
        }
    }

    ui.clean_up()?;
    Ok(())
}
