# Architecture

This document describes the internal structure of `emed` for anyone reading or contributing to the code.

## High-level flow (read → translate → apply)

The main loop is split into three steps:

1. **Read** — block for terminal input via `crossterm::event::read()`.
2. **Translate** — convert the raw `crossterm::Event` into an `EditorCommand`.
3. **Apply** — execute the command by mutating `EditorState` and redrawing via `EditorUi`.

Keeping translation separate from execution makes keybindings easy to change and keeps
terminal-specific types from leaking into the core.

When the editor is in **prompt mode** (e.g. "Save as"), keypresses are routed to a prompt
handler instead of the normal command pipeline. The prompt state is tracked via
`EditorState.prompt_buffer`.

The same applies to **search mode**: while `EditorState.is_searching()` is true, keypresses
are routed to `handle_search_key` instead. One exception: keys that lead toward quitting or
saving (`Ctrl+Q`, `Ctrl+X`) are checked first via `escapes_search` — if matched, the search is
cancelled and the key falls through to the normal pipeline instead, so quitting (or saving)
is never unreachable just because a search happens to be open.

## Modules

| File              | Responsibility                                                               |
|-------------------|------------------------------------------------------------------------------|
| `src/main.rs`     | Event loop, keybindings, command dispatch, file I/O, prompt handling         |
| `src/lib.rs`      | Editor state (text buffer + cursor), editing operations, file type detection |
| `src/ui.rs`       | Terminal rendering, status bar, cursor movement (view)                       |
| `src/settings.rs` | Configuration loading from TOML with fallback defaults                       |
| `src/theme.rs`    | Color theme definitions and named color abstraction                          |
| `src/lexer.rs`    | Syntax highlighting: lexer trait, per-language lexers                        |
| `src/search.rs`   | Incremental search: pure `find_from` algorithm and `SearchSession` state     |

## Core types

- **`EditorState`** — owns the text buffer (`ropey::Rope`), cursor position, scroll offsets, filename, file type,
  prompt state, and an optional in-progress search (`search: Option<SearchSession>`)
- **`EditorUi`** — owns `stdout` and a `Theme`; renders an `EditorState` to the terminal
- **`EditorCommand`** — a small vocabulary of editor actions (move, insert, save, quit, start search, …)
- **`InputKey`** — a simplified, backend-agnostic representation of a keypress
- **`ApplyResult`** — return value from applying a command (`NoChange`, `Changed`, `Quit`)
- **`Theme`** — a set of named colours for foreground, background, status bar, and tilde lines
- **`ThemeColor`** — human-readable colour names that map to `crossterm::style::Color`
- **`Lexer`** (trait) — turns a single line into a sequence of `Token`s; one impl per language
- **`Token`** — a coloured span within a line: byte offset, length, and `TokenKind`
- **`TokenKind`** — the category of a token (`Normal`, `Number`, `Comment`, `Operator`, …)
- **`SearchSession`** — bookkeeping for an in-progress incremental search: the query typed so
  far and the char index the cursor started at; knows nothing about `EditorState` or cursors

## Input / event matching

Key presses are translated from `crossterm::Event` → `InputKey` → `EditorCommand`.

The `Ctrl+X` prefix arms a flag (`saw_ctrl_x`); the next keypress completes the chord:

- `Ctrl+C` → `Quit`
- `Ctrl+S` → `SaveFile`
- anything else → cancels the prefix

Outside of the `Ctrl+X` prefix, plain `Ctrl+S` → `StartSearch`.

## Rendering model

Full-screen redraw every frame (simple + robust):

- `EditorUi::draw_screen()` clears and repaints the entire terminal.
- Empty rows show `~` (Vim-style) to mark the end of file content.
- The bottom two rows are reserved: a reverse-video **status bar** and a **help/message line**.
- After drawing, the terminal cursor is positioned to match `EditorState`'s cursor.

## Scrolling

The editor supports both vertical and horizontal scrolling.

- **Vertical:** `row_offset` tracks the first buffer line visible at the top of the screen.
- **Horizontal:** `col_offset` tracks the first screen column visible at the left edge.

When the cursor moves off-screen, `ensure_cursor_visible()` adjusts both offsets so the
viewport follows.

### Tab handling

Tab characters are expanded to spaces for rendering. The tab width defaults to 4 columns
and is configurable via `tab_width` in `settings.toml`. The value is stored in
`EditorState.tab_width` and used by `display_width()` for all width calculations.
Display-width calculations use `unicode-width` for regular characters. When a tab is too
wide to fit the remaining visible columns, the line is truncated at that point.

