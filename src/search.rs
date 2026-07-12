//! Pure, UI-agnostic text search.
//!
//! Step 1: only the `find_from` primitive lives here.  It knows nothing about
//! `EditorState`, cursors, scrolling, or crossterm — it is just a function over
//! strings.  Everything is expressed in **char indices** (Unicode scalar values),
//! never byte offsets, so callers in the editor (which think in char positions)
//! can use the results directly.

/// Find the next occurrence of `needle` in `haystack`, starting the search at
/// char index `start`.
///
/// Returns the **char index** of the start of the match, or `None` if there is
/// no match.
///
/// Behaviour:
/// - An empty `needle` never matches (returns `None`).  This mirrors Emacs,
///   where an empty incremental-search query does not move point.
/// - `start` is clamped to the length of `haystack`; a `start` past the end
///   simply means "no match from here".
/// - If `wrap` is `true` and no match is found in `haystack[start..]`, the
///   search continues from the beginning of the buffer (so the whole text is
///   covered exactly once).  If `wrap` is `false`, only `haystack[start..]`
///   is searched.
pub fn find_from(haystack: &str, needle: &str, start: usize, wrap: bool) -> Option<usize> {
    // Emacs behaviour: an empty query never matches / never moves point.
    if needle.is_empty() {
        return None;
    }

    // `start` is a *char* index; clamp it to the number of chars so a `start`
    // past the end simply means "search nothing from here".
    let char_len = haystack.chars().count();
    let start = start.min(char_len);

    // Convert the char-index `start` into a *byte* offset, because `str::find`
    // and slicing both work in bytes.
    let byte_start = char_index_to_byte(haystack, start);

    // 1) Search forward in the tail `haystack[byte_start..]`.
    if let Some(rel_byte) = haystack[byte_start..].find(needle) {
        let abs_byte = byte_start + rel_byte;
        return Some(byte_to_char_index(haystack, abs_byte));
    }

    // 2) Nothing after `start`. If wrapping is allowed, search from the top.
    //    (We search the whole string; the first hit is necessarily before
    //    `start`, otherwise step 1 would have found it.)
    if wrap && let Some(abs_byte) = haystack.find(needle) {
        return Some(byte_to_char_index(haystack, abs_byte));
    }

    None
}

/// Convert a char index into the corresponding byte offset within `s`.
/// If `char_idx` is at (or past) the end, returns `s.len()` (the end byte).
fn char_index_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(byte, _)| byte)
        .unwrap_or(s.len())
}

/// Convert a byte offset (which must land on a char boundary) into a char index.
fn byte_to_char_index(s: &str, byte_idx: usize) -> usize {
    s[..byte_idx].chars().count()
}

/// Bookkeeping for an in-progress incremental search: the query typed so far,
/// and the char index the cursor was at when the search started.
///
/// This struct knows nothing about `EditorState` — it only tracks its own
/// fields and can compute where the current query matches in a haystack
/// that's handed to it. Nothing here moves a cursor.
pub struct SearchSession {
    /// What the user has typed so far.
    pub query: String,
    /// Char index the cursor was at when the search started. Every match is
    /// searched for starting here — Emacs semantics, so the match "grows"
    /// from the origin as the query grows, rather than drifting forward from
    /// wherever the previous (shorter) query happened to match.
    origin: usize,
}

