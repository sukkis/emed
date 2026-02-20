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
    _String,
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
    /// Byte offset of the first character within the line.
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

/// Is `chars[i]` the start of a standalone number literal?
///
/// A digit counts as a number only if it's NOT preceded by a letter or underscore.
/// This prevents highlighting the "16" in "u16" or "32" in "my_var32".
fn is_number_start(chars: &[char], i: usize) -> bool {
    chars[i].is_ascii_digit()
        && (i == 0 || !(chars[i - 1].is_ascii_alphanumeric() || chars[i - 1] == '_'))
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
        // For now, Rust only highlights numbers.
        // Later: call tokenize_numbers, then refine Normal spans
        // into keywords, types, strings, comments, etc.
        (tokenize_numbers(line), false)
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
