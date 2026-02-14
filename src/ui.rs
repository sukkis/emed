use crate::{VERSION};
use crossterm::style::{
    Attribute, Color, Print, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::{cursor, execute, queue, style::ResetColor, terminal};
use emed_core::EditorState;
use std::io;
use std::io::{Stdout, Write};

pub struct EditorUi {
    stdout: Stdout,
}
impl EditorUi {
    pub fn new(stdout: Stdout) -> Self {
        Self { stdout }
    }

    fn _clear_screen(&mut self) -> io::Result<()> {
        execute!(
            self.stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0)
        )?;
        Ok(())
    }

    pub fn clean_up(&mut self) -> io::Result<()> {
        terminal::disable_raw_mode()?;
        queue!(
            self.stdout,
            ResetColor,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0),
            cursor::Show
        )?;
        self.stdout.flush()?;
        Ok(())
    }

    pub fn print_editor_version(&mut self) -> io::Result<()> {
        let (cols, rows) = terminal::size()?;
        let title = format!("EMED editor version {}", VERSION);
        let chars = title.chars().count();
        let _ = execute!(
            self.stdout,
            SetBackgroundColor(Color::Black),
            SetForegroundColor(Color::Magenta),
            SetAttribute(Attribute::Bold),
            cursor::MoveTo((cols / 2) - chars as u16 / 2, rows / 2 - 2),
            Print(&title),
            cursor::Hide
        );
        Ok(())
    }
    pub fn initialise_screen(&mut self) -> io::Result<()> {
        let (_, rows) = terminal::size()?;
        let x = 0;

        for y in 0..rows {
            let _ = execute!(self.stdout, cursor::MoveTo(x, y), Print("~\n"));
        }
        Ok(())
    }

    pub fn initialise_editing(&mut self) -> io::Result<()> {
        self.set_colour_scheme()?;
        let _ = execute!(
            self.stdout,
            cursor::MoveTo(0, 0),
            terminal::Clear(terminal::ClearType::CurrentLine),
            cursor::Show
        );
        Ok(())
    }

    pub fn set_colour_scheme(&mut self) -> io::Result<()> {
        let _ = execute!(
            self.stdout,
            SetBackgroundColor(Color::Black),
            SetForegroundColor(Color::Magenta)
        );
        Ok(())
    }

    pub fn draw_screen(&mut self, state: &EditorState) -> io::Result<()> {
        let (_cols, rows) = terminal::size()?;
        let number_of_lines = state.index_of_last_line() + 1;

        execute!(
            self.stdout,
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
            execute!(self.stdout, cursor::MoveTo(0, y as u16), Print(line))?;
        }

        for y in visible..max_rows {
            execute!(self.stdout, cursor::MoveTo(0, y as u16), Print("~"))?;
        }

        self.move_cursor_to(state)?;
        Ok(())
    }
    //
    // cursor movement functions
    //
    pub fn move_cursor_to(&mut self, state: &EditorState) -> io::Result<()> {
        let (cx, cy) = state.cursor_pos(); // (usize, usize)
        execute!(self.stdout, cursor::MoveTo(to_u16(cx), to_u16(cy)))?;
        Ok(())
    }

    pub fn left(&mut self, state: &mut EditorState) -> io::Result<()> {
        state.cursor_left();
        self.draw_screen(state)
    }

    pub fn right(&mut self, state: &mut EditorState) -> io::Result<()> {
        state.cursor_right();
        self.draw_screen(state)
    }

    pub fn up(&mut self, state: &mut EditorState) -> io::Result<()> {
        state.cursor_up();
        self.draw_screen(state)
    }

    pub fn down(&mut self, state: &mut EditorState) -> io::Result<()> {
        state.cursor_down();
        self.draw_screen(state)
    }
}

fn to_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}
