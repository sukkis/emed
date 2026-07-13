pub mod lexer;
pub mod search;
use lexer::{Lexer, Token, lexer_for_file_type};
use ropey::{Rope, RopeSlice};
use search::SearchSession;
use std::path::Path;
use unicode_width::UnicodeWidthChar;

pub type ScreenSize = (u16, u16);

/// Number of consecutive Quit presses required to discard unsaved changes.
pub const QUIT_CONFIRM_COUNT: u8 = 3;

/// Default help message shown in the bottom line of the editor.
pub const DEFAULT_HELP_MESSAGE: &str = "HELP: C-x C-s to Save, C-x C-c to Quit";

/// The state of the editor has no UI implementation details,
/// and does not depend on crossterm.
/// A "rope" (Ropey) is used to store the whole text.
/// Rendered text is calculated in the UI side using slices from full text.
/// Cursor position is the character-based position in buffer,
/// not the visual position seen on the screen.
/// E.g. If an opened file has a tab character, it is only one character in the buffer,
/// but visually it is rendered as multiple characters.
pub struct EditorState {
    text: Rope,        // contains all text from all the rows
    cx: usize,         // cursor column in characters (within the line)
    cy: usize,         // cursor line index
    row_offset: usize, // needed for scrolling
    col_offset: usize, // horizontal scrolling
    screen_size: ScreenSize,
    pub filename: String,
    pub file_type: FileType,
    pub help_message: String,
    /// When `Some`, the editor is in prompt mode (e.g. "Save as").
    /// The `String` accumulates the user's typed input.
    /// `None` means normal editing mode.
    pub prompt_buffer: Option<String>,
    pub dirty: bool,
    /// How many times the user has pressed Quit while the buffer is dirty.
    /// When this reaches QUIT_CONFIRM_COUNT the editor actually exits.
    pub quit_count: u8,
    pub tab_width: usize,
    /// Whether long lines wrap at word boundaries instead of scrolling
    /// horizontally. Mirrors Emacs' `visual-line-mode`. Rendering support
    /// for this is not wired up yet — for now it's just a flag with a
    /// default and a settings-file override.
    pub visual_line_mode: bool,
    /// Syntax lexer chosen based on `file_type`.  `None` = no highlighting.
    lexer: Option<Box<dyn Lexer>>,
    /// Per-line token cache.  `token_cache[i]` holds the tokens for line `i`.
    /// Invalidated on any edit (initially just clear the whole vec;
    /// later we can do smarter incremental invalidation).
    token_cache: Vec<Vec<Token>>,
    /// When `Some`, an incremental search is in progress.
    search: Option<SearchSession>,
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
    SaveFile,
    PromptSaveAs,
    StartSearch,
    ToggleVisualLineMode,
    NoOp,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKey {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Ctrl(char),
}

// for now we use this for interaction with user about file name to save
// later this could be used for find
pub enum EditorMode {
    Normal,
    PromptInput,
}

pub enum FileType {
    Unknown,
    Text,
    Binary,
    C,
    Rust,
}

impl FileType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileType::Unknown => "unknown",
            FileType::Text => "text",
            FileType::Binary => "binary",
            FileType::C => "C file",
            FileType::Rust => "Rust file",
        }
    }
}

/// Result of applying an `EditorCommand` to the editor state.
///
/// This is intentionally UI-agnostic: the binary can decide whether/how to redraw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyResult {
    /// No visible state change (no redraw needed).
    NoChange,
    /// State changed (redraw recommended).
    Changed,
    /// Request to quit the application.
    Quit,
}

impl EditorState {
    pub fn new(screen_size: ScreenSize) -> Self {
        Self {
            text: Rope::new(),
            cx: 0,
            cy: 0,
            row_offset: 0,
            col_offset: 0,
            screen_size,
            filename: "-".to_string(),
            file_type: FileType::Unknown,
            help_message: DEFAULT_HELP_MESSAGE.to_string(),
            prompt_buffer: None,
            dirty: false,
            quit_count: 0,
            tab_width: 4,
            visual_line_mode: false,
            lexer: Some(lexer_for_file_type(&FileType::Unknown)),
            token_cache: vec![Vec::new(); 1], // Rope::new() has 1 line
            search: None,
        }
    }

