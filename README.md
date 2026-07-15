# emed

A tiny terminal-based text editor, built to learn Rust and terminal UI programming.

Early prototype — expect rough edges and frequent changes.

## Controls

- Arrow keys — move cursor
- `Ctrl+q` — quit
- `Ctrl+x` then `Ctrl+c` — quit (Emacs-style)
- `Ctrl+x` then `Ctrl+s` — save file (prompts for filename if unknown)
- `Ctrl+g` — cancel prompt, or cancel an in-progress search (restores cursor)
- `Ctrl+s` — start incremental search forward; `Ctrl+r` — start incremental search backward.
  While searching, type to refine, `Ctrl+s`/`Ctrl+r` to jump to the next/previous match
  (wrapping, and flipping direction if you switch keys mid-search), `Enter` to accept
- `Ctrl+c` then `l` — toggle soft line wrap (`visual_line_mode`); wrapped lines break at word boundaries and cursor movement follows the wrapped rows
- Typing, Enter, Backspace, Delete — edit text as expected

## Dependencies

| Crate                                                   | Purpose                                                           |
|---------------------------------------------------------|-------------------------------------------------------------------|
| [crossterm](https://crates.io/crates/crossterm)         | Terminal I/O: raw mode, key events, cursor control, styled output |
| [ropey](https://crates.io/crates/ropey)                 | Rope data structure for the text buffer                           |
| [clap](https://crates.io/crates/clap)                   | Command-line argument parsing                                     |
| [config](https://crates.io/crates/config)               | Configuration file loading with defaults                          |
| [unicode-width](https://crates.io/crates/unicode-width) | Display-width calculation for Unicode characters and tabs         |

## Configuration

Copy the example config and edit to taste:

`cp settings.toml.example settings.toml`

Available settings:

| Key         | Default  | Description                         |
|-------------|----------|-------------------------------------|
| `theme`     | `"pink"` | Color theme — `"pink"` or `"ocean"` |
| `tab_width` | `"4"`    | Tab display width in columns        |

## Architecture

See [architecture.md](architecture.md) for design notes, module layout, and internal details.

## Roadmap (kilo feature parity)

- [x] Basic editing (insert, delete, backspace, newline)
- [x] Cursor movement (arrow keys)
- [x] Vertical scrolling
- [x] Horizontal scrolling
- [x] Status bar (filename, line count, cursor position, dirty flag)
- [x] File I/O (open, save, "Save as" prompt)
- [x] Quit confirmation for unsaved changes
- [x] Tab rendering (configurable width)
- [x] Unicode display-width support
- [x] Incremental search (find), forward only
- [x] Reverse incremental search (`C-r`) — jump back through earlier matches
- [x] Syntax highlighting (number literals only; word-boundary aware) — first slice
- [ ] Syntax highlighting for Rust (kilo highlights C; Rust is the equivalent language
      here) — string literals, char literals, line comments, keywords, and primitive
      types done; still pending: std/user-defined types, traits, block comments
      (`/* */`, including multi-line + nesting), and operators

Extras

- [x] Colour themes support
- [x] Configurable tab width
- [x] Panic-safe terminal cleanup
- [x] Soft line wrapping (`visual_line_mode`, word-wrap, toggled with `C-c l`)
- [x] Syntax highlighting in wrapped mode (same token coloring as unwrapped)
- [ ] Indent-aware wrap prefix for soft-wrapped lines
- [ ] Syntax highlighting for Markdown
- [ ] Syntax highlighting of search matches
- [ ] Cycle to next theme with a keybinding (e.g. `C-c t`, Emacs-style) — needs a design
      decision first: theme currently lives on `EditorUi`, not `EditorState`, so a
      command-driven toggle needs somewhere testable to track "current theme"

## License

GNU General Public License v3.0 (GPL-3.0).
See `LICENSE` for details.