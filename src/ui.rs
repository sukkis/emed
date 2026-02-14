use crate::VERSION;
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

    pub fn clear_screen(&mut self) -> io::Result<()> {
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

    pub fn initialise_editing(&mut self) -> io::Result<()> {
        self.set_colour_scheme()?;
        execute!(
            self.stdout,
            cursor::MoveTo(0, 0),
            terminal::Clear(terminal::ClearType::CurrentLine),
            cursor::Show
        )?;
        Ok(())
    }

    pub fn set_colour_scheme(&mut self) -> io::Result<()> {
        execute!(
            self.stdout,
            SetBackgroundColor(Color::Black),
            SetForegroundColor(Color::Magenta)
        )?;
        Ok(())
    }
    // two last rows are for status information and help.
    // the lowest one is help, status is shown above it
    pub fn update_status_information(&mut self, state: &EditorState) -> io::Result<()> {
        let (cols, rows) = terminal::size()?;
        let filetype_str = state.file_type.as_str();
        if rows < 2 {
            return Ok(()); // two small screen to show status
        }
        let status_y = rows - 2;
        let help_y = rows - 1;

        let filetype_str = state.file_type.as_str();

        let status_message = format!(
            "{}: {} lines, {} chars",
            filetype_str,
            state.index_of_last_line() + 1,
            state.char_count()
        );

        execute!(
            self.stdout,
            cursor::MoveTo(0, status_y),
            terminal::Clear(terminal::ClearType::CurrentLine),
            SetAttribute(Attribute::Reverse),
            SetAttribute(Attribute::Bold),
            Print(fit_to_width(&status_message, cols as usize)),
            SetAttribute(Attribute::Reset),
            cursor::MoveTo(0, help_y),
            terminal::Clear(terminal::ClearType::CurrentLine),
            Print(fit_to_width(&state.help_message, cols as usize)),
        )?;

        // Re-assert base theme so the rest of the editor stays "pink on black".
        self.set_colour_scheme()?;

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

        for y in visible..max_rows - 2 {
            execute!(self.stdout, cursor::MoveTo(0, y as u16), Print("~"))?;
        }

        self.update_status_information(state)?;

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

// helper functions

fn to_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

fn fit_to_width(s: &str, width: usize) -> String {
    let mut out: String = s.chars().take(width).collect();
    let len = out.chars().count();
    if len < width {
        out.extend(std::iter::repeat_n(' ', width - len));
    }
    out
}
