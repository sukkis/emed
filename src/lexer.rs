use crate::FileType;

/// A language-agnostic token category.
///
/// Kept intentionally small (à la Kilo).  New variants can be added as needed,
/// but every concrete lexer maps into these same kinds so the theme layer
/// stays decoupled from any particular language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Ordinary source text — default foreground color.
    Normal,
    /// Language keyword (`fn`, `let`, `if`, `while`, `return`, …).
    _Keyword,
    /// Built-in or well-known type (`i32`, `String`, `int`, …).
    _Type,
    /// String literal (including the quotes).
    String,
    /// Numeric literal (`42`, `3.14`, `0xff`).
    Number,
    /// Comment (line or block).
    Comment,
    /// Punctuation / operators (`+`, `->`, `::`, …).
    Operator,
}

/// One coloured span within a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Char index (not byte offset) of the first character within the line —
    /// lines are tokenized over `Vec<char>`, so multi-byte characters are
    /// always whole positions, never split mid-character.
    pub start: usize,
    /// Number of characters this token spans.
    pub len: usize,
    pub kind: TokenKind,
}

/// A lexer turns a single line of source text into a sequence of tokens.
///
/// Why a trait?
/// - Each language gets its own small struct (e.g. `RustLexer`, `CLexer`).
/// - `EditorState` holds a `Box<dyn Lexer>` chosen when a file is opened,
///   so the rest of the code never mentions a specific language.
/// - Adding a new language = one new file + one `impl Lexer`.
pub trait Lexer {
    fn tokenize_line(&self, line: &str, in_comment: bool) -> (Vec<Token>, bool);
}

/// Pick the right lexer based on file type.
/// This is the only function lib.rs needs to call —
/// it never sees RustLexer, PlainLexer, etc.
pub fn lexer_for_file_type(ft: &FileType) -> Box<dyn Lexer> {
    match ft {
        FileType::Rust => Box::new(RustLexer),
        _ => Box::new(PlainLexer),
    }
}

pub struct RustLexer;
pub struct PlainLexer;

// ── Shared highlighting primitives ──────────────────────────────────

/// A digit counts as a number only if it's NOT preceded by a letter or underscore.
/// This prevents highlighting the "16" in "u16" or "32" in "my_var32".
fn is_number_start(chars: &[char], i: usize) -> bool {
    chars[i].is_ascii_digit()
        && (i == 0 || !(chars[i - 1].is_ascii_alphanumeric() || chars[i - 1] == '_'))
}

/// If `chars[start]` is an opening `"`, find the index of the matching
/// closing `"` on this same line, honoring backslash-escapes (a `\` always
/// consumes itself plus the following character, whatever it is — this
/// correctly skips `\"` and `\\` without needing to know Rust's actual
/// escape-sequence set).
///
/// Returns `None` if no closing quote is found before end of line — the
/// caller then treats the opening `"` as ordinary text rather than
/// colour the rest of the line as an incorrectly open-ended string
/// (single-line-only strings; see docs/rust-highlighting.md).
fn find_string_end(chars: &[char], start: usize) -> Option<usize> {
    let len = chars.len();
    let mut j = start + 1;
    while j < len {
        match chars[j] {
            '\\' => j += 2,
            '"' => return Some(j),
            _ => j += 1,
        }
    }
    None
}

/// If `chars[start]` is an opening `'`, find the index of the closing `'`
/// of a char literal — but only for the narrow, fixed-length shape a char
/// literal actually has: one plain character, or one backslash-escaped
/// character (same generic "\` skips the next char" rule as strings),
/// immediately followed by a closing `'`.
///
/// This is what disambiguates a char literal from a lifetime (`'a`,
/// `'static`): a lifetime is never followed by a bare `'`, so it simply
/// never matches this fixed-length shape and `None` is returned — the `'`
/// is then left as ordinary text by the caller, same as an unterminated
/// string. Unicode escapes (`'\u{1F600}'`) are out of scope for now (see
/// docs/rust-highlighting.md) since they aren't fixed-length.
fn find_char_literal_end(chars: &[char], start: usize) -> Option<usize> {
    let len = chars.len();
    if start + 1 >= len {
        return None;
    }
    let close = if chars[start + 1] == '\\' {
        start + 3
    } else {
        start + 2
    };
    if close < len && chars[close] == '\'' {
        Some(close)
    } else {
        None
    }
}