    /// Add tokens of one line to the cache.
    /// Calling lexer.tokenize_line() does the heavy lifting.
    pub fn tokens_for_line(&mut self, line_index: usize) -> &[Token] {
        // If the cache hasn't been initialized (no file loaded, or index out of range),
        // return an empty slice — no highlighting.
        if line_index >= self.token_cache.len() {
            return &[];
        }

        // only do something if we don't have a cache
        if self.token_cache[line_index].is_empty() {
            // do we have a lexer? (might be a plain text file)
            if let Some(lexer) = &self.lexer {
                // Step 3: Get the line text from the rope, tokenize it.
                let line_str = self.text.line(line_index).to_string();
                let (tokens, _) = lexer.tokenize_line(&line_str, false);

                // Step 4: Store the result so we don't re-tokenize next time.
                self.token_cache[line_index] = tokens;
            }
            // If lexer is None, token_cache stays empty → no highlighting.
        }

        // return a reference to the cached tokens (possibly empty).
        &self.token_cache[line_index]
    }

    /// Any mutation (insert_char, delete_char, backspace, insert_newline)
    /// clears the cache.
    pub fn invalidate_syntax_highlighting(&mut self) {
        self.invalidate_tokens();
    }
    fn invalidate_tokens(&mut self) {
        self.token_cache.clear();
        self.token_cache.resize(self.text.len_lines(), Vec::new());
    }

    /// Convert a char-index on a given line to its screen column.
    pub fn cx_to_screen_col(&self, line_index: usize, cx: usize) -> usize {
        self.text
            .line(line_index)
            .chars()
            .take(cx)
            .map(|c| self.display_width(c))
            .sum()
    }