## Configuration & theming

Settings are loaded at startup from `settings.toml` in the working directory (if present).
The `config` crate handles parsing and merging with built-in defaults, so missing keys are
always safe.

Currently supported settings:

- **`theme`** — selects a built-in colour theme (`"pink"` or `"ocean"`). Unknown names
  fall back to `"pink"`.
- **`tab_width`** — tab display width in columns (default: 4).

Themes are defined in `src/theme.rs`. Each theme specifies foreground, background, status-bar,
and tilde-line colours using `ThemeColor`, which wraps `crossterm::style::Color` behind
readable names. Adding a new theme means adding a constructor to `Theme` and a match arm in
`Theme::from_name()`.

## Syntax highlighting

Syntax highlighting is implemented as a simple per-line lexer pipeline:

1. **Lexer selection** — when a file is loaded, `load_document()` picks a lexer based on file
   extension (`RustLexer` for `.rs`, `PlainLexer` for everything else). A fresh buffer with
   no file also gets a `PlainLexer` so that number literals are highlighted immediately.

2. **Tokenization** — each `Lexer` implements `tokenize_line(line, in_comment) → (Vec<Token>, bool)`.
   `RustLexer` scans a line once, char by char, checking "does a token start here?" in
   priority order at each position (string start before number start) and consuming the
   whole token in one bite when matched — rather than running a baseline pass over the whole
   line and refining it afterward. This matters because refine-after-the-fact can't be undone
   cleanly: e.g. digits inside a string literal must never become a separate `Number` token,
   which a baseline numbers-first pass would get wrong. Shared rules (number-literal detection
   with word-boundary awareness, string-literal boundary detection with backslash-escapes) live
   in free functions (`is_number_start`, `find_string_end`) called from within that scan.
   `PlainLexer` still just calls `tokenize_numbers()` (no strings). See
   `docs/rust-highlighting.md` for the full design rationale and increment plan.

3. **Caching** — `EditorState` maintains a `token_cache: Vec<Vec<Token>>` with one entry per
   line. `tokens_for_line(i)` tokenizes on first access and returns the cached result.
   The entire cache is invalidated on every edit (via `set_dirty() → invalidate_tokens()`).

4. **Rendering** — `draw_screen()` walks each visible character, looks up which token it
   belongs to, and sets the foreground colour accordingly (e.g. `number_fg` for `Number`
   tokens, `string_fg` for `String` tokens). Characters that don't match any token fall back
   to the theme's default foreground.

### Word-boundary rule

A digit is only classified as a `Number` token if it is **not** preceded by a letter or
underscore. This prevents the "16" in `u16` or the "32" in `my_var32` from being highlighted
as numbers — they're part of an identifier. Standalone literals like `42`, `(123)`, and
`x + 7` are highlighted correctly.

### String literals (Rust only, single-line)

A `"` starts a string token; `find_string_end` scans forward for the matching closing `"`,
treating `\` as always consuming itself plus the next character (so `\"` and `\\` are handled
correctly without needing to know Rust's actual escape-sequence set). If no closing quote is
found before end of line, the opening `"` is treated as ordinary text instead of coloring the
rest of the line as an incorrectly open-ended string — multi-line strings aren't supported yet
(see `docs/rust-highlighting.md`).

### Char literals (Rust only, reuse `TokenKind::String`)

A `'` starts a char-literal check via `find_char_literal_end`, which only matches the fixed,
narrow shape a char literal actually has: one plain character, or one backslash-escaped
character, immediately followed by a closing `'`. Char literals render with the same
`TokenKind::String` as strings rather than a separate kind. This fixed-length shape is also
what disambiguates a char literal from a lifetime (`'a`, `'static`) without needing to
understand identifiers at all — a lifetime is never followed by a bare `'`, so it simply never
matches and the `'` is left as ordinary text, same as an unterminated string. Unicode escapes
(`'\u{1F600}'`) are out of scope, since they aren't fixed-length (see
`docs/rust-highlighting.md`).

### Line comments (Rust only)

