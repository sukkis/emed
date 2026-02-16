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

## Modules

| File          | Responsibility                                                               |
|---------------|------------------------------------------------------------------------------|
| `src/main.rs` | Event loop, keybindings, command dispatch, file I/O, prompt handling         |
| `src/lib.rs`  | Editor state (text buffer + cursor), editing operations, file type detection |
| `src/ui.rs`   | Terminal rendering, status bar, cursor movement (view)                       |

## Core types

- **`EditorState`** — owns the text buffer (`ropey::Rope`), cursor position, scroll offsets, filename, file type, and
  prompt state
- **`EditorUi`** — owns `stdout` and renders an `EditorState` to the terminal
- **`EditorCommand`** — a small vocabulary of editor actions (move, insert, save, quit, …)
- **`InputKey`** — a simplified, backend-agnostic representation of a keypress
- **`ApplyResult`** — return value from applying a command (`NoChange`, `Changed`, `Quit`)

## Input / event matching

Key presses are translated from `crossterm::Event` → `InputKey` → `EditorCommand`.

The `Ctrl+X` prefix arms a flag (`saw_ctrl_x`); the next keypress completes the chord:

- `Ctrl+C` → `Quit`
- `Ctrl+S` → `SaveFile`
- anything else → cancels the prefix

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

Tab characters are expanded to spaces for rendering (fixed at 4 columns, defined by `TAB_WIDTH`).
Display-width calculations use `unicode-width` for regular characters and the fixed tab width
for `\t`. When a tab is too wide to fit the remaining visible columns, the line is truncated
at that point.