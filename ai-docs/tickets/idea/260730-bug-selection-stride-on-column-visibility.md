---
title: "Selection and interactive cell decode with the wrong column stride after a visibility change"
related-mental-model:
  - table-state
---

# Selection and interactive cell decode with the wrong column stride after a visibility change

## Background

`VisLinearIdx` flattens a cell position as `row * ncol + col`. Everywhere in the
crate the stride `ncol` is the **visible** column count (`p.vis_cols.len()`) —
`collect_selection`, `collect_selected_rows`, `moved_position`,
`set_interactive_cell`, the clipboard paths, and every `action.rs` site. Two
places break that convention, and both only misbehave once the visible count
diverges from the total count, i.e. after any column is hidden.

Found while forging `ai-docs/mental-model/table-state.md`; confirmed by reading
source, not by running a repro.

1. **Cursor retention in `validate_cc`.** The `CursorState::Select` branch
   decodes with `cc_prev_n_columns` and re-encodes with `self.p.num_columns`
   (search `let old_cols = self.cc_prev_n_columns` in
   `src/draw/state/validation.rs`). While all columns are visible the two counts
   are equal, so the bug is invisible; with a hidden column, any selection that
   survives a `cc_dirty` pass is re-encoded against the wrong stride and every
   later decode resolves to the wrong cells. Note `cc_prev_n_columns` is only
   ever written inside this branch, so it is also stale whenever the branch does
   not run.
2. **`validate_interactive_cell` is called at two different times relative to the
   mutation it compensates for.** The function decodes and re-encodes using
   `self.p.vis_cols.len()` and uses its `new_num_column` argument only to clamp
   the column component. `command.rs`'s `SetVisibleColumns` arm calls it
   *before* replacing `p.vis_cols`, so both decode and re-encode use the old
   stride; `validate_cc` then calls it again after the mutation, decoding an
   old-stride value with the new stride. The two transforms compound.

## Decisions

- Treat `p.vis_cols.len()` as the single legal stride. Any fix that makes
  `p.num_columns` a legal stride in one more place is the wrong direction.
- Fix both sites together: they are the same defect class, and fixing only the
  retention branch would leave the interactive cell landing on a different cell
  than the selection it belongs to.

## Constraints

- Selections are stored as `VisSelection(min, max)` rectangles, so a stride
  change is not a linear remap — decode to `(row, col)` with the stride that
  produced the value, then re-encode with the new one. Getting the *producing*
  stride right is the whole problem; consider storing selections as row/column
  pairs instead of pre-flattened linear indices so the question cannot arise.

## Phases

### Phase 1: Correct the stride handling and pin it with tests

Make the cursor-retention and interactive-cell paths agree on which stride
produced a stored index and which stride should encode the new one. Decide
whether to keep `cc_prev_n_columns` (and then maintain it on every path that
changes the visible count) or to remove it in favour of storing unflattened
coordinates.

Verification must include the currently missing cases: a live multi-cell
selection surviving a column hide, a column hide while the interactive cell sits
to the right of the hidden column, and a hide followed by a show. `state/tests.rs`
today only asserts `vis_cols().len()` after `SetVisibleColumns` and never has a
selection active across the change.