    // buffer changes or not? if edited, "dirty"
    fn set_dirty(&mut self) {
        self.dirty = true;
        self.invalidate_tokens();
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
        self.quit_count = 0;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn reset_quit_count(&mut self) {
        self.quit_count = 0;
    }

    /// calculate screen width for a single character, using unicode-width
    pub fn display_width(&self, c: char) -> usize {
        match c {
            '\t' => self.tab_width,
            '\n' => 0,
            '\r' => 0,
            _ => c.width().unwrap_or(0),
        }
    }

    // calculate screen width for whole line
    pub fn display_width_of_line(&self, line_index: usize) -> usize {
        self.text
            .line(line_index)
            .chars()
            .map(|c| self.display_width(c))
            .sum()
    }

    /// Collect characters from `chars` that fit within `max_cols` screen columns.
    /// Tabs are expanded to spaces. Returns the rendered string.
    fn render_to_width(&self, chars: impl Iterator<Item = char>, max_cols: usize) -> String {
        let mut out = String::new();
        let mut cols_used = 0;

        for c in chars {
            let w = self.display_width(c);
            if cols_used + w > max_cols {
                break;
            }
            if c == '\t' {
                // Expand tab to spaces
                out.extend(std::iter::repeat_n(' ', w));
            } else {
                out.push(c);
            }
            cols_used += w;
        }

        out
    }

    /// Return the visible portion of a buffer line as a rendered string.
    ///
    /// Applies horizontal scrolling (`col_offset`) and truncates to
    /// `screen_width` columns. Tabs are expanded to spaces; trailing
    /// newlines are stripped. The result is ready to print to the terminal.
    pub fn get_slice(&self, line_index: usize, screen_width: usize) -> String {
        let line = self.text.line(line_index);

        // Skip characters until we've passed col_offset screen columns.
        let mut skip_cols = 0;
        let visible_chars = line.chars().filter(|&c| c != '\n').skip_while(|&c| {
            let w = self.display_width(c);
            if skip_cols + w <= self.col_offset {
                skip_cols += w;
                true
            } else {
                false
            }
        });

        self.render_to_width(visible_chars, screen_width)
    }

    /// Word-wrap a single buffer line into chunks that each fit within
    /// `width` display columns, for `visual_line_mode` rendering.
    ///
    /// Breaks at the nearest space at or before the width limit; the space
    /// stays attached to the end of the earlier chunk, so every chunk
    /// starts at a real character and concatenating all chunks reproduces
    /// the original line exactly. A word with no space that is itself
    /// longer than `width` is hard-broken, since there is no space to back
    /// up to. This never modifies the buffer — it only reads `self.text`.
    pub fn wrapped_lines(&self, line_index: usize, width: usize) -> Vec<String> {
        // Nothing can ever fit in zero columns. Without this, the loop
        // below never advances (every character overflows immediately and
        // there's no space to break at, so `chunk_start` never moves) and
        // spins forever. Bail out the same way an empty line already does.
        if width == 0 {
            return Vec::new();
        }

        // Collected (rather than iterated in place) because the algorithm
        // needs to look backward to the last space once a chunk overflows,
        // which a `Rope` line iterator alone doesn't support.
        let chars: Vec<char> = self
            .text
            .line(line_index)
            .chars()
            .filter(|&c| c != '\n')
            .collect();

        let mut chunks = Vec::new();
        // Index into `chars` where the chunk currently being built begins.
        let mut chunk_start = 0;
        // Display columns used so far by the current (unfinished) chunk.
        let mut cols_used = 0;
        // Index of the most recent space seen since `chunk_start` — the
        // fallback break point if the chunk overflows. `None` means no
        // space has been seen yet, so an overflow must hard-break instead.
        let mut last_space_index: Option<usize> = None;
        let mut i = 0;

        while i < chars.len() {
            let char_width = self.display_width(chars[i]);

            if cols_used + char_width > width {
                // This character would overflow the chunk — decide where
                // to cut *before* it. Break just after the last space if
                // we saw one (word-wrap); otherwise break right here, mid
                // word, since there is nothing better to back up to.
                let break_at = last_space_index.map_or(i, |space| space + 1);
                chunks.push(chars[chunk_start..break_at].iter().collect());

                // Start a fresh chunk at the break point. `i` is rewound
                // to `chunk_start` (rather than left where it was) because
                // when breaking at a space, the characters between the
                // space and the old `i` were scanned for the chunk we just
                // closed but never pushed anywhere — they belong to this
                // new chunk and must be re-counted from `cols_used = 0`.
                chunk_start = break_at;
                i = chunk_start;
                cols_used = 0;
                last_space_index = None;
                continue;
            }

            if chars[i] == ' ' {
                last_space_index = Some(i);
            }
            cols_used += char_width;
            i += 1;
        }

        // The loop only pushes a chunk when it overflows, so the final,
        // still-open chunk (which never overflowed) is pushed here.
        if chunk_start < chars.len() {
            chunks.push(chars[chunk_start..].iter().collect());
        }

        chunks
    }

    /// Compose `wrapped_lines` across the whole buffer, starting at
    /// `row_offset`, into the flat list of screen rows to paint when
    /// `visual_line_mode` is on.
    ///
    /// Returns exactly `height` entries: `Some(chunk)` for a screen row
    /// with real content, `None` once the buffer runs out (so the caller
    /// can print `~`, matching the non-wrapped rendering path). A blank
    /// buffer line still claims exactly one row (an empty chunk), even
    /// though `wrapped_lines` itself returns nothing for it. A line that
    /// wraps into more chunks than there is room left for is clipped —
    /// a known limitation for this increment; scrolling by visual row
    /// instead of buffer line is expected to fix it later.
    pub fn wrapped_screen_rows(&self, height: usize, width: usize) -> Vec<Option<String>> {
        let mut rows = Vec::with_capacity(height);
        let mut line_index = self.row_offset;

        while rows.len() < height && line_index <= self.index_of_last_line() {
            let chunks = self.wrapped_lines(line_index, width);

            if chunks.is_empty() {
                rows.push(Some(String::new()));
            } else {
                for chunk in chunks {
                    if rows.len() == height {
                        break;
                    }
                    rows.push(Some(chunk));
                }
            }

            line_index += 1;
        }

        while rows.len() < height {
            rows.push(None);
        }

        rows
    }

    /// How many wrapped screen rows the buffer lines from `row_offset` up
    /// to (but not including) `line_index` occupy. The row/Y half of
    /// mapping a buffer position to a screen position — reuses the same
    /// "a blank line still counts as one row" rule as
    /// `wrapped_screen_rows`, via `.max(1)` on each line's chunk count.
    pub fn screen_rows_before_line(&self, line_index: usize, width: usize) -> usize {
        (self.row_offset..line_index)
            .map(|i| self.wrapped_lines(i, width).len().max(1))
            .sum()
    }

    /// The within-the-current-line half of mapping a buffer position to a
    /// screen position: given `cx` on `line_index`, which wrapped chunk
    /// (row within the line) does it fall in, and what display column
    /// within that chunk? A `cx` sitting exactly on a mid-line chunk
    /// boundary belongs to the start of the *next* chunk, not the end of
    /// the previous one, matching `wrapped_lines`' "every chunk starts at
    /// a real character" rule.
    pub fn wrapped_cursor_offset(
        &self,
        line_index: usize,
        cx: usize,
        width: usize,
    ) -> (usize, usize) {
        let chunks = self.wrapped_lines(line_index, width);

        if chunks.is_empty() {
            return (0, 0);
        }

        let mut chars_before = 0;
        for (chunk_idx, chunk) in chunks.iter().enumerate() {
            let chunk_char_count = chunk.chars().count();
            let is_last = chunk_idx == chunks.len() - 1;

            // Keep looking for a later chunk unless `cx` genuinely falls
            // within this one, or there's nowhere else left to put it
            // (the last chunk also has to catch `cx` sitting right at the
            // end of the line).
            if cx < chars_before + chunk_char_count || is_last {
                let offset_chars = cx - chars_before;
                let col = chunk
                    .chars()
                    .take(offset_chars)
                    .map(|c| self.display_width(c))
                    .sum();
                return (chunk_idx, col);
            }

            chars_before += chunk_char_count;
        }

        unreachable!("the last chunk always satisfies the condition above")
    }

    // Saving a file step 1, have it as a string that can be written to a file
    pub fn save_to_string(&self) -> String {
        self.text.to_string()
    }

    /// Replace the entire buffer with `contents` and update metadata.
    ///
    /// This is a pure operation — no file-system access; the caller provides
    /// the file contents. Sets the filename, detects the [`FileType`] from
    /// the extension, initializes the syntax [`Lexer`] and token cache,
    /// and resets the cursor and scroll position.
    pub fn load_document(&mut self, contents: &str, filename: Option<&str>) {
        self.text = Rope::from_str(contents);

        if let Some(name) = filename {
            self.filename = name.to_string();
            self.file_type = file_type_from_filename(name);
        } else {
            self.filename = "-".to_string();
            self.file_type = FileType::Unknown;
        }

        // Initialize the lexer based on the detected file type.
        self.lexer = Some(lexer_for_file_type(&self.file_type));

        // Initialize the token cache with one empty vec per line.
        self.token_cache = vec![Vec::new(); self.text.len_lines()];

        self.cx = 0;
        self.cy = 0;
        self.row_offset = 0;
        self.ensure_cursor_visible();
        self.clear_dirty();
        self.search = None;
    }

    /// Apply an `EditorCommand` to `EditorState` (no UI, no IO).
    ///
    /// This is useful for end-to-end style core tests:
    /// `InputKey` → `EditorCommand` → `EditorState`.
    pub fn apply_command(&mut self, cmd: EditorCommand) -> ApplyResult {
        match cmd {
            EditorCommand::Quit => ApplyResult::Quit,

            EditorCommand::MoveLeft => {
                self.cursor_left();
                ApplyResult::Changed
            }
            EditorCommand::MoveRight => {
                self.cursor_right();
                ApplyResult::Changed
            }
            EditorCommand::MoveUp => {
                self.cursor_up();
                ApplyResult::Changed
            }
            EditorCommand::MoveDown => {
                self.cursor_down();
                ApplyResult::Changed
            }

            EditorCommand::InsertChar(c) => {
                self.insert_char(c);
                ApplyResult::Changed
            }
            EditorCommand::InsertNewline => {
                self.insert_newline();
                ApplyResult::Changed
            }
            EditorCommand::DeleteChar => {
                self.delete_char();
                ApplyResult::Changed
            }
            EditorCommand::Backspace => {
                self.backspace();
                ApplyResult::Changed
            }
            EditorCommand::SaveFile | EditorCommand::PromptSaveAs => ApplyResult::NoChange,

            EditorCommand::StartSearch => {
                self.search_start();
                ApplyResult::Changed
            }

            EditorCommand::ToggleVisualLineMode => {
                self.visual_line_mode = !self.visual_line_mode;
                ApplyResult::Changed
            }

            EditorCommand::NoOp => ApplyResult::NoChange,
        }
    }

    /// Adjust `row_offset` and `col_offset` so the cursor is visible.
    ///
    /// Called after every cursor movement or buffer mutation. Shifts the
    /// viewport vertically and horizontally so that the cursor line and
    /// column fall within the on-screen text area.
    pub fn ensure_cursor_visible(&mut self) {
        // vertical scrolling
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

        // horizontal scrolling
        let width = self.text_area_width();
        let screen_col = self.cx_to_screen_col(self.cy, self.cx);

        // If the line is shorter than the screen we want to keep col_offset = 0.
        // Otherwise we slide the viewport so that `cx` lands inside `[col_offset, col_offset+width)`.
        if width == 0 {
            self.col_offset = screen_col;
        } else if screen_col < self.col_offset {
            // cursor moved left of the visible window
            self.col_offset = screen_col;
        } else if screen_col >= self.col_offset + width {
            // cursor moved right past the right edge
            self.col_offset = screen_col + 1 - width;
        }
    }

    /// Height of the editable text area (terminal rows minus status + help).
    pub fn text_area_height(&self) -> usize {
        let (_cols, rows) = self.screen_size;
        (rows as usize).saturating_sub(2)
    }

    pub fn text_area_width(&self) -> usize {
        let (cols, _rows) = self.screen_size;
        cols as usize
    }

    /// The first buffer line currently visible at the top of the screen.
    pub fn row_offset(&self) -> usize {
        self.row_offset
    }

    pub fn col_offset(&self) -> usize {
        self.col_offset
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

        self.set_dirty();
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
        self.ensure_cursor_visible();

        self.set_dirty();
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

        self.ensure_cursor_visible();
    }

    pub fn insert_newline(&mut self) {
        let ropey_line_start = self.text.line_to_char(self.cy);
        let index = ropey_line_start + self.cx;
        self.text.insert_char(index, '\n');
        self.cy += 1;
        self.cx = 0;

        self.ensure_cursor_visible();
        self.set_dirty();
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

    /// Convert a char index into the buffer into a `(cx, cy)` cursor
    /// position. An index at or past the end of the buffer clamps to
    /// `len_chars()`, which lands on the trailing empty line ropey adds
    /// after a final `\n`.
    pub fn char_index_to_cursor(&self, idx: usize) -> (usize, usize) {
        let idx = idx.min(self.text.len_chars());
        let cy = self.text.char_to_line(idx);
        let cx = idx - self.text.line_to_char(cy);
        (cx, cy)
    }

    /// Begin an incremental search, anchored at the current cursor position.
    pub fn search_start(&mut self) {
        let origin = self.text.line_to_char(self.cy) + self.cx;
        self.search = Some(SearchSession::new(origin));
    }

    /// Re-run the active session's match against the whole buffer and, if it
    /// found something, move the cursor there. No match leaves the cursor
    /// exactly where it was.
    fn refresh_search_match(&mut self) {
        let query_match = match self.search.as_ref() {
            Some(session) => session.current_match(&self.save_to_string()),
            None => return,
        };

        if let Some(idx) = query_match {
            let (cx, cy) = self.char_index_to_cursor(idx);
            self.set_cursor(cx, cy);
            self.ensure_cursor_visible();
        }
    }

    /// Append a character to the active search query and jump to the match,
    /// if any. Does nothing if no search is in progress.
    pub fn search_push_char(&mut self, c: char) {
        if let Some(session) = self.search.as_mut() {
            session.push_char(c);
        }
        self.refresh_search_match();
    }

    /// Remove the last character from the active search query and re-match.
    /// Does nothing if no search is in progress.
    pub fn search_backspace(&mut self) {
        if let Some(session) = self.search.as_mut() {
            session.backspace();
        }
        self.refresh_search_match();
    }

    /// Move to the next occurrence of the active query, wrapping around
    /// the buffer if necessary. Does nothing if no search is in progress.
    pub fn search_repeat(&mut self) {
        let next_match = match self.search.as_ref() {
            Some(session) => {
                let current = self.text.line_to_char(self.cy) + self.cx;
                session.repeat_match(&self.save_to_string(), current)
            }
            None => return,
        };

        if let Some(idx) = next_match {
            let (cx, cy) = self.char_index_to_cursor(idx);
            self.set_cursor(cx, cy);
            self.ensure_cursor_visible();
        }
    }

    /// End the search, restoring the cursor to where it was when the
    /// search began. Does nothing if no search is in progress.
    pub fn search_cancel(&mut self) {
        if let Some(session) = self.search.take() {
            let (cx, cy) = self.char_index_to_cursor(session.origin());
            self.set_cursor(cx, cy);
            self.ensure_cursor_visible();
        }
    }

    /// End the search, leaving the cursor at the current match.
    pub fn search_accept(&mut self) {
        self.search = None;
    }

    pub fn is_searching(&self) -> bool {
        self.search.is_some()
    }

    /// The query typed so far, or `None` if no search is in progress.
    pub fn search_query(&self) -> Option<&str> {
        self.search.as_ref().map(|session| session.query.as_str())
    }

    /// What the help line at the bottom of the screen should currently
    /// show: the "Save as" prompt input, the active search query, or the
    /// default help message — in that priority order.
    pub fn status_help_line(&self) -> String {
        if let Some(ref input) = self.prompt_buffer {
            format!("Save as: {}", input)
        } else if let Some(query) = self.search_query() {
            format!("I-search: {}", query)
        } else {
            self.help_message.clone()
        }
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
        if self.visual_line_mode {
            self.move_cursor_visual_up();
        } else if self.cy > 0 {
            self.cy -= 1;
            self.cx = self.cx.min(self.current_line_len());
        }
        self.ensure_cursor_visible();
    }
    pub fn cursor_down(&mut self) {
        if self.visual_line_mode {
            self.move_cursor_visual_down();
        } else if self.cy < self.index_of_last_line() {
            self.cy += 1;
            self.cx = self.cx.min(self.current_line_len());
        }
        self.ensure_cursor_visible();
    }

    /// How many characters come before chunk `idx` in a line's wrapped
    /// chunks — the buffer-column offset where that chunk begins.
    fn chars_before_chunk(chunks: &[String], idx: usize) -> usize {
        chunks[..idx].iter().map(|c| c.chars().count()).sum()
    }

    /// The inverse of `wrapped_cursor_offset`'s column half: given one
    /// wrapped chunk and a target display column, which character index
    /// in that chunk is closest to it? Clamps to the end of the chunk if
    /// `target_col` is wider than the chunk itself.
    fn char_offset_for_col(&self, chunk: &str, target_col: usize) -> usize {
        let mut col = 0;
        for (i, c) in chunk.chars().enumerate() {
            if col >= target_col {
                return i;
            }
            col += self.display_width(c);
        }
        chunk.chars().count()
    }

    /// `cursor_down`'s wrapped-mode behaviour: move to the next chunk on
    /// the same buffer line if there is one, otherwise to the first chunk
    /// of the next buffer line. Either way, `col_within_row` (the
    /// cursor's current wrapped column) is used as a one-shot target
    /// column in the destination chunk — not remembered across repeated
    /// moves, matching plain (non-wrapped) `cursor_down`.
    fn move_cursor_visual_down(&mut self) {
        let width = self.text_area_width();
        let chunks = self.wrapped_lines(self.cy, width);
        let (row_within_line, col_within_row) = self.wrapped_cursor_offset(self.cy, self.cx, width);

        if row_within_line + 1 < chunks.len() {
            let target_row = row_within_line + 1;
            let chars_before = Self::chars_before_chunk(&chunks, target_row);
            self.cx = chars_before + self.char_offset_for_col(&chunks[target_row], col_within_row);
        } else if self.cy < self.index_of_last_line() {
            self.cy += 1;
            let next_chunks = self.wrapped_lines(self.cy, width);
            self.cx = next_chunks
                .first()
                .map_or(0, |chunk| self.char_offset_for_col(chunk, col_within_row));
        }
    }

    /// `cursor_up`'s wrapped-mode behaviour — the mirror image of
    /// `move_cursor_visual_down`: previous chunk on the same buffer line,
    /// or the *last* chunk of the *previous* buffer line.
    fn move_cursor_visual_up(&mut self) {
        let width = self.text_area_width();
        let chunks = self.wrapped_lines(self.cy, width);
        let (row_within_line, col_within_row) = self.wrapped_cursor_offset(self.cy, self.cx, width);

        if row_within_line > 0 {
            let target_row = row_within_line - 1;
            let chars_before = Self::chars_before_chunk(&chunks, target_row);
            self.cx = chars_before + self.char_offset_for_col(&chunks[target_row], col_within_row);
        } else if self.cy > 0 {
            self.cy -= 1;
            let prev_chunks = self.wrapped_lines(self.cy, width);
            self.cx = match prev_chunks.len().checked_sub(1) {
                Some(last_row) => {
                    let chars_before = Self::chars_before_chunk(&prev_chunks, last_row);
                    chars_before + self.char_offset_for_col(&prev_chunks[last_row], col_within_row)
                }
                None => 0,
            };
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

fn file_type_from_filename(name: &str) -> FileType {
    let path = Path::new(name);
    match path.extension().and_then(|s| s.to_str()) {
        Some("rs") => FileType::Rust,
        Some("c") | Some("h") => FileType::C,
        Some(_) => FileType::Text,
        None => FileType::Unknown,
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
        self.prompt_buffer = None;
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
/// Keys that should end an active search rather than be typed into the
/// query, because they lead toward quitting (`Ctrl-q` directly, or
/// `Ctrl-x` which may start `C-x C-c`/`C-x C-s`). The caller should cancel
/// the search first, then let the key be processed normally — otherwise
/// quitting (or saving) is unreachable while a search is open.
pub fn escapes_search(key: InputKey) -> bool {
    matches!(key, InputKey::Ctrl('q') | InputKey::Ctrl('x'))
}

/// Whether receiving this command should cancel a pending quit
/// confirmation (the "quit N more times" counter). `NoOp` must not cancel
/// it — arming the `C-x` prefix produces `NoOp`, and letting it cancel the
/// counter would make completing `C-x C-c` across two key presses
/// impossible. `Quit` itself is handled by its own branch and is never a
/// "cancelling" command either.
pub fn cancels_pending_quit(cmd: EditorCommand) -> bool {
    !matches!(cmd, EditorCommand::Quit | EditorCommand::NoOp)
}

pub fn command_from_key(
    key: InputKey,
    saw_ctrl_x: &mut bool,
    saw_ctrl_c: &mut bool,
) -> EditorCommand {
    // Quit on Ctrl-Q. Alternative to C-x C-c.
    if key == InputKey::Ctrl('q') {
        *saw_ctrl_x = false;
        *saw_ctrl_c = false;
        return EditorCommand::Quit;
    }

    // Ctrl-X prefix handling. Starting this prefix abandons any pending
    // C-c prefix rather than leaving it armed for an unrelated keypress.
    if key == InputKey::Ctrl('x') {
        *saw_ctrl_x = true;
        *saw_ctrl_c = false;
        return EditorCommand::NoOp;
    }

    if *saw_ctrl_x {
        *saw_ctrl_x = false;
        return match key {
            InputKey::Ctrl('c') => EditorCommand::Quit,
            InputKey::Ctrl('s') => EditorCommand::SaveFile,
            _ => EditorCommand::NoOp,
        };
    }

    // Ctrl-C prefix handling — a second, independent prefix (mirrors
    // Emacs' reserved user/minor-mode C-c prefix) for editor-level
    // toggles like `visual_line_mode`. Only reached once the C-x-prefix
    // paths above have already returned, so `Ctrl('c')` completing
    // `C-x C-c` (quit) can never be mistaken for a fresh C-c press here.
    if *saw_ctrl_c {
        *saw_ctrl_c = false;
        return match key {
            InputKey::Char('l') => EditorCommand::ToggleVisualLineMode,
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
        InputKey::Ctrl('s') => EditorCommand::StartSearch,
        InputKey::Ctrl('c') => {
            *saw_ctrl_c = true;
            EditorCommand::NoOp
        }
        InputKey::Ctrl(_) => EditorCommand::NoOp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_editor_state_defaults_to_visual_line_mode_off() {
        let state = EditorState::new((80, 24));

        assert!(!state.visual_line_mode);
    }

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
