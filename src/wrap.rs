//! Word-wrap (`visual_line_mode`) support: turning one buffer line into
//! display-width-sized chunks, composing those into paintable screen rows,
//! mapping buffer positions to wrapped screen positions and back, and the
//! wrapped-mode cursor Up/Down movement built on top of all of that.

use crate::EditorState;

impl EditorState {
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
    pub(crate) fn move_cursor_visual_down(&mut self) {
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
    pub(crate) fn move_cursor_visual_up(&mut self) {
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
}
