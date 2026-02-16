use crate::VERSION;
use crossterm::style::{
    Attribute, Color, Print, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::{cursor, queue, style::ResetColor, terminal};
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

    pub fn print_editor_version(&mut self, cols: u16, rows: u16) -> io::Result<()> {
        let title = format!("EMED editor version {}", VERSION);
        let chars = title.chars().count();
        let _ = queue!(
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
        queue!(
            self.stdout,
            // black on pink theme
            SetBackgroundColor(Color::Black),
            SetForegroundColor(Color::Magenta),
            // clear and move cursor to right place
            cursor::MoveTo(0, 0),
            terminal::Clear(terminal::ClearType::CurrentLine),
            cursor::Show
        )?;
        Ok(())
    }

    // two last rows are for status information and help.
    // the lowest one is help, status is shown above it
    pub fn queue_status_information(
        &mut self,
        state: &EditorState,
        cols: u16,
        rows: u16,
    ) -> io::Result<()> {
        if rows < 2 {
            return Ok(()); // two small screen to show status
        }
        let status_y = rows - 2;
        let help_y = rows - 1;

        let filetype_str = state.file_type.as_str();
        let cx = state.cursor_pos().0;
        let cy = state.cursor_pos().1;

        // formulate status message from blocks (left, right)
        let mut left_part = format!(
            "{}: {} lines, {} chars",
            filetype_str,
            state.index_of_last_line() + 1,
            state.char_count()
        );
        if state.is_dirty() {
            left_part.push_str(" (modified) ");
        }

        if state.quit_count > 0 {
            left_part.push_str(&format!(" ({} more quit(s) to discard)", state.quit_count));
        }

        let right_part = format!("(col: {}, row: {})", cx, cy);
        let status_message = format!("{}    {}", left_part, right_part);

        // When in prompt mode, show the prompt on the help line;
        // otherwise show the normal help message.
        let help_line = if let Some(ref input) = state.prompt_buffer {
            format!("Save as: {}", input)
        } else {
            state.help_message.clone()
        };

        queue!(
            self.stdout,
            cursor::MoveTo(0, status_y),
            terminal::Clear(terminal::ClearType::CurrentLine),
            SetAttribute(Attribute::Reverse),
            SetAttribute(Attribute::Bold),
            Print(fit_to_width(&status_message, cols as usize)),
            SetAttribute(Attribute::Reset),
            cursor::MoveTo(0, help_y),
            terminal::Clear(terminal::ClearType::CurrentLine),
            Print(fit_to_width(&help_line, cols as usize)),
        )?;

        // Re-assert base theme so the rest of the editor stays "pink on black".
        queue!(
            self.stdout,
            SetBackgroundColor(Color::Black),
            SetForegroundColor(Color::Magenta),
        )?;

        Ok(())
    }

    pub fn draw_screen(&mut self, state: &EditorState) -> io::Result<()> {
        // Draw a complete "frame" of the editor.
        //
        // Rendering model:
        // - Full redraw (simple + robust): we clear and repaint the entire terminal every time.
        // - The bottom 2 rows are reserved for UI chrome:
        //     * second-to-last row: status bar (reverse video)
        //     * last row: help / message line
        //   Everything above those rows is the text viewport.
        //
        // Scrolling model:
        // - `EditorState` keeps the cursor position in *buffer coordinates* (cx, cy), where `cy`
        //   is the absolute line index in the rope.
        // - `EditorState` also stores `row_offset`, which is the buffer line shown at screen row 0.
        //   When the cursor would move off-screen, the state bumps `row_offset` to keep it visible.
        // - During drawing we map:
        //     buffer line index = row_offset + screen_y
        //   so that the viewport "slides" over the buffer.
        //
        // Cursor mapping:
        // - Terminal cursor uses *screen coordinates* (x, y).
        // - The buffer cursor uses *buffer coordinates* (cx, cy).
        // - To place the terminal cursor correctly in the viewport we subtract the scroll offset:
        //     screen_cy = cy - row_offset
        //   (using `saturating_sub` to avoid underflow if something goes out of sync).
        let (cols, rows) = terminal::size()?;
        // let number_of_lines = state.index_of_last_line() + 1;
        let max_rows = rows as usize;
        let text_rows = max_rows.saturating_sub(2);
        let row_offset = state.row_offset();
        let col_offset = state.col_offset();
        let width = cols as usize;

        queue!(self.stdout, cursor::Hide,)?;

        for screen_y in 0..text_rows {
            let line_index = row_offset + screen_y;

            queue!(self.stdout, cursor::MoveTo(0, screen_y as u16))?;

            // First, erase everything on this line.
            queue!(
                self.stdout,
                terminal::Clear(terminal::ClearType::CurrentLine)
            )?;

            if line_index <= state.index_of_last_line() {
                //let mut line = state.line_as_string(line_index);
                let visible = state.get_slice(line_index, width);

                queue!(
                    self.stdout,
                    Print(visible),
                    terminal::Clear(terminal::ClearType::UntilNewLine)
                )?;
            } else {
                queue!(
                    self.stdout,
                    Print("~"),
                    terminal::Clear(terminal::ClearType::UntilNewLine)
                )?;
            }
        }

        self.queue_status_information(state, cols, rows)?;

        // Cursor is in buffer coordinates; convert to screen coordinates using the viewport offset.
        let (cx, cy) = state.cursor_pos();
        let screen_cy = cy.saturating_sub(row_offset);
        let screen_xy = cx.saturating_sub(col_offset);
        queue!(
            self.stdout,
            cursor::MoveTo(to_u16(screen_xy), to_u16(screen_cy)),
            cursor::Show
        )?;

        // single flush
        self.stdout.flush()?;

        Ok(())
    }

    //
    // cursor movement functions
    //

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

pub fn fit_to_width(s: &str, width: usize) -> String {
    let mut out: String = s.chars().take(width).collect();
    let len = out.chars().count();
    if len < width {
        out.extend(std::iter::repeat_n(' ', width - len));
    }
    out
}