impl SearchSession {
    pub fn new(origin: usize) -> Self {
        SearchSession {
            query: String::new(),
            origin,
        }
    }

    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
    }

    pub fn backspace(&mut self) {
        self.query.pop();
    }

    /// Where the current query matches in `haystack`, searching forward from
    /// `origin` only — no wraparound. Wrapping is reserved for the explicit
    /// "search again" action (Commit 4's `search_repeat`), not for typing.
    pub fn current_match(&self, haystack: &str) -> Option<usize> {
        find_from(haystack, &self.query, self.origin, false)
    }

    /// Where the *next* occurrence of the query is, for the explicit
    /// "search again" action. `after` is the position the cursor is
    /// already at (the match `current_match` previously found) — the
    /// search starts at `after + 1`, not `after`, so this never
    /// re-reports the match already sitting under the cursor. Wraps
    /// around the buffer if nothing is found before the end.
    pub fn repeat_match(&self, haystack: &str, after: usize) -> Option<usize> {
        find_from(haystack, &self.query, after + 1, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finds_match_after_start() {
        // The core job: locate the needle at or after `start`.
        assert_eq!(find_from("hello world", "world", 0, false), Some(6));
    }

    #[test]
    fn start_skips_an_earlier_match() {
        // "abcabc": starting past the first "abc" finds the second at 3.
        assert_eq!(find_from("abcabc", "abc", 1, false), Some(3));
    }

    #[test]
    fn no_match_returns_none() {
        assert_eq!(find_from("hello", "xyz", 0, false), None);
    }

    #[test]
    fn wrap_finds_earlier_match_when_none_after_start() {
        // Nothing in [5..], but wrap lets us find the "abc" at 0.
        assert_eq!(find_from("abc abc", "abc", 5, true), Some(0));
        // Same input, wrap off → must NOT find it.
        assert_eq!(find_from("abc abc", "abc", 5, false), None);
    }

    #[test]
    fn empty_needle_never_matches() {
        // Emacs behaviour: an empty query does not jump.
        assert_eq!(find_from("hello", "", 0, true), None);
    }

    #[test]
    fn returns_char_index_not_byte_index() {
        // "áé" is 2 chars but 4 bytes. 'x' is char index 2 (byte index 4).
        // This guards the one subtle part of the implementation.
        assert_eq!(find_from("áéx", "x", 0, false), Some(2));
    }

    // --- SearchSession: query bookkeeping, no EditorState involved ---

    #[test]
    fn push_char_accumulates_query() {
        let mut session = SearchSession::new(0);
        session.push_char('c');
        session.push_char('a');
        session.push_char('t');
        assert_eq!(session.query, "cat");
    }

    #[test]
    fn backspace_shrinks_query() {
        let mut session = SearchSession::new(0);
        session.push_char('c');
        session.push_char('a');
        session.push_char('t');
        session.backspace();
        assert_eq!(session.query, "ca");
    }

    #[test]
    fn empty_query_has_no_match() {
        // Mirrors find_from's own rule: an empty needle never matches.
        let session = SearchSession::new(0);
        assert_eq!(session.current_match("hello world"), None);
    }

    #[test]
    fn growing_query_refinds_from_origin_not_zero() {
        // "cat cat": origin sits right at the second "cat" (index 4), so a
        // correct implementation must search from `origin`, not from 0 —
        // otherwise every match below would be Some(0) instead of Some(4).
        let haystack = "cat cat";
        let mut session = SearchSession::new(4);

        session.push_char('c');
        assert_eq!(session.current_match(haystack), Some(4));

        session.push_char('a');
        assert_eq!(session.current_match(haystack), Some(4));

        session.push_char('t');
        assert_eq!(session.current_match(haystack), Some(4));
    }

    // --- SearchSession: repeat_match, for the explicit "search again" action ---
    //
    // `after` is where the cursor already is (the match `current_match`
    // already found and jumped to). `repeat_match` finds the *next* one
    // past that — never `after` itself — which is why the first case below
    // is asked to move from 0 to 4, not confirm 0 again.

    #[test]
    fn repeat_match_finds_next_occurrence_after_given_position() {
        let mut session = SearchSession::new(0);
        for c in "cat".chars() {
            session.push_char(c);
        }
        // "cat cat cat": occurrences at 0, 4, 8. Already at 0, next is 4.
        assert_eq!(session.repeat_match("cat cat cat", 0), Some(4));
    }

    #[test]
    fn repeat_match_wraps_to_first_occurrence_when_none_after_position() {
        let mut session = SearchSession::new(0);
        for c in "cat".chars() {
            session.push_char(c);
        }
        // Already at the last occurrence (8); nothing follows, so it wraps.
        assert_eq!(session.repeat_match("cat cat cat", 8), Some(0));
    }
}
