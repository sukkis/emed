use crossterm::event::{KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, SetAttribute};
use crossterm::{
    cursor,
    event::{Event, KeyCode, read},
    execute,
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
    terminal,
};
use std::io::{self};

use emed_core::EditorState;
mod ui;
use ui::EditorUi;
const VERSION: &str = "0.0.1";

/* helper functions */
fn to_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

/*
cursor movements
    */

fn move_cursor_to(stdout: &mut io::Stdout, state: &EditorState) -> io::Result<()> {
    let (cx, cy) = state.cursor_pos(); // (usize, usize)
    execute!(stdout, cursor::MoveTo(to_u16(cx), to_u16(cy)))?;
    Ok(())
}
fn left(state: &mut EditorState, stdout: &mut io::Stdout) -> io::Result<()> {
    state.cursor_left();
    move_cursor_to(stdout, state)
}

fn right(state: &mut EditorState, stdout: &mut io::Stdout) -> io::Result<()> {
    state.cursor_right();
    move_cursor_to(stdout, state)
}

fn up(state: &mut EditorState, stdout: &mut io::Stdout) -> io::Result<()> {
    state.cursor_up();
    move_cursor_to(stdout, state)
}

fn down(state: &mut EditorState, stdout: &mut io::Stdout) -> io::Result<()> {
    state.cursor_down();
    move_cursor_to(stdout, state)
}

fn print_editor_version(stdout: &mut io::Stdout) -> io::Result<()> {
    let (cols, rows) = terminal::size()?;
    let title = format!("EMED editor version {}", VERSION);
    let chars = title.chars().count();
    let _ = execute!(
        stdout,
        SetBackgroundColor(Color::Black),
        SetForegroundColor(Color::Magenta),
        SetAttribute(Attribute::Bold),
        cursor::MoveTo((cols / 2) - chars as u16 / 2, rows / 2 - 2),
        Print(&title),
        cursor::Hide
    );
    Ok(())
}

fn initialise_screen(stdout: &mut io::Stdout) -> io::Result<()> {
    let (_, rows) = terminal::size()?;
    let x = 0;

    for y in 0..rows {
        let _ = execute!(stdout, cursor::MoveTo(x, y), Print("~\n"));
    }
    Ok(())
}

fn initialise_editing(stdout: &mut io::Stdout) -> io::Result<()> {
    set_colour_scheme(stdout)?;
    let _ = execute!(
        stdout,
        cursor::MoveTo(0, 0),
        terminal::Clear(terminal::ClearType::CurrentLine),
        cursor::Show
    );
    Ok(())
}

fn set_colour_scheme(stdout: &mut io::Stdout) -> io::Result<()> {
    let _ = execute!(
        stdout,
        SetBackgroundColor(Color::Black),
        SetForegroundColor(Color::Magenta)
    );
    Ok(())
}

fn clear_screen(stdout: &mut io::Stdout) -> io::Result<()> {
    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    Ok(())
}

fn draw_screen(stdout: &mut io::Stdout, state: &EditorState) -> io::Result<()> {
    let (_cols, rows) = terminal::size()?;
    let number_of_lines = state.index_of_last_line() + 1;

    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;

    let max_rows = rows as usize;
    let visible = number_of_lines.min(max_rows);
    for y in 0..visible {
        let mut line = state.line_as_string(y);
        // remove last unicode scalar of string if it is newline
        // otherwise you might have extra blank lines or cursor in wrong place
        if line.ends_with('\n') {
            line.pop();
        }
        execute!(stdout, cursor::MoveTo(0, y as u16), Print(line))?;
    }

    for y in visible..max_rows {
        execute!(stdout, cursor::MoveTo(0, y as u16), Print("~"))?;
    }

    move_cursor_to(stdout, state)?;
    Ok(())
}

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    clear_screen(&mut stdout)?;

    print_editor_version(&mut stdout)?;

    initialise_screen(&mut stdout)?;

    initialise_editing(&mut stdout)?;

    let screen_size = terminal::size()?;
    let mut state = EditorState::new(screen_size);
    draw_screen(&mut stdout, &state)?;

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
                left(&mut state, &mut stdout)?;
                draw_screen(&mut stdout, &state)?;
            }

            //right
            Event::Key(key_event)
                if key_event.kind == KeyEventKind::Press && key_event.code == KeyCode::Right =>
            {
                right(&mut state, &mut stdout)?;
                draw_screen(&mut stdout, &state)?;
            }

            // up
            Event::Key(key_event)
                if key_event.kind == KeyEventKind::Press && key_event.code == KeyCode::Up =>
            {
                up(&mut state, &mut stdout)?;
                draw_screen(&mut stdout, &state)?;
            }

            // down
            Event::Key(key_event)
                if key_event.kind == KeyEventKind::Press && key_event.code == KeyCode::Down =>
            {
                down(&mut state, &mut stdout)?;
                draw_screen(&mut stdout, &state)?;
            }

            Event::Key(_) => {
                saw_ctrl_x = false;
            }

            _ => {}
        }
    }

    let _ = ui::EditorUi::clean_up(&mut stdout);
    Ok(())
}
