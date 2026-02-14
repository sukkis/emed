# emed

`emed` is a tiny terminal-based editor project I’m building to learn Rust and terminal UI programming (via `crossterm`). It’s a learning playground first, so expect rough edges and frequent changes.

## Status

Early prototype / work in progress. Not useful (yet).

## Goals

- Learn Rust by building something interactive
- Keep the codebase small and understandable

## Build & run

Prerequisites: a recent Rust toolchain.

## Controls

- `Ctrl+q` — quit
- `Ctrl+x` then `Ctrl+c` — quit (Emacs-style)

## Architecture

This project follows a small “model + view + event loop” structure. The goal is to keep the code easy to navigate while still being explicit about terminal details.

### High-level flow (read → translate → apply)

The main loop is intentionally split into three steps:

Event (crossterm) → EditorCommand → (EditorState + EditorUi)

1) **Read**: block for terminal input via `crossterm::event::read()`.
2) **Translate**: convert the raw `crossterm::Event` into an `EditorCommand`.
3) **Apply**: execute the `EditorCommand` by mutating `EditorState` and redrawing via `EditorUi`.

Keeping translation separate from execution makes the keybindings easy to change and keeps terminal-specific types from leaking everywhere.

### Modules

- `src/main.rs` — event loop, keybindings, command dispatch
- `src/lib.rs` — editor state (text buffer + cursor), editing operations
- `src/ui.rs` — terminal rendering and cursor movement (view)

### Core types (structs/enums)

- `EditorState` — owns the text buffer (currently a `ropey::Rope`) plus the cursor position (`cx`, `cy`)
- `EditorUi` — owns `stdout` and renders an `EditorState` to the terminal
- `EditorCommand` — a small “vocabulary” of editor actions (move, insert, quit, etc.)

### Input / event matching

Key presses are translated from `crossterm::Event` into `EditorCommand`. This is also where multi-key “chords” live.

Example: Emacs-style quitting uses a prefix key:

- press `Ctrl+X` to *arm* a prefix
- the next keypress is interpreted in that context:
    - `Ctrl+C` becomes `EditorCommand::Quit`
    - anything else cancels the prefix

This is tracked via a tiny state flag (`saw_ctrl_x`) that persists across events.

### Rendering model

Rendering is currently “full screen redraw”:

- `EditorUi::draw_screen()` clears the screen and draws the visible buffer.
- Empty rows are filled with `~` (Vim-style) to make it obvious where the file content ends.
- After drawing, the terminal cursor is moved to match `EditorState`’s cursor position.

### Future work (short)

- Text editing: backspace/delete, newline handling, tabs
- Scrolling / viewport (so cursor can move beyond the visible screen)
- Status line / message area
- File I/O (open/save) and a proper startup file argument
- More keybindings and tests for input translation

## License

GNU General Public License v3.0 (GPL-3.0).
See `LICENSE` for details.