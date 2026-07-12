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
}