/// Does a string, char literal, or number literal start at `chars[i]`?
/// Shared by the Normal-run scan (stop here) and, individually, by the
/// main loop's own checks (which branch also needs to know *which* kind
/// matched, not just whether one did).
fn token_starts_at(chars: &[char], i: usize) -> bool {
    (chars[i] == '"' && find_string_end(chars, i).is_some())
        || (chars[i] == '\'' && find_char_literal_end(chars, i).is_some())
        || is_number_start(chars, i)
}

/// Tokenize a line using only the universal "number vs. normal" rule.
///
/// Every language-specific lexer can call this as a baseline pass.
/// Later, language-specific lexers can either:
///   - Call this and then refine the tokens (split Normal spans into keywords, etc.)
///   - Or build their own loop that calls `is_number_start` at the right point.
fn tokenize_numbers(line: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if is_number_start(&chars, i) {
            let start = i;
            while i < len && chars[i].is_ascii_digit() {
                i += 1;
            }
            tokens.push(Token {
                start,
                len: i - start,
                kind: TokenKind::Number,
            });
        } else {
            let start = i;
            while i < len && !is_number_start(&chars, i) {
                i += 1;
            }
            tokens.push(Token {
                start,
                len: i - start,
                kind: TokenKind::Normal,
            });
        }
    }
    tokens
}

// ── Concrete lexers ─────────────────────────────────────────────────

impl Lexer for RustLexer {
    fn tokenize_line(&self, line: &str, _in_comment: bool) -> (Vec<Token>, bool) {
        // Single-pass, priority-ordered scan: at each position, check for a
        // string start before a number start, so a token is never created
        // wrong in the first place (e.g. digits inside a string literal
        // must never become a separate Number token). Later categories
        // (comments, keywords, operators) slot into this same ordered scan.
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut tokens = Vec::new();
        let mut i = 0;

        while i < len {
            // Unterminated (find_string_end returns None): falls through and
            // treats the quote as ordinary text, absorbed into the
            // surrounding Normal run.
            if chars[i] == '"'
                && let Some(end) = find_string_end(&chars, i)
            {
                tokens.push(Token {
                    start: i,
                    len: end - i + 1,
                    kind: TokenKind::String,
                });
                i = end + 1;
                continue;
            }

            // A lifetime (`'a`, `'static`) never matches this fixed-length
            // shape, so it falls through untouched — see
            // find_char_literal_end's doc comment.
            if chars[i] == '\''
                && let Some(end) = find_char_literal_end(&chars, i)
            {
                tokens.push(Token {
                    start: i,
                    len: end - i + 1,
                    kind: TokenKind::String,
                });
                i = end + 1;
                continue;
            }

            if is_number_start(&chars, i) {
                let start = i;
                while i < len && chars[i].is_ascii_digit() {
                    i += 1;
                }
                tokens.push(Token {
                    start,
                    len: i - start,
                    kind: TokenKind::Number,
                });
                continue;
            }

            let start = i;
            while i < len && !token_starts_at(&chars, i) {
                i += 1;
            }
            tokens.push(Token {
                start,
                len: i - start,
                kind: TokenKind::Normal,
            });
        }

        (tokens, false)
    }
}

