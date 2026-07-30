---
title: "max_undo_history has no way to express a disabled undo history"
related-mental-model:
  - public-api
  - command-undo
---

# max_undo_history has no way to express a disabled undo history

## Background

`Style::max_undo_history` is remapped at the point of use: zero means "use the
built-in default of 100" (search `max_undo_history == 0` in `src/draw/body.rs`).
There is no value that means "no undo history".

The underlying trim in `push_new_command` does not disable anything at zero
either — `capacity.saturating_sub(1)` keeps zero old entries but the
just-applied command is still pushed, leaving exactly one undoable entry.

So a consumer that deliberately sets `0` to disable undo silently gets the
deepest history the crate offers. Found while forging
`ai-docs/mental-model/public-api.md`.

## Decisions

- The zero-means-default overload is the defect. `Style` derives `Default`, so
  "unset" is already expressible without stealing a meaningful value.

## Constraints

- Changing what `0` means is a silent behavior change for anyone currently
  relying on it as "default" — most likely by copying it out of a struct literal.
  Weigh a compatible option (`Option<usize>` or a named constructor) against the
  breaking one, and record the choice; a breaking change here needs a CHANGELOG
  entry, not just a Result note.
- `Style` is `#[non_exhaustive]`, so adding a field is not itself breaking.
- If a truly disabled history is supported, `undo()`/`redo()` must stay no-ops
  rather than panicking, and the auto-commit recursion in `push_new_command`
  must still work when nothing is retained.

## Phases

### Phase 1: Give undo capacity an unambiguous encoding

Replace the zero overload with an encoding that can express default, an explicit
depth, and disabled. Verify the boundary cases: disabled, capacity one, and the
existing trim behavior at capacity N — the current test only asserts an upper
bound on the queue length.
