---
title: "Clipboard decode accepts a column index one past the last visible column"
related-mental-model:
  - clipboard-tsv
---

# Clipboard decode accepts a column index one past the last visible column

## Background

`try_update_clipboard_from_string` rejects a decoded column with
`if col_idx > self.p.vis_cols.len()` (search `If the column is out of range` in
`src/draw/state/clipboard.rs`). Valid indices are `0..vis_cols.len()`, so the
check should be `>=`: `col_idx == vis_cols.len()` passes and is handed straight
to `codec.decode_column`, and later to `set_cell_value` / `is_editable_cell`
through the resulting `SetCells` command.

The earlier guard in the same function checks the raw pasted table width against
the visible column count, but not width plus the selection offset — so a paste
whose selection starts near the last column drives `col_idx` to exactly
`vis_cols.len()`. Whether this panics or silently writes nowhere depends on the
consumer's `RowViewer` implementation; a viewer matching exhaustively on column
index will panic.

Found while forging `ai-docs/mental-model/clipboard-tsv.md`; confirmed by reading
the guard and the offset arithmetic, not by running a repro.

## Decisions

- Fix the comparison rather than clamping the index. The function's stated intent
  is to reject out-of-range data ("we'll just ignore it"), and clamping would
  silently write pasted content into the wrong column.

## Constraints

- The two guards must agree: whichever one is authoritative, the combination of
  pasted width and selection offset has to be bounded, not just the width. Fixing
  only the inner comparison leaves an inconsistent pair.
- The whole function returns `false` on any rejection, and the caller discards
  that value (see `research-clipboard-source-precedence`). Do not build the fix
  on the assumption that a `false` return prevents the paste.

## Phases

### Phase 1: Bound the decoded column index correctly

Correct the comparison, make the width guard account for the selection offset,
and add tests for a paste landing exactly at and one past the last visible
column. `clipboard.rs` currently has no test coverage at all.
