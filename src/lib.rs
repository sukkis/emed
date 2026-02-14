use ropey::{Rope, RopeSlice};

pub type ScreenSize = (u16, u16);
pub struct EditorState {
    text: Rope,        // contains all text from all the rows
    cx: usize,         // cursor column in characters (within the line)
    cy: usize,         // cursor line index
    row_offset: usize, // needed for scrolling
    screen_size: ScreenSize,
    pub filename: String,
    pub file_type: FileType,
    pub help_message: String,
}

/// High-level actions the editor understands.
///
/// Intent:
/// - Provide a small, stable “vocabulary” of editor operations (move, insert, delete, quit).
/// - Keep the core editor logic (state mutations + keybinding decisions) independent of any
///   particular terminal/input backend.
///
/// How it fits together:
/// - The binary crate (src/main.rs) reads input (currently via `crossterm`) and translates it
///   into an `EditorCommand`.
/// - The core library can also translate simplified input (`InputKey`) into an `EditorCommand`
///   via `EditorState::command_from_key(...)`. This path is deliberately easy to unit-test.
/// - Executing a command typically means: mutate `EditorState`, then ask the UI to redraw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorCommand {
    Quit,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    InsertChar(char),
    InsertNewline,
    DeleteChar,
    Backspace,
    NoOp,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKey { Char(char), Enter, Backspace, Delete, Left, Right, Up, Down, Ctrl(char) }

pub enum FileType {
    Unknown,
    Text,
    Binary,
    C,
}

impl FileType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileType::Unknown => "unknown",
            FileType::Text => "text",
            FileType::Binary => "binary",
            FileType::C => "c",
        }
    }
}

impl EditorState {
    pub fn new(screen_size: ScreenSize) -> Self {
        Self {
            text: Rope::new(),
            cx: 0,
            cy: 0,
            row_offset: 0,
            screen_size,
            filename: "-".to_string(),
            file_type: FileType::Unknown,
            help_message: "HELP: C-x C-c OR Ctrl-Q to quit".to_string(),
        }
    }



    // scrolling

    /// Adjust `row_offset` so that the cursor line (`cy`) is visible.
    ///
    /// This is what makes "press Enter on the last visible row" scroll the view:
    /// after `cy` changes, we shift the viewport until `cy` fits in the text area.
    pub fn ensure_cursor_visible(&mut self) {
        let height = self.text_area_height();
        if height == 0 {
            self.row_offset = self.cy;
            return;
        }

        if self.cy < self.row_offset {
            self.row_offset = self.cy;
        } else if self.cy >= self.row_offset + height {
            self.row_offset = self.cy + 1 - height;
        }
    }

    /// Height of the editable text area (terminal rows minus status + help).
    pub fn text_area_height(&self) -> usize {
        let (_cols, rows) = self.screen_size;
        (rows as usize).saturating_sub(2)
    }

    /// The first buffer line currently visible at the top of the screen.
    pub fn row_offset(&self) -> usize {
        self.row_offset
    }

    // character operations

    pub fn insert_char(&mut self, c: char) {
        // ropey has all text in one string,
        // so we need to find the start of the current line
        let ropey_line_start = self.text.line_to_char(self.cy);
        let index = ropey_line_start + self.cx;
        self.text.insert_char(index, c);
        self.cx += 1;

        self.ensure_cursor_visible();
    }

    /// Deletes the character *at* the cursor position (not before it).
    ///
    /// Important detail:
    /// - If the cursor is at the end of a line (where the underlying rope has a '\n'),
    ///   deleting that '\n' merges the next line into the current line.
    pub fn delete_char(&mut self) {
        // Can't delete past end-of-buffer.
        let ropey_line_start = self.text.line_to_char(self.cy);
        let index = ropey_line_start + self.cx;

        if index >= self.text.len_chars() {
            return;
        }

        // If we're at the visual end-of-line, there are two cases:
        // - there is a '\n' at idx => deleting it joins lines (great)
        // - there isn't (last line) => nothing to delete
        if self.cx == self.current_line_len() {
            // If we're on the last line, there's typically no '\n' to delete.
            if self.cy >= self.index_of_last_line() {
                return;
            }
        }

        self.text.remove(index..index + 1);
        // self.ensure_cursor_visible();  // later when we have scrolling...
    }

    /// Backspace behavior:
    /// - If we're not at column 0, delete the character *before* the cursor.
    /// - If we're at column 0 and not on the first line, merge this line into the previous one
    ///   by deleting the newline at the end of the previous line.
    pub fn backspace(&mut self) {
        if self.cx > 0 {
            self.cx -= 1;
            self.delete_char(); // deletes the char we just moved onto
        } else if self.cy > 0 {
            self.cy -= 1;
            self.cx = self.current_line_len(); // end of previous line (before '\n')
            self.delete_char(); // deletes the '\n' at end of previous line => merges lines
        }

        // self.ensure_cursor_visible(); // later, with scrolling
    }

    pub fn insert_newline(&mut self) {
        let ropey_line_start = self.text.line_to_char(self.cy);
        let index = ropey_line_start + self.cx;
        self.text.insert_char(index, '\n');
        self.cy += 1;
        self.cx = 0;

        self.ensure_cursor_visible();
    }

    pub fn set_screen_size(&mut self, screen_size: ScreenSize) {
        self.screen_size = screen_size;
        self.ensure_cursor_visible();
    }
    pub fn screen_size(&self) -> ScreenSize {
        self.screen_size
    }
    pub fn set_cursor(&mut self, cx: usize, cy: usize) {
        self.cx = cx;
        self.cy = cy;
    }

    pub fn cursor_pos(&self) -> (usize, usize) {
        (self.cx, self.cy)
    }
    pub fn cursor_left(&mut self) {
        if self.cx > 0 {
            self.cx -= 1;
        } else if self.cy > 0 {
            self.cy -= 1;
            self.cx = self.current_line_len();
        }

        self.ensure_cursor_visible();
    }
    pub fn cursor_right(&mut self) {
        let len = self.current_line_len();

        if self.cx < len {
            self.cx += 1;
        } else if self.cy < self.index_of_last_line() {
            self.cy += 1;
            self.cx = 0;
        }
        self.ensure_cursor_visible();
    }

    pub fn cursor_up(&mut self) {
        if self.cy > 0 {
            self.cy -= 1;
            self.cx = self.cx.min(self.current_line_len());
        }
        self.ensure_cursor_visible();
    }
    pub fn cursor_down(&mut self) {
        if self.cy < self.index_of_last_line() {
            self.cy += 1;
            self.cx = self.cx.min(self.current_line_len());
        }
        self.ensure_cursor_visible();
    }
    pub fn current_line(&self) -> RopeSlice<'_> {
        self.text.line(self.cy)
    }
    pub fn current_line_len(&self) -> usize {
        let line = self.current_line();
        let mut len = line.len_chars();

        if len > 0 && line.char(len - 1) == '\n' {
            len -= 1;
        }

        len
    }

    /// Total number of Unicode scalar values (`char`s) in the buffer.
    ///
    /// Note: this is *not* the same as bytes, and not the same as grapheme clusters.
    /// It's consistent with how the editor currently measures cursor movement and line lengths.
    pub fn char_count(&self) -> usize {
        self.text.len_chars()
    }

    pub fn line_as_string(&self, line_index: usize) -> String {
        self.text.line(line_index).to_string()
    }

    pub fn index_of_last_line(&self) -> usize {
        self.text.len_lines() - 1
    }
}

#[cfg(test)]
impl EditorState {
    /// Test helper: replace the entire buffer with `s` and reset cursor/scroll.
    ///
    /// This keeps tests small and readable without exposing `text` publicly.
    fn set_buffer_for_test(&mut self, s: &str) {
        self.text = Rope::from_str(s);
        self.cx = 0;
        self.cy = 0;
        self.row_offset = 0;
        self.ensure_cursor_visible();
    }

    /// Test helper: return the whole buffer as a `String` (for easy assertions).
    fn buffer_as_string_for_test(&self) -> String {
        self.text.to_string()
    }
}




/// Translate a simplified input key into an editor command.
///
/// This function is deliberately pure (except for the `saw_ctrl_x` flag),
/// so we can unit-test keybindings like Ctrl+X then Ctrl+C without involving crossterm.
pub fn command_from_key(key: InputKey, saw_ctrl_x: &mut bool) -> EditorCommand {
    // Quit on Ctrl-Q. Alternative to C-x C-c.
    if key == InputKey::Ctrl('q') {
        *saw_ctrl_x = false;
        return EditorCommand::Quit;
    }

    // Ctrl-X prefix handling.
    if key == InputKey::Ctrl('x') {
        *saw_ctrl_x = true;
        return EditorCommand::NoOp;
    }

    if *saw_ctrl_x {
        *saw_ctrl_x = false;
        return match key {
            InputKey::Ctrl('c') => EditorCommand::Quit,
            _ => EditorCommand::NoOp,
        };
    }

    match key {
        InputKey::Left => EditorCommand::MoveLeft,
        InputKey::Right => EditorCommand::MoveRight,
        InputKey::Up => EditorCommand::MoveUp,
        InputKey::Down => EditorCommand::MoveDown,
        InputKey::Enter => EditorCommand::InsertNewline,
        InputKey::Delete => EditorCommand::DeleteChar,
        InputKey::Backspace => EditorCommand::Backspace,
        InputKey::Char(c) => EditorCommand::InsertChar(c),
        InputKey::Ctrl(_) => EditorCommand::NoOp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn last_line_index_is_lines_minus_one_with_four_lines() {
        // initialize state with one line
        let mut state = EditorState::new((80, 1));

        // 4 lines of actual text (note: no trailing '\n', so it's exactly 4 lines)
        let rope = Rope::from_str("one\ntwo\nthree\nfour");

        state.text = rope;
        let number_of_lines = state.text.len_lines();
        let last_index = state.index_of_last_line();

        assert_eq!(number_of_lines, 4);
        assert_eq!(last_index, 3);
        assert_eq!(number_of_lines, last_index + 1);
    }

    // Small but “feature rich” test text:
    // - multiple lines
    // - last line without trailing '\n' (common case)
    const SAMPLE: &str = "ab\ncde\nXYZ";

    #[test]
    fn insert_char_inserts_at_cursor_and_advances_cx() {
        let mut state = EditorState::new((80, 24));
        state.set_buffer_for_test("ab\n");

        state.set_cursor(1, 0); // a|b
        state.insert_char('X');

        assert_eq!(state.buffer_as_string_for_test(), "aXb\n");
        assert_eq!(state.cursor_pos(), (2, 0));
    }

    #[test]
    fn insert_newline_splits_line_and_moves_cursor_to_next_line_start() {
        let mut state = EditorState::new((80, 24));
        state.set_buffer_for_test("hello");

        state.set_cursor(2, 0); // he|llo
        state.insert_newline();

        assert_eq!(state.buffer_as_string_for_test(), "he\nllo");
        assert_eq!(state.cursor_pos(), (0, 1));
    }

    #[test]
    fn delete_char_deletes_at_cursor_and_is_noop_at_end_of_buffer() {
        let mut state = EditorState::new((80, 24));
        state.set_buffer_for_test(SAMPLE);

        // Delete in the middle of a line: c|de -> ce
        state.set_cursor(1, 1);
        state.delete_char();
        assert_eq!(state.buffer_as_string_for_test(), "ab\nce\nXYZ");

        // Delete at end-of-buffer should be a no-op (and must not panic).
        state.set_cursor(3, 2); // XYZ| (end of buffer)
        state.delete_char();
        assert_eq!(state.buffer_as_string_for_test(), "ab\nce\nXYZ");
    }

    #[test]
    fn delete_char_at_end_of_line_joins_lines_when_not_last_line() {
        let mut state = EditorState::new((80, 24));
        state.set_buffer_for_test("ab\ncd\n");

        state.set_cursor(2, 0); // ab|<newline>
        state.delete_char(); // deletes '\n' => joins "cd" onto "ab"

        assert_eq!(state.buffer_as_string_for_test(), "abcd\n");
        assert_eq!(state.cursor_pos(), (2, 0)); // cursor stays at join point
    }

    #[test]
    fn backspace_in_middle_deletes_previous_char_and_moves_left() {
        let mut state = EditorState::new((80, 24));
        state.set_buffer_for_test("ab\n");

        state.set_cursor(2, 0); // ab|
        state.backspace(); // deletes 'b'

        assert_eq!(state.buffer_as_string_for_test(), "a\n");
        assert_eq!(state.cursor_pos(), (1, 0));
    }

    #[test]
    fn backspace_at_start_of_line_merges_with_previous_line() {
        let mut state = EditorState::new((80, 24));
        state.set_buffer_for_test("ab\ncd\n");

        state.set_cursor(0, 1); // |cd
        state.backspace(); // merges lines by deleting the newline after "ab"

        assert_eq!(state.buffer_as_string_for_test(), "abcd\n");
        assert_eq!(state.cursor_pos(), (2, 0)); // end of previous line
    }

    #[test]
    fn cursor_up_and_down_clamp_cx_to_line_length() {
        let mut state = EditorState::new((80, 24));
        state.set_buffer_for_test("longline\nshrt\nlongline\n");

        state.set_cursor(7, 0); // longlin|e (cx=7)
        state.cursor_down(); // onto "shrt" (len 4), cx should clamp to 4

        assert_eq!(state.cursor_pos(), (4, 1));

        state.cursor_down(); // back onto "longline", cx should remain 4
        assert_eq!(state.cursor_pos(), (4, 2));
    }

    #[test]
    fn ensure_cursor_visible_scrolls_down_when_cursor_moves_below_viewport() {
        // screen_size rows=4 => text area height = 2 (rows - 2)
        let mut state = EditorState::new((80, 4));
        state.set_buffer_for_test("0\n1\n2\n3\n4\n");

        state.set_cursor(0, 0);
        state.ensure_cursor_visible();
        assert_eq!(state.row_offset(), 0);

        state.set_cursor(0, 2); // cy=2 should not fit into viewport [0..2)
        state.ensure_cursor_visible();
        assert_eq!(state.row_offset(), 1); // cy + 1 - height = 2 + 1 - 2 = 1

        state.set_cursor(0, 4);
        state.ensure_cursor_visible();
        assert_eq!(state.row_offset(), 3); // 4 + 1 - 2 = 3
    }

    #[test]
    fn ensure_cursor_visible_scrolls_up_when_cursor_moves_above_viewport() {
        let mut state = EditorState::new((80, 4)); // text height = 2
        state.set_buffer_for_test("0\n1\n2\n3\n4\n");

        // Pretend we've scrolled down
        state.set_cursor(0, 4);
        state.ensure_cursor_visible();
        assert_eq!(state.row_offset(), 3);

        // Now move cursor back above the viewport; offset should follow up.
        state.set_cursor(0, 1);
        state.ensure_cursor_visible();
        assert_eq!(state.row_offset(), 1);
    }
}
