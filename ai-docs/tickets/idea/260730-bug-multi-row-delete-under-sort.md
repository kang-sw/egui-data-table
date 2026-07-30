---
title: "Deleting multiple rows while a column sort is active violates RemoveRow's ordering invariant"
related-mental-model:
  - command-undo
  - table-state
---

# Deleting multiple rows while a column sort is active violates RemoveRow's ordering invariant

## Background

`Command::RemoveRow(Vec<RowIdx>)` requires unique ascending indices. Two
mechanisms depend on it: the contiguous-chunk grouping that builds the inverse
`InsertRows` commands, and the `binary_search`-based retain in `cmd_apply`. The
invariant is guarded only by `debug_assert!`, so it disappears in release builds.

`UiAction::DeleteRow` builds its index list from `collect_selected_rows()` — a
`BTreeSet<VisRowPos>`, ascending in **visual** order — mapped through `cc_rows`
to `RowIdx`. When `p.sort` is non-empty, `cc_rows` is reordered by `compare_cell`,
so that mapping is not monotonic. Selecting several rows in a sorted table and
deleting them therefore produces an unsorted list.

Effect: debug builds panic on the `debug_assert`; release builds run
`binary_search` against unsorted data, so the wrong rows can be removed and the
undo entry reconstructs the table incorrectly.

Found while forging `ai-docs/mental-model/command-undo.md`; confirmed by reading
`action.rs`, `selection.rs` and `validation.rs` together, not by running a repro.

## Decisions

- Sort at the producer (`UiAction::DeleteRow`), not by relaxing the invariant.
  The chunking and `binary_search` in `cmd_apply` both genuinely need ordered
  input, and other producers already satisfy it.
- Keep the `debug_assert`s. They are what surfaced the defect; the fix is to stop
  violating them, not to remove them.

## Constraints

- Deletion order is also user-visible through `on_row_removed`, which fires per
  index before the retain pass. Sorting the list changes the callback order for
  this path — acceptable, but state it in the Result so a viewer relying on
  visual-order callbacks is not surprised silently.
- Any other producer of `RemoveRow` added later inherits the same requirement;
  consider whether a constructor that sorts and dedups is worth more than the
  assertions.

## Phases

### Phase 1: Guarantee ordered indices at the delete-row producer

Sort and dedup the collected indices before constructing `Command::RemoveRow`,
and add a regression test that deletes multiple non-adjacent rows with an active
sort, asserting both the resulting rows and a correct undo. The existing
`RemoveRow` test uses a single index and no sort, so it cannot catch this.