impl Lexer for PlainLexer {
    fn tokenize_line(&self, line: &str, _in_comment: bool) -> (Vec<Token>, bool) {
        (tokenize_numbers(line), false)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper ──────────────────────────────────────────────────────
    /// Convenience: tokenize a line with RustLexer, not inside a comment.
    fn rust_tokens(line: &str) -> Vec<Token> {
        RustLexer.tokenize_line(line, false).0
    }

    // ── Basic number detection ──────────────────────────────────────
    #[test]
    fn plain_number_is_highlighted() {
        let tokens = rust_tokens("42");
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 2,
                kind: TokenKind::Number
            }
        );
    }

    #[test]
    fn number_surrounded_by_text() {
        // "abc 123 xyz" → Normal, Number, Normal
        let tokens = rust_tokens("abc 123 xyz");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::Normal);
        assert_eq!(
            tokens[1],
            Token {
                start: 4,
                len: 3,
                kind: TokenKind::Number
            }
        );
        assert_eq!(tokens[2].kind, TokenKind::Normal);
    }

    #[test]
    fn number_at_start_of_line() {
        let tokens = rust_tokens("99 bottles");
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 2,
                kind: TokenKind::Number
            }
        );
        assert_eq!(tokens[1].kind, TokenKind::Normal);
    }

    #[test]
    fn number_at_end_of_line() {
        let tokens = rust_tokens("count = 7");
        let last = tokens.last().unwrap();
        assert_eq!(
            last,
            &Token {
                start: 8,
                len: 1,
                kind: TokenKind::Number
            }
        );
    }

    // ── Word-boundary rule (the u16 corner case) ────────────────────
    #[test]
    fn digits_after_letter_are_not_number() {
        // "u16" should be entirely Normal — the "16" is part of an identifier.
        let tokens = rust_tokens("u16");
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 3,
                kind: TokenKind::Normal
            }
        );
    }

    #[test]
    fn digits_after_underscore_are_not_number() {
        let tokens = rust_tokens("var_2");
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 5,
                kind: TokenKind::Normal
            }
        );
    }

    #[test]
    fn type_names_with_digits_stay_normal() {
        for name in &["i32", "u64", "f32", "i128"] {
            let tokens = rust_tokens(name);
            assert_eq!(tokens.len(), 1, "{name} should produce a single token");
            assert_eq!(
                tokens[0].kind,
                TokenKind::Normal,
                "{name} should be Normal, not Number"
            );
        }
    }

    #[test]
    fn number_after_space_is_still_highlighted() {
        // "u16 = 42" → Normal("u16 = "), Number("42")
        let tokens = rust_tokens("u16 = 42");
        let last = tokens.last().unwrap();
        assert_eq!(
            last,
            &Token {
                start: 6,
                len: 2,
                kind: TokenKind::Number
            }
        );
    }

    #[test]
    fn number_after_paren_is_highlighted() {
        let tokens = rust_tokens("foo(42)");
        assert!(
            tokens
                .iter()
                .any(|t| t.kind == TokenKind::Number && t.start == 4 && t.len == 2),
            "42 inside parens should be a Number token"
        );
    }

    // ── String literals ─────────────────────────────────────────────
    #[test]
    fn plain_string_is_single_token() {
        let tokens = rust_tokens("\"hello\"");
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 7,
                kind: TokenKind::String
            }
        );
    }

    #[test]
    fn two_strings_separated_by_normal_text() {
        // `"hello" + "world"` → String, Normal(" + "), String
        let tokens = rust_tokens("\"hello\" + \"world\"");
        assert_eq!(tokens.len(), 3);
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 7,
                kind: TokenKind::String
            }
        );
        assert_eq!(
            tokens[1],
            Token {
                start: 7,
                len: 3,
                kind: TokenKind::Normal
            }
        );
        assert_eq!(
            tokens[2],
            Token {
                start: 10,
                len: 7,
                kind: TokenKind::String
            }
        );
    }

    #[test]
    fn string_assigned_to_variable() {
        // `let s = "hi";` → Normal("let s = "), String("\"hi\""), Normal(";")
        let tokens = rust_tokens("let s = \"hi\";");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::Normal);
        assert_eq!(
            tokens[1],
            Token {
                start: 8,
                len: 4,
                kind: TokenKind::String
            }
        );
        assert_eq!(
            tokens[2],
            Token {
                start: 12,
                len: 1,
                kind: TokenKind::Normal
            }
        );
    }

    #[test]
    fn empty_string_is_still_a_token() {
        let tokens = rust_tokens("\"\"");
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 2,
                kind: TokenKind::String
            }
        );
    }

    #[test]
    fn escaped_quote_does_not_end_string() {
        // Target text: "say \"hi\""  (one String token, quotes and all)
        let tokens = rust_tokens("\"say \\\"hi\\\"\"");
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 12,
                kind: TokenKind::String
            }
        );
    }

    #[test]
    fn escaped_backslash_does_not_escape_the_next_char() {
        // Target text: "back\\slash" — the escaped backslash must not also
        // consume the following char, and must not prevent the real
        // closing quote from being recognized.
        let tokens = rust_tokens("\"back\\\\slash\"");
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 13,
                kind: TokenKind::String
            }
        );
    }

    #[test]
    fn unterminated_string_falls_back_to_normal() {
        // No closing quote before end of line: per the multi-line-strings
        // deferral, the whole line stays Normal rather than being colored
        // as an (incorrectly) open-ended String.
        let tokens = rust_tokens("let s = \"unterminated");
        assert_eq!(
            tokens.len(),
            1,
            "no closing quote: whole line should stay one Normal run"
        );
        assert_eq!(tokens[0].kind, TokenKind::Normal);
        assert_eq!(tokens[0].len, 21);
    }

    #[test]
    fn digits_inside_string_are_not_split_into_a_number_token() {
        // Regression test for the single-pass, priority-ordered rewrite:
        // the string check must win over the number check, so "42" inside
        // a string is never split out as its own Number token.
        let tokens = rust_tokens("\"room 42\"");
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 9,
                kind: TokenKind::String
            }
        );
    }

    // ── Char literals ───────────────────────────────────────────────
    #[test]
    fn plain_char_literal_is_single_token() {
        let tokens = rust_tokens("'x'");
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 3,
                kind: TokenKind::String
            }
        );
    }

    #[test]
    fn digit_char_literal_is_not_split_into_a_number_token() {
        let tokens = rust_tokens("'0'");
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 3,
                kind: TokenKind::String
            }
        );
    }

    #[test]
    fn escaped_quote_char_literal() {
        // Target text: '\''  (char literal for a single quote)
        let tokens = rust_tokens("'\\''");
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 4,
                kind: TokenKind::String
            }
        );
    }

    #[test]
    fn escaped_backslash_char_literal() {
        // Target text: '\\'  (char literal for a backslash)
        let tokens = rust_tokens("'\\\\'");
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 4,
                kind: TokenKind::String
            }
        );
    }

    #[test]
    fn char_literal_assigned_to_variable() {
        let tokens = rust_tokens("let c = 'x';");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::Normal);
        assert_eq!(
            tokens[1],
            Token {
                start: 8,
                len: 3,
                kind: TokenKind::String
            }
        );
        assert_eq!(
            tokens[2],
            Token {
                start: 11,
                len: 1,
                kind: TokenKind::Normal
            }
        );
    }

    #[test]
    fn lifetime_reference_is_not_highlighted() {
        // `'a` is a lifetime, not a char literal: no closing `'` follows
        // immediately, so the whole line must stay Normal.
        let tokens = rust_tokens("&'a str");
        assert_eq!(
            tokens.len(),
            1,
            "lifetime should not produce a String token"
        );
        assert_eq!(tokens[0].kind, TokenKind::Normal);
        assert_eq!(tokens[0].len, 7);
    }

    #[test]
    fn lifetime_in_generic_bound_is_not_highlighted() {
        let tokens = rust_tokens("fn foo<'a>()");
        assert_eq!(
            tokens.len(),
            1,
            "lifetime should not produce a String token"
        );
        assert_eq!(tokens[0].kind, TokenKind::Normal);
        assert_eq!(tokens[0].len, 12);
    }

    #[test]
    fn char_literal_and_string_coexist_on_one_line() {
        let tokens = rust_tokens("\"a\" + 'x'");
        assert_eq!(tokens.len(), 3);
        assert_eq!(
            tokens[0],
            Token {
                start: 0,
                len: 3,
                kind: TokenKind::String
            }
        );
        assert_eq!(
            tokens[1],
            Token {
                start: 3,
                len: 3,
                kind: TokenKind::Normal
            }
        );
        assert_eq!(
            tokens[2],
            Token {
                start: 6,
                len: 3,
                kind: TokenKind::String
            }
        );
    }

    #[test]
    fn unterminated_char_literal_falls_back_to_normal() {
        let tokens = rust_tokens("let c = 'x");
        assert_eq!(
            tokens.len(),
            1,
            "no closing quote: whole line should stay one Normal run"
        );
        assert_eq!(tokens[0].kind, TokenKind::Normal);
        assert_eq!(tokens[0].len, 10);
    }

    // ── Edge cases ──────────────────────────────────────────────────
    #[test]
    fn empty_line_produces_no_tokens() {
        let tokens = rust_tokens("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn line_with_no_digits_is_all_normal() {
        let tokens = rust_tokens("let x = foo;");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Normal);
    }

    #[test]
    fn multiple_numbers_on_one_line() {
        let tokens = rust_tokens("1 + 2 + 3");
        let numbers: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Number)
            .collect();
        assert_eq!(numbers.len(), 3);
    }

    #[test]
    fn tokens_cover_entire_line_without_gaps() {
        let line = "let x: u16 = 42;";
        let tokens = rust_tokens(line);
        // Verify tokens tile the full line with no gaps or overlaps.
        let total_len: usize = tokens.iter().map(|t| t.len).sum();
        assert_eq!(
            total_len,
            line.len(),
            "tokens must cover exactly the whole line"
        );

        // Check contiguity.
        for window in tokens.windows(2) {
            assert_eq!(
                window[0].start + window[0].len,
                window[1].start,
                "gap between tokens at {} and {}",
                window[0].start,
                window[1].start
            );
        }
    }
}
