# emed

A tiny terminal-based text editor, built to learn Rust and terminal UI programming.

Early prototype — expect rough edges and frequent changes.

## Controls

- Arrow keys — move cursor
- `Ctrl+q` — quit
- `Ctrl+x` then `Ctrl+c` — quit (Emacs-style)
- `Ctrl+x` then `Ctrl+s` — save file (prompts for filename if unknown)
- `Ctrl+g` — cancel prompt
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
- [ ] Incremental search (find)
- [ ] Syntax highlighting

Extras

- [x] Colour themes support
- [x] Configurable tab width

## License

GNU General Public License v3.0 (GPL-3.0).
See `LICENSE` for details.