use crate::Theme::{self};
use crate::VERSION;
use crossterm::style::{Attribute, Print, SetAttribute, SetBackgroundColor, SetForegroundColor};
use crossterm::{cursor, queue, style::ResetColor, terminal};
use emed_core::EditorState;
use emed_core::lexer::TokenKind;
use std::io;
use std::io::{Stdout, Write};

pub struct EditorUi {
    stdout: Stdout,
    theme: Theme,
}
impl EditorUi {
    pub fn new(stdout: Stdout, theme: Theme) -> Self {
        Self { stdout, theme }
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
            SetBackgroundColor(self.theme.bg.to_crossterm()),
            SetForegroundColor(self.theme.fg.to_crossterm()),
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
            SetBackgroundColor(self.theme.bg.to_crossterm()),
            SetForegroundColor(self.theme.fg.to_crossterm()),
            // clear and move cursor to right place
            cursor::MoveTo(0, 0),
            terminal::Clear(terminal::ClearType::CurrentLine),
            cursor::Show
        )?;
        Ok(())
    }

    /// Queue the status bar and help/message line into the terminal buffer.
    ///
    /// Renders two rows at the bottom of the screen:
    /// - **Status bar** — file type, line/char counts, `(modified)` flag,
    ///   cursor position. Displayed in reverse-video (status theme colours).
    /// - **Help line** — either the default keybinding hints, a transient
    ///   message (e.g. "File saved"), or the prompt input when in prompt mode.
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
            SetBackgroundColor(self.theme.status_bg.to_crossterm()),
            SetForegroundColor(self.theme.status_fg.to_crossterm()),
            SetAttribute(Attribute::Bold),
            Print(fit_to_width(&status_message, cols as usize)),
            SetAttribute(Attribute::Reset),
            cursor::MoveTo(0, help_y),
            terminal::Clear(terminal::ClearType::CurrentLine),
            SetBackgroundColor(self.theme.bg.to_crossterm()),
            SetForegroundColor(self.theme.fg.to_crossterm()),
            Print(fit_to_width(&help_line, cols as usize)),
        )?;

        // Re-assert base theme so the rest of the editor stays "pink on black".
        queue!(
            self.stdout,
            SetBackgroundColor(self.theme.bg.to_crossterm()),
            SetForegroundColor(self.theme.fg.to_crossterm()),
        )?;

        Ok(())
    }

    /// Render a complete frame of the editor to the terminal.
    ///
    /// Performs a full redraw every time: clears each line and repaints it.
    /// The screen is divided into three regions:
    ///
    /// - **Text area** (top) — visible portion of the buffer, with syntax
    ///   highlighting applied via the token cache in [`EditorState`]. Lines
    ///   beyond the end of the buffer show a `~` in the tilde colour.
    /// - **Status bar** (second-to-last row) — file type, line count, dirty
    ///   flag, and cursor coordinates.
    /// - **Help / message line** (last row) — keybinding hints, or the
    ///   prompt input when in prompt mode.
    ///
    /// The viewport scrolls so that the cursor (in buffer coordinates) is
    /// always visible: `row_offset` / `col_offset` from [`EditorState`]
    /// control which slice of the buffer is shown.
    pub fn draw_screen(&mut self, state: &mut EditorState) -> io::Result<()> {
        let (cols, rows) = terminal::size()?;
        let max_rows = rows as usize;
        let text_rows = max_rows.saturating_sub(2);
        let row_offset = state.row_offset();
        let col_offset = state.col_offset();
        let width = cols as usize;

        queue!(self.stdout, cursor::Hide)?;

        for screen_y in 0..text_rows {
            let line_index = row_offset + screen_y;

            queue!(self.stdout, cursor::MoveTo(0, screen_y as u16))?;

            queue!(
                self.stdout,
                terminal::Clear(terminal::ClearType::CurrentLine)
            )?;

            if line_index <= state.index_of_last_line() {
                let visible = state.get_slice(line_index, width);

                let tokens = state.tokens_for_line(line_index).to_vec();
                if tokens.is_empty() {
                    queue!(self.stdout, Print(&visible))?;
                } else {
                    for (char_idx, ch) in visible.chars().enumerate() {
                        let buf_col = col_offset + char_idx;

                        let kind = tokens
                            .iter()
                            .find(|t| buf_col >= t.start && buf_col < t.start + t.len)
                            .map(|t| t.kind)
                            .unwrap_or(TokenKind::Normal);

                        match kind {
                            TokenKind::Number => {
                                queue!(
                                    self.stdout,
                                    SetForegroundColor(self.theme.number_fg.to_crossterm()),
                                    Print(ch),
                                )?;
                            }
                            _ => {
                                queue!(
                                    self.stdout,
                                    SetForegroundColor(self.theme.fg.to_crossterm()),
                                    Print(ch),
                                )?;
                            }
                        }
                    }
                    queue!(
                        self.stdout,
                        SetForegroundColor(self.theme.fg.to_crossterm()),
                    )?;
                }

                queue!(
                    self.stdout,
                    terminal::Clear(terminal::ClearType::UntilNewLine)
                )?;
            } else {
                queue!(
                    self.stdout,
                    SetForegroundColor(self.theme.tilde_fg.to_crossterm()),
                    Print("~"),
                    SetForegroundColor(self.theme.fg.to_crossterm()),
                    terminal::Clear(terminal::ClearType::UntilNewLine)
                )?;
            }
        }

        self.queue_status_information(state, cols, rows)?;

        let (cx, cy) = state.cursor_pos();
        let screen_cy = cy.saturating_sub(row_offset);
        let screen_col = state.cx_to_screen_col(cy, cx);
        let screen_cx = screen_col.saturating_sub(col_offset);
        queue!(
            self.stdout,
            cursor::MoveTo(to_u16(screen_cx), to_u16(screen_cy)),
            cursor::Show
        )?;

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
