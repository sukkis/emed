# emed

`emed` is a tiny terminal-based editor project I’m building to learn Rust and terminal UI programming (via `crossterm`). 
It’s a learning playground first, so expect rough edges and frequent changes.

## Status

Early prototype / work in progress. 

Basic editing, scrolling, file I/O, and a status bar are functional.

## Goals

- Learn Rust by building something interactive
- Keep the codebase small and understandable

## Build & run

Prerequisites: a recent Rust toolchain.

## Controls

- Arrow keys — move cursor
- `Ctrl+q` — quit
- `Ctrl+x` then `Ctrl+c` — quit (Emacs-style)
- `Ctrl+x` then `Ctrl+s` — save file (prompts for filename if unknown)
- `Ctrl+g` — cancel prompt (e.g. "Save as")
- Typing, Enter, Backspace, Delete — edit text as expected

This project follows a small "model + view + event loop" structure. The goal is to keep the code easy to navigate while still being explicit about terminal details.

### High-level flow (read → translate → apply)

The main loop is intentionally split into three steps:

Event (crossterm) → EditorCommand → (EditorState + EditorUi)

1) **Read**: block for terminal input via `crossterm::event::read()`.
2) **Translate**: convert the raw `crossterm::Event` into an `EditorCommand`.
3) **Apply**: execute the `EditorCommand` by mutating `EditorState` and redrawing via `EditorUi`.

Keeping translation separate from execution makes the keybindings easy to change and keeps terminal-specific types from leaking everywhere.

When the editor is in **prompt mode** (e.g. "Save as"), keypresses are routed to a prompt handler instead of the normal command pipeline. The prompt state is tracked via `EditorState.prompt_buffer`.

### Modules

- `src/main.rs` — event loop, keybindings, command dispatch, file I/O, prompt handling
- `src/lib.rs` — editor state (text buffer + cursor), editing operations, file type detection
- `src/ui.rs` — terminal rendering, status bar, and cursor movement (view)

### Core types (structs/enums)

- `EditorState` — owns the text buffer (`ropey::Rope`), cursor position, scroll offset, filename, file type, and prompt state
- `EditorUi` — owns `stdout` and renders an `EditorState` to the terminal
- `EditorCommand` — a small "vocabulary" of editor actions (move, insert, save, quit, etc.)
- `InputKey` — a simplified, backend-agnostic representation of a keypress
- `EditorMode` — tracks whether the editor is in normal editing or prompt input mode

### Input / event matching

Key presses are translated from `crossterm::Event` into `EditorCommand`. This is also where multi-key "chords" live.

The `Ctrl+X` prefix arms a flag (`saw_ctrl_x`); the next keypress is interpreted in that context:

- `Ctrl+C` → `Quit`
- `Ctrl+S` → `SaveFile`
- anything else → cancels the prefix

### Rendering model

Rendering is currently "full screen redraw":

- `EditorUi::draw_screen()` clears the screen and draws the visible buffer.
- Empty rows are filled with `~` (Vim-style) to make it obvious where the file content ends.
- The bottom two rows are reserved: a reverse-video **status bar** (file type, line/char counts) and a **help/message line** (keybinding hints, "File saved", or the "Save as" prompt).
- After drawing, the terminal cursor is moved to match `EditorState`'s cursor position.

### Scrolling

The editor supports vertical scrolling. `EditorState` tracks `row_offset` (the first buffer line visible at the top of the screen). When the cursor moves off-screen, `ensure_cursor_visible()` adjusts the offset so the viewport follows.

### Future work (short)

- Tabs / indentation
- Search / find
- Syntax highlighting
- "Dirty" flag / unsaved-changes warning on quit


## License

GNU General Public License v3.0 (GPL-3.0).
See `LICENSE` for details.