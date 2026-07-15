//! Pure, UI-agnostic text search.
//!
//! Step 1: only the `find_from` primitive lives here.  It knows nothing about
//! `EditorState`, cursors, scrolling, or crossterm — it is just a function over
//! strings.  Everything is expressed in **char indices** (Unicode scalar values),
//! never byte offsets, so callers in the editor (which think in char positions)
//! can use the results directly.

/// Which way a search scans the haystack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

/// Find the nearest occurrence of `needle` in `haystack` on the `direction`
/// side of char index `start` — at or after `start` when `Forward`, at or
/// before `start` when `Backward`.
///
/// Returns the **char index** of the start of the match, or `None` if there is
/// no match.
///
/// Behaviour:
/// - An empty `needle` never matches (returns `None`).  This mirrors Emacs,
///   where an empty incremental-search query does not move point.
/// - `start` is clamped to the length of `haystack`; a `start` past the end
///   (or, for `Backward`, before the start) simply means "no match from here".
/// - If `wrap` is `true` and no match is found on the `direction` side of
///   `start`, the search continues from the other end of the buffer (so the
///   whole text is covered exactly once). If `wrap` is `false`, only the
///   `direction` side of `start` is searched.
pub fn find_from(
    haystack: &str,
    needle: &str,
    start: usize,
    wrap: bool,
    direction: Direction,
) -> Option<usize> {
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

    match direction {
        Direction::Forward => {
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
        Direction::Backward => {
            // 1) Search backward in the head `haystack[..byte_start]` — the
            //    last match whose end is at or before `start`.
            if let Some(abs_byte) = haystack[..byte_start].rfind(needle) {
                return Some(byte_to_char_index(haystack, abs_byte));
            }

            // 2) Nothing before `start`. If wrapping is allowed, search the
            //    whole string for its last match — that hit is necessarily
            //    at or after `start`, otherwise step 1 would have found it.
            if wrap && let Some(abs_byte) = haystack.rfind(needle) {
                return Some(byte_to_char_index(haystack, abs_byte));
            }

            None
        }
    }
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
    /// Which way the session currently searches. Set at construction and
    /// flipped by the explicit "search again" action (`repeat`), never by
    /// typing — typing always re-anchors to `origin` in whichever direction
    /// is currently active.
    direction: Direction,
    /// Whether the most recent match attempt (`current_match` or `repeat`)
    /// succeeded. Drives the "Failing I-search" status-line indicator.
    /// Stored rather than recomputed live, because a live recompute from
    /// `query` alone would be wrong right after a wrapped `repeat`: it only
    /// re-checks `haystack[..origin]`/`haystack[origin..]` (no wrap), so it
    /// could report "failing" even though `repeat` just wrapped around and
    /// landed on a real match elsewhere in the buffer.
    found: bool,
}

impl SearchSession {
    pub fn new(origin: usize, direction: Direction) -> Self {
        SearchSession {
            query: String::new(),
            origin,
            direction,
            found: true,
        }
    }

    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
    }

    pub fn backspace(&mut self) {
        self.query.pop();
    }

    /// Where the current query matches in `haystack`, searching from `origin`
    /// in the session's current `direction` only — no wraparound. Wrapping is
    /// reserved for the explicit "search again" action (`repeat`), not for
    /// typing.
    pub fn current_match(&mut self, haystack: &str) -> Option<usize> {
        let result = find_from(haystack, &self.query, self.origin, false, self.direction);
        self.found = result.is_some();
        result
    }

    /// Move to the next (or previous, if `direction` is `Backward`)
    /// occurrence of the query for the explicit "search again" action, and
    /// record `direction` as the session's new direction — this is what
    /// lets C-s/C-r flip an active session's direction mid-search.
    ///
    /// `after` is the position the cursor is already at (the match
    /// `current_match` or a previous `repeat` found) — forward search
    /// starts at `after + 1`, backward search starts at `after`, so this
    /// never re-reports the match already sitting under the cursor. Wraps
    /// around the buffer if nothing is found on the way to the end/start.
    pub fn repeat(&mut self, haystack: &str, after: usize, direction: Direction) -> Option<usize> {
        self.direction = direction;
        let start = match direction {
            Direction::Forward => after + 1,
            Direction::Backward => after,
        };
        let result = find_from(haystack, &self.query, start, true, direction);
        self.found = result.is_some();
        result
    }

    /// The char index the search began at — used to restore the cursor if
    /// the search is cancelled.
    pub fn origin(&self) -> usize {
        self.origin
    }

    /// The direction the session currently searches in — for the "I-search
    /// backward" status-line wording.
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// Whether the query currently has no match — drives the "Failing
    /// I-search" status-line prefix. An empty query is never "failing"
    /// (real Emacs shows plain "I-search:" immediately after C-s/C-r,
    /// before anything's been typed), regardless of `found`'s stored value.
    pub fn is_failing(&self) -> bool {
        !self.query.is_empty() && !self.found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finds_match_after_start() {
        // The core job: locate the needle at or after `start`.
        assert_eq!(
            find_from("hello world", "world", 0, false, Direction::Forward),
            Some(6)
        );
    }

    #[test]
    fn start_skips_an_earlier_match() {
        // "abcabc": starting past the first "abc" finds the second at 3.
        assert_eq!(
            find_from("abcabc", "abc", 1, false, Direction::Forward),
            Some(3)
        );
    }

    #[test]
    fn no_match_returns_none() {
        assert_eq!(
            find_from("hello", "xyz", 0, false, Direction::Forward),
            None
        );
    }

    #[test]
    fn wrap_finds_earlier_match_when_none_after_start() {
        // Nothing in [5..], but wrap lets us find the "abc" at 0.
        assert_eq!(
            find_from("abc abc", "abc", 5, true, Direction::Forward),
            Some(0)
        );
        // Same input, wrap off → must NOT find it.
        assert_eq!(
            find_from("abc abc", "abc", 5, false, Direction::Forward),
            None
        );
    }

    #[test]
    fn empty_needle_never_matches() {
        // Emacs behaviour: an empty query does not jump.
        assert_eq!(find_from("hello", "", 0, true, Direction::Forward), None);
    }

    #[test]
    fn returns_char_index_not_byte_index() {
        // "áé" is 2 chars but 4 bytes. 'x' is char index 2 (byte index 4).
        // This guards the one subtle part of the implementation.
        assert_eq!(find_from("áéx", "x", 0, false, Direction::Forward), Some(2));
    }

    // --- Backward direction: mirrors every test above, one-for-one ---

    #[test]
    fn finds_match_before_start() {
        // Backward analog of finds_match_after_start: searching backward
        // from the end of "hello world" for "hello" finds it at 0.
        assert_eq!(
            find_from("hello world", "hello", 11, false, Direction::Backward),
            Some(0)
        );
    }

    #[test]
    fn start_skips_a_later_match() {
        // "abcabc": the second "abc" starts at 3 but ends at 6, which is
        // past start=4, so a backward search from 4 must not count it —
        // only the first "abc" (ending at 3, <= start) qualifies.
        assert_eq!(
            find_from("abcabc", "abc", 4, false, Direction::Backward),
            Some(0)
        );
    }

    #[test]
    fn no_match_returns_none_backward() {
        assert_eq!(
            find_from("hello", "xyz", 5, false, Direction::Backward),
            None
        );
    }

    #[test]
    fn wrap_finds_later_match_when_none_before_start() {
        // Nothing in [..2], but wrap lets us find the "abc" at 4.
        assert_eq!(
            find_from("abc abc", "abc", 2, true, Direction::Backward),
            Some(4)
        );
        // Same input, wrap off → must NOT find it.
        assert_eq!(
            find_from("abc abc", "abc", 2, false, Direction::Backward),
            None
        );
    }

    #[test]
    fn empty_needle_never_matches_backward() {
        assert_eq!(find_from("hello", "", 5, true, Direction::Backward), None);
    }

    #[test]
    fn returns_char_index_not_byte_index_backward() {
        // "áéx": 'é' is char index 1 but byte offset 2. Searching backward
        // from the end must report the char index, not the byte offset.
        assert_eq!(
            find_from("áéx", "é", 3, false, Direction::Backward),
            Some(1)
        );
    }

    // --- SearchSession: query bookkeeping, no EditorState involved ---

    #[test]
    fn push_char_accumulates_query() {
        let mut session = SearchSession::new(0, Direction::Forward);
        session.push_char('c');
        session.push_char('a');
        session.push_char('t');
        assert_eq!(session.query, "cat");
    }

    #[test]
    fn backspace_shrinks_query() {
        let mut session = SearchSession::new(0, Direction::Forward);
        session.push_char('c');
        session.push_char('a');
        session.push_char('t');
        session.backspace();
        assert_eq!(session.query, "ca");
    }

    #[test]
    fn empty_query_has_no_match() {
        // Mirrors find_from's own rule: an empty needle never matches.
        let mut session = SearchSession::new(0, Direction::Forward);
        assert_eq!(session.current_match("hello world"), None);
    }

    #[test]
    fn growing_query_refinds_from_origin_not_zero() {
        // "cat cat": origin sits right at the second "cat" (index 4), so a
        // correct implementation must search from `origin`, not from 0 —
        // otherwise every match below would be Some(0) instead of Some(4).
        let haystack = "cat cat";
        let mut session = SearchSession::new(4, Direction::Forward);

        session.push_char('c');
        assert_eq!(session.current_match(haystack), Some(4));

        session.push_char('a');
        assert_eq!(session.current_match(haystack), Some(4));

        session.push_char('t');
        assert_eq!(session.current_match(haystack), Some(4));
    }

    #[test]
    fn growing_query_refinds_from_origin_not_end_backward() {
        // "cat cat": origin sits right after the first "cat" (index 3), so a
        // correct backward implementation must search from `origin`, not
        // from the end of the buffer — otherwise every match below would be
        // Some(4) (the second "cat") instead of Some(0).
        let haystack = "cat cat";
        let mut session = SearchSession::new(3, Direction::Backward);

        session.push_char('c');
        assert_eq!(session.current_match(haystack), Some(0));

        session.push_char('a');
        assert_eq!(session.current_match(haystack), Some(0));

        session.push_char('t');
        assert_eq!(session.current_match(haystack), Some(0));
    }

    // --- SearchSession: repeat(direction), for the explicit "search again" action ---
    //
    // `after` is where the cursor already is (the match `current_match`
    // already found and jumped to). Forward `repeat` finds the *next* one
    // past that — never `after` itself — which is why the first case below
    // is asked to move from 0 to 4, not confirm 0 again. `repeat` also
    // performs the step *and* records the direction it was asked to search
    // in, which is what lets C-s/C-r flip an active session's direction
    // mid-search (covered further down, once both directions are in play).

    #[test]
    fn repeat_forward_finds_next_occurrence_after_given_position() {
        let mut session = SearchSession::new(0, Direction::Forward);
        for c in "cat".chars() {
            session.push_char(c);
        }
        // "cat cat cat": occurrences at 0, 4, 8. Already at 0, next is 4.
        assert_eq!(
            session.repeat("cat cat cat", 0, Direction::Forward),
            Some(4)
        );
    }

    #[test]
    fn repeat_forward_wraps_to_first_occurrence_when_none_after_position() {
        let mut session = SearchSession::new(0, Direction::Forward);
        for c in "cat".chars() {
            session.push_char(c);
        }
        // Already at the last occurrence (8); nothing follows, so it wraps.
        assert_eq!(
            session.repeat("cat cat cat", 8, Direction::Forward),
            Some(0)
        );
    }

    #[test]
    fn repeat_backward_finds_previous_occurrence_before_given_position() {
        let mut session = SearchSession::new(0, Direction::Forward);
        for c in "cat".chars() {
            session.push_char(c);
        }
        // "cat cat cat": occurrences at 0, 4, 8. Already at 8, previous is 4.
        assert_eq!(
            session.repeat("cat cat cat", 8, Direction::Backward),
            Some(4)
        );
    }

    #[test]
    fn repeat_backward_wraps_to_last_occurrence_when_none_before_position() {
        let mut session = SearchSession::new(0, Direction::Forward);
        for c in "cat".chars() {
            session.push_char(c);
        }
        // Already at the first occurrence (0); nothing precedes it, so it
        // wraps to the last occurrence (8).
        assert_eq!(
            session.repeat("cat cat cat", 0, Direction::Backward),
            Some(8)
        );
    }

    #[test]
    fn repeat_flips_direction_and_steps_from_current_position_not_origin() {
        // Mirrors docs/search-reverse.md decision 3: flipping direction
        // steps from wherever the cursor currently is, not back to origin.
        let haystack = "cat cat cat";
        let mut session = SearchSession::new(0, Direction::Forward);
        for c in "cat".chars() {
            session.push_char(c);
        }

        // Walk forward twice: 0 -> 4 -> 8.
        assert_eq!(session.repeat(haystack, 0, Direction::Forward), Some(4));
        assert_eq!(session.repeat(haystack, 4, Direction::Forward), Some(8));

        // Flip to backward from the current position (8), not origin (0):
        // steps back to 4, then to 0 — not a re-jump to some origin-relative
        // match.
        assert_eq!(session.repeat(haystack, 8, Direction::Backward), Some(4));
        assert_eq!(session.repeat(haystack, 4, Direction::Backward), Some(0));
    }

    // --- SearchSession: found / is_failing, for the "Failing I-search" indicator ---
    //
    // `found` is updated as a side effect of whichever call actually
    // computed a match (`current_match` or `repeat`) — not recomputed live
    // from the query — so it reflects what's really on screen even after a
    // wrapped `repeat` lands on a match that a fresh no-wrap check from
    // `origin` wouldn't find on its own.

    #[test]
    fn is_failing_false_for_a_brand_new_session() {
        // No query typed yet — never "failing", per decision 11.
        let session = SearchSession::new(0, Direction::Forward);
        assert!(!session.is_failing());
    }

    #[test]
    fn is_failing_false_after_a_successful_match() {
        let mut session = SearchSession::new(0, Direction::Forward);
        for c in "cat".chars() {
            session.push_char(c);
        }
        session.current_match("cat");
        assert!(!session.is_failing());
    }

    #[test]
    fn is_failing_true_after_no_match() {
        let mut session = SearchSession::new(0, Direction::Forward);
        session.push_char('z');
        session.current_match("cat"); // no "z" in "cat"
        assert!(session.is_failing());
    }

    #[test]
    fn is_failing_false_once_query_is_backspaced_to_empty() {
        let mut session = SearchSession::new(0, Direction::Forward);
        session.push_char('z');
        session.current_match("cat"); // fails
        assert!(session.is_failing());

        session.backspace(); // query now empty again
        assert!(!session.is_failing()); // empty query is never "failing"
    }

    #[test]
    fn is_failing_tracks_repeat_too() {
        let mut session = SearchSession::new(0, Direction::Forward);
        for c in "cat".chars() {
            session.push_char(c);
        }
        assert_eq!(
            session.repeat("cat cat cat", 0, Direction::Forward),
            Some(4)
        );
        assert!(!session.is_failing());

        // "catz" appears nowhere in the haystack.
        session.push_char('z');
        assert_eq!(session.repeat("cat cat cat", 4, Direction::Forward), None);
        assert!(session.is_failing());
    }
}