A `//` starts a `Comment` token via `is_comment_start`, which consumes everything from there to
end of line in one bite — no closing delimiter to search for, no escapes, `///` and `//!` need
no special-casing (they still start with `//`; the extra character is just more comment text).
Block comments (`/* */`, including Rust's nesting and the multi-line carry-state that requires)
are a separate, later increment (see `docs/rust-highlighting.md`).

### Keywords and primitive types (Rust only)

`scan_word` scans the *whole* identifier-shaped word at a position (letters, digits,
underscores, left-boundary-checked the same way `is_number_start` checks digits) and returns
its text — checking the full word first, rather than matching a prefix, is what keeps
"structure" from being misread as containing "struct", or "boolean" as containing "bool".
`find_keyword_end` and `find_type_end` both call `scan_word` and differ only in which list they
check the result against: `KEYWORDS` (alphabetically sorted, so a human can scan and confirm a
word is or isn't in it) or `PRIMITIVE_TYPES` (kept in Rust's own conventional bit-width order —
`i8, i16, i32, i64, i128, isize, …` — since that's more human-scannable than strict alphabetical
for this particular list). A word matching neither isn't treated as a token start at all, so
it's absorbed into the surrounding `Normal` run exactly as before either increment — no
fragmentation cost for ordinary identifiers. `KEYWORDS` deliberately excludes primitive/std type
names and unused-but-reserved words (`abstract`, `become`, …); `true`/`false` are included,
matching Rust's own grammar, which classifies them as keywords rather than a separate literal
kind. `PRIMITIVE_TYPES` covers exactly the fixed, exhaustive set of built-in type names
(`i8`…`i128`, `u8`…`u128`, `isize`, `usize`, `f32`, `f64`, `bool`, `char`, `str`) — std types
(`String`, `Vec`, `Option`, …) and user-defined types are separate, later increments, since an
exhaustive list stops being possible once user-defined types are in scope (see
`docs/rust-highlighting.md`).

### Adding a new language

1. Create a new struct (e.g. `CLexer`) in `lexer.rs`.
2. Implement the `Lexer` trait as a single char-by-char scan, checking token-start conditions
   in priority order at each position (see `RustLexer::tokenize_line`), rather than a baseline
   pass over the whole line refined afterward.
3. Add a match arm in `lexer_for_file_type()`.
4. Add the file extension in `file_type_from_filename()` in `lib.rs`.

## Incremental search

Search is built as three layers, each with one job. It's direction-aware throughout — one
`Direction { Forward, Backward }` enum (`search.rs`), threaded down from `EditorCommand` to
the pure algorithm, rather than separate forward/backward code paths:

1. **`find_from(haystack, needle, start, wrap, direction)`** (`search.rs`) — the pure
   algorithm. Works in char indices (not byte offsets), so callers never have to think about
   UTF-8. An empty needle never matches (Emacs behaviour: an empty query doesn't move point).
   `Forward` searches `haystack[start..]` (`.find`, first match); `Backward` searches
   `haystack[..start]` (`.rfind`, last match whose *end* is `<= start`) — symmetric slicing,
   same wraparound trick in both directions (searching the whole string is safe once the
   half-string search has already come up empty).

2. **`SearchSession`** (`search.rs`) — bookkeeping for one in-progress search: the `query`
   typed so far, the `origin` char index the cursor was at when the search began, the current
   `direction`, and `found` (whether the last match attempt succeeded, for the "Failing
   I-search" indicator — stored rather than recomputed live, since a live recompute from
   `query` alone would be wrong right after a wrapped `repeat`). It exposes two different
   search policies, deliberately kept apart:
   - `current_match(haystack)` — searches from `origin` in the session's current `direction`,
     **no wraparound**. Used as you type or backspace, so the match "grows" from where you
     started rather than drifting. Never changes `direction` itself.
   - `repeat(haystack, after, direction)` — searches from `after` (± 1, direction-dependent —
     never re-reporting the match already under the cursor) in the *given* `direction`, **with
     wraparound**, and records that direction as the session's new one. This is what lets
     `C-s`/`C-r` flip an active session's direction mid-search, stepping from the current
     position rather than jumping back to `origin`.

   Both policies live here rather than in `EditorState`, because `origin`/`direction`/`found`
   are private fields — this is the only way other modules can get an answer without reaching
   into `SearchSession`'s internals.

3. **`EditorState` driver methods** — `search_start(direction)`, `search_push_char`,
   `search_backspace`, `search_repeat(direction)`, `search_accept`, `search_cancel`,
   `is_searching`, `search_query`, `is_search_failing`, `is_search_backward`. These convert a
   found char index into a `(cx, cy)` cursor position (`char_index_to_cursor`) and move the
   cursor there; on no match, the cursor is left exactly where it was. `search_cancel` restores
   the cursor to `origin`; `search_accept` just ends the session, leaving the cursor at the
   match.

`EditorCommand::StartSearch(Direction)` carries the direction from `command_from_key` — plain
`Ctrl+s` produces `Forward`, plain `Ctrl+r` produces `Backward` (cold-start `Ctrl+r` begins a
session already searching backward, matching real Emacs' `isearch-backward`). `main.rs`'s
`handle_search_key` maps mid-search keys to the driver methods above and is the only untested
layer — it's pure dispatch to methods that are already covered by tests, and can't
meaningfully be unit tested itself (it needs a real `crossterm::Event`), matching how
`handle_prompt_key` already works.

The help line at the bottom of the screen shows the query while searching
(`EditorState::status_help_line`), with priority: "Save as" prompt, then active search query,
then the default help message. The search-query line itself composes two independent optional
fragments around "I-search": a `"Failing "` prefix when `is_search_failing()` (never shown for
an empty query, regardless of `found`'s stored value — matches real Emacs, which shows plain
`I-search:` immediately after `C-s`/`C-r`), and a `" backward"` suffix when
`is_search_backward()` — e.g. `"Failing I-search backward: xyz"`.

## Soft line wrapping (`visual_line_mode`)

Toggled with `C-c l` (`EditorCommand::ToggleVisualLineMode`, handled identically — two
independent, exhaustive matches — in both `main.rs::apply_command`, the real event loop, and
`EditorState::apply_command`, which exists only so tests can drive it). Off by default,
configurable via `settings.toml`.

Built in layers, each a pure function of the buffer plus a width, so the whole thing is
unit-testable without a terminal:

1. **`wrapped_lines(line_index, width)`** — word-wrap for a single buffer line into a
   `Vec<String>` of chunks. Breaks at the nearest preceding space (the space stays attached to
   the end of the earlier chunk); hard-breaks a single word longer than `width` with nothing
   better to back up to. Guards `width == 0` to avoid an infinite loop.
2. **`wrapped_screen_rows(height, width)`** — composes `wrapped_lines` across every buffer line
   from `row_offset` into the flat list of screen rows `draw_screen` paints, as `WrappedRow {
   line_index, start_col, text }` (`src/wrap.rs`). Carrying `line_index`/`start_col` (not just
   the chunk text) is what lets `draw_screen` reconstruct each character's buffer column —
   `start_col + char_idx` — and look up its syntax-highlight token the same way the unwrapped
   path does with `col_offset + char_idx`. A blank line is still 1 row, not 0 (otherwise
   everything below it would shift up). Known limitation: if a line's chunks don't fully fit in
   the remaining rows, the rest are clipped — `row_offset` is a buffer-line index, not a
   visual-row index.
3. **Buffer ↔ screen position mapping**, needed because the terminal cursor is a screen
   position but `EditorState` tracks a buffer position (`cx`, `cy`):
   - `screen_rows_before_line(line_index, width)` — the Y half: how many wrapped rows the lines
     from `row_offset` up to `line_index` occupy.
   - `wrapped_cursor_offset(line_index, cx, width)` — the X half: which wrapped chunk `cx` falls
     in, and the column within it. A `cx` sitting exactly on a chunk boundary belongs to the
     *start* of the next chunk, not the end of the previous one.
   - `char_offset_for_col` / `chars_before_chunk` (private) — the inverse of the above, used to
     turn a wrapped-row Up/Down move back into a buffer `cx`.
   Both directions are exercised by `draw_screen`'s cursor placement and by
   `cursor_up`/`cursor_down`, which move by wrapped chunk instead of whole buffer line when
   `visual_line_mode` is on (no "goal column" memory across repeated moves — matches the
   existing plain `cursor_up`/`cursor_down`, which don't track one either).
4. **Status bar** — `status_line()` appends a `(wrap)` tag when `visual_line_mode` is on, using
   the same "only shown when true" idiom as the `(modified)` tag.

Deliberately out of scope for now: an indent-aware wrap prefix for continuation lines (matching
the line's own leading whitespace, à la Emacs 30's `visual-wrap-prefix-mode`) — this would
require every mapping function above to account for a narrower usable width and a column offset
on continuation chunks, so it's planned as a separate, later config toggle rather than bolted on
here.

## Terminal safety

The main function wraps the editor loop in `std::panic::catch_unwind` so that
`EditorUi::clean_up()` always runs — even on panics. This restores the terminal from raw
mode and prevents the user from being stranded in an unusable terminal session.