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

impl Lexer for RustLexer {
    fn tokenize_line(&self, line: &str, _in_comment: bool) -> (Vec<Token>, bool) {
        // Everything is Normal — no highlighting at all.
        let tokens = vec![Token {
            start: 0,
            len: line.len(),
            kind: TokenKind::Normal,
        }];
        (tokens, false)
    }
}

impl Lexer for PlainLexer {
    fn tokenize_line(&self, line: &str, _in_comment: bool) -> (Vec<Token>, bool) {
        // Everything is Normal — no highlighting at all.
        let tokens = vec![Token {
            start: 0,
            len: line.len(),
            kind: TokenKind::Normal,
        }];
        (tokens, false)
    }
}
