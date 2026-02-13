use crossterm::event::{KeyEventKind, KeyModifiers};
use crossterm::{
    cursor,
    event::{Event, KeyCode, read},
    execute, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal,
};
use std::io::{self, Write};

const VERSION: &str = "0.0.1";

fn print_editor_version(stdout: &mut io::Stdout) {
    let (cols, rows) = terminal::size().unwrap();
    let title = format!("EMED editor version {}", VERSION);
    let _ = execute!(
        stdout,
        cursor::MoveTo((cols / 2) - 20, (rows / 2) - 5),
        Print(&title)
    );
}

fn set_colour_scheme(stdout: &mut io::Stdout) {
    let _ = execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        SetBackgroundColor(Color::Red),
        SetForegroundColor(Color::Blue)
    );
}

fn main() -> io::Result<()> {
    //let version = 0.01.to_string();

    let mut stdout = io::stdout();
    let _ = terminal::enable_raw_mode();

    set_colour_scheme(&mut stdout);
    print_editor_version(&mut stdout);

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
