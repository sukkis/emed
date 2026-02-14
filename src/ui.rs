use crossterm::{cursor, queue, style::ResetColor, terminal};
use std::io;
use std::io::{Stdout, Write};
pub struct EditorUi {
    stdout: Stdout,
}
impl EditorUi {
    pub fn new(stdout: Stdout) -> Self {
        Self { stdout }
    }

    pub fn clean_up(stdout: &mut io::Stdout) -> io::Result<()> {
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
}
