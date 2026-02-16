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

| File              | Responsibility                                                               |
|-------------------|------------------------------------------------------------------------------|
| `src/main.rs`     | Event loop, keybindings, command dispatch, file I/O, prompt handling         |
| `src/lib.rs`      | Editor state (text buffer + cursor), editing operations, file type detection |
| `src/ui.rs`       | Terminal rendering, status bar, cursor movement (view)                       |
| `src/settings.rs` | Configuration loading from TOML with fallback defaults                       |
| `src/theme.rs`    | Color theme definitions and named color abstraction                          |

## Core types

- **`EditorState`** — owns the text buffer (`ropey::Rope`), cursor position, scroll offsets, filename, file type, and
  prompt state
- **`EditorUi`** — owns `stdout` and a `Theme`; renders an `EditorState` to the terminal
- **`EditorCommand`** — a small vocabulary of editor actions (move, insert, save, quit, …)
- **`InputKey`** — a simplified, backend-agnostic representation of a keypress
- **`ApplyResult`** — return value from applying a command (`NoChange`, `Changed`, `Quit`)
- **`Theme`** — a set of named colours for foreground, background, status bar, and tilde lines
- **`ThemeColor`** — human-readable colour names that map to `crossterm::style::Color`

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