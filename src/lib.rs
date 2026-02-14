use ropey::{Rope, RopeSlice};

pub type ScreenSize = (u16, u16);
pub struct EditorState {
    text: Rope, // contains all text from all the rows
    cx: usize,  // cursor column in characters (within the line)
    cy: usize,  // cursor line index
    screen_size: ScreenSize,
}

impl EditorState {
    pub fn new(screen_size: ScreenSize) -> Self {
        Self {
            text: Rope::from_str("hello\nworld\n"),
            cx: 0,
            cy: 0,
            screen_size,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        // ropey has all text in one string,
        // so we need to find the start of the current line
        let ropey_line_start = self.text.line_to_char(self.cy);
        let index = ropey_line_start + self.cx;
        self.text.insert_char(index, c);
        self.cx += 1;
    }
    pub fn set_screen_size(&mut self, screen_size: ScreenSize) {
        self.screen_size = screen_size;
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
    }
    pub fn cursor_right(&mut self) {
        let len = self.current_line_len();

        if self.cx < len {
            self.cx += 1;
        } else if self.cy < self.index_of_last_line() {
            self.cy += 1;
            self.cx = 0;
        }
    }
    pub fn cursor_up(&mut self) {
        if self.cy > 0 {
            self.cy -= 1;
            self.cx = self.cx.min(self.current_line_len());
        }
    }
    pub fn cursor_down(&mut self) {
        if self.cy < self.index_of_last_line() {
            self.cy += 1;
            self.cx = self.cx.min(self.current_line_len());
        }
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

    pub fn line_as_string(&self, line_index: usize) -> String {
        self.text.line(line_index).to_string()
    }

    pub fn index_of_last_line(&self) -> usize {
        self.text.len_lines() - 1
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
}
