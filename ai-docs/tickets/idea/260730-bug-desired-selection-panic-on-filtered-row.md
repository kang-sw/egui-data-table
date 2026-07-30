---
title: "handle_desired_selection panics when an inserted row is filtered out"
related-mental-model:
  - table-state
  - command-undo
---

# handle_desired_selection panics when an inserted row is filtered out

## Background

`Command::InsertRows` unconditionally queues the inserted range for selection
(search `queue_select_rows(range.map(RowIdx))` in `src/draw/state/command.rs`)
and sets `cc_dirty`, forcing a full re-filter on the next frame.
`handle_desired_selection` then resolves each queued `RowIdx` through
`self.cc_row_id_to_vis[&row_id]` — an unchecked `HashMap` index — but
`cc_row_id_to_vis` is rebuilt from the **filtered** row set only.

If the viewer implements `filter_row` and a freshly pasted or duplicated row
does not match the active filter, the next frame panics with "no entry found for
key". Reachable from `UiAction::PasteInsert` and `UiAction::DuplicateRow` in any
application with a search/filter box.

Found while forging `ai-docs/mental-model/table-state.md`; derived from code
tracing, not reproduced at runtime.

## Decisions

- The queued selection is a best-effort UX affordance, not a contract. Skipping
  rows that did not survive the filter is the right behavior; there is no reason
  to force a filtered-out row into view or to clear the filter on the user's
  behalf.

## Constraints

- `handle_desired_selection` returns `bool` to tell `validate_cc` whether to skip
  the cursor-retention branch. If every queued row is filtered out, decide
  deliberately whether that counts as "handled" (leaving an empty selection) or
  as "not handled" (falling through to retention of the previous cursor); the two
  differ in what the user sees after pasting rows that the filter hides.

## Phases

### Phase 1: Make desired-selection resolution total

Replace the unchecked index with a lookup that skips missing rows, and settle the
`bool` return question above. Add a regression test combining an active
`filter_row` that rejects the inserted row with `queue_select_rows` — the
existing tests cover filtering and desired selection separately but never
together.
