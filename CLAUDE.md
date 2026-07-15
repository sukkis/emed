# emed — Learning-Focused AI Instructions

## Purpose
emed exists so the user learns Rust and terminal UI programming — not to
ship features quickly. The measure of success for any session is whether
the user understands every line that got written, not how much roadmap
got covered.

Inherit all general standards from the parent `CLAUDE.md` (simplicity
first, dependency discipline, git branching, TDD). The rules below are
additive and specific to how we work in *this* project.

## Pace: Human Speed, Not Machine Speed
- Work in the smallest increment that is still a coherent step — one
  test, one function, one concept. Not a whole feature in one pass.
- A roadmap item may reasonably span several sessions. That is correct,
  not a failure to be efficient.
- **Hard rule:** after explaining a completed increment, stop and wait
  for the user's go-ahead before starting the next one — do not treat
  "I explained it" as license to keep going in the same turn. The
  parent `CLAUDE.md`'s requirement to stop after phase 1 (failing test)
  is one instance of this; it applies at every increment boundary, not
  only there.
- **An increment isn't finished until the docs it affects say the same
  thing the code now does.** This includes doc comments (e.g. a struct
  field comment that described the old behavior), `architecture.md`,
  `README.md`, and any `docs/*.md` design doc the increment was scoped
  from. If the increment changed an API, a design decision, or introduced
  a known shortcut/gap, that update happens in the same increment, not
  filed away as future cleanup — stale docs are exactly the kind of thing
  that makes a future session reconstruct the wrong *why*.

## Explain and Discuss
- Before introducing anything new to the codebase — a crate, a Rust
  pattern not yet used, a data structure — explain it briefly and why
  it's the right tool, before writing the code.
- After writing code, walk through what it does, especially anything
  Rust-specific that isn't obvious at a glance: ownership, borrowing,
  lifetimes, trait bounds. These are the actual point of the project.
- When there's a real design decision (e.g. rope vs. gap buffer, how
  ownership of the buffer should work, error-handling strategy), raise
  it as a discussion with options — don't silently pick one and move on.
  This can take a whole session with little or no code landing, if
  that's what understanding it requires.

## What Not to Do
- Don't skip explanation because the code is short or "self-explanatory."
  Readable code still hides *why*, and *why* is what's being learned.

## Suggested Rhythm
1. Pick the next roadmap item (see `README.md`) and agree on the
   smallest useful slice of it.
2. Write the failing test only. Stop. Explain what it checks and why.
3. Wait for the user to run it and confirm the failure.
4. Implement minimally. Stop. Walk through the implementation.
5. Run the full suite, confirm green. Update any doc comments,
   `architecture.md`, `README.md`, or `docs/*.md` entries this increment
   made stale or incomplete.
6. Stop. Discuss what's next, and what was deferred and why, before
   starting another increment.
