use crossterm::event::{KeyEventKind, KeyModifiers};
use crossterm::{
    cursor,
    event::{Event, KeyCode, read},
    execute, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal,
};
use ropey;
use std::io::{self, Write};
use crossterm::style::{Attribute, SetAttribute};

const VERSION: &str = "0.0.1";

fn print_editor_version(stdout: &mut io::Stdout) {
    let (cols, rows) = terminal::size().unwrap();
    let title = format!("EMED editor version {}", VERSION);
    let chars = title.chars().count();
    let _ = execute!(
        stdout,
        SetBackgroundColor(Color::Black),
        SetForegroundColor(Color::Magenta),
        SetAttribute(Attribute::Bold),
        cursor::MoveTo((cols / 2) - chars as u16/2, rows / 2 - 2 ),
        Print(&title),
        cursor::Hide
    );
}

fn initialise_screen(stdout: &mut io::Stdout) {
    let (_, rows) = terminal::size().unwrap();
    let x = 0;

    for y in 0..rows {
        let _ = execute!(stdout, cursor::MoveTo(x, y), Print("~\n"));
    }
}

fn _set_colour_scheme(stdout: &mut io::Stdout) {
    let _ = execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        SetBackgroundColor(Color::Black),
        SetForegroundColor(Color::Magenta)
    );
}

fn clear_screen(stdout: &mut io::Stdout) -> io::Result<()> {
    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    Ok(())
}

fn main() -> io::Result<()> {

    let mut stdout = io::stdout();
    let _ = terminal::enable_raw_mode();
    clear_screen(&mut stdout)?;

    print_editor_version(&mut stdout);
//    set_colour_scheme(&mut stdout);

    initialise_screen(&mut stdout);

    let mut saw_ctrl_x = false;
    loop {
        match read()? {
            Event::Key(key_event)
                if key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && key_event.code == KeyCode::Char('x') =>
            {
                saw_ctrl_x = true
            }

            Event::Key(key_event)
                if saw_ctrl_x
                    && key_event.kind == KeyEventKind::Press
                    && key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && key_event.code == KeyCode::Char('c') =>
            {
                break;
            }

            Event::Key(_) => saw_ctrl_x = false,

            _ => {}
        }
    }

    let _ = clean_up(&mut stdout);
    Ok(())
}

fn clean_up(stdout: &mut io::Stdout) -> io::Result<()> {
    terminal::disable_raw_mode()?;
    queue!(
        stdout,
        ResetColor,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    stdout.flush()?;
    Ok(())
}
