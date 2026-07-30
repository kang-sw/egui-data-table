---
title: "Renderer::new's seed row bypasses the command pipeline and on_row_inserted"
related-mental-model:
  - public-api
  - command-undo
---

# Renderer::new's seed row bypasses the command pipeline and on_row_inserted

## Background

`Renderer::new` pushes a row into the table when `rows` is empty and
`allow_row_insertions()` is true (search `table.rows.is_empty()` in
`src/draw/mod.rs`). The push goes through `DataTable`'s `DerefMut`, so it:

- creates no undo entry — the row cannot be undone;
- never calls `RowViewer::on_row_inserted` — it is the only insertion path that
  skips the callback, so a viewer assigning IDs or sequence numbers there misses
  exactly this row;
- sets `cc_dirty` but not `dirty_flag`, so it does not count as a user
  modification (this part is arguably correct).

There is also no doc comment on `Renderer::new` mentioning that constructing a
renderer mutates the table.

A related consequence: delete the last row, and on the next frame a fresh
untracked row appears. Undoing the delete then restores the original row
alongside the seeded one.

Found while forging `ai-docs/mental-model/public-api.md`.

## Decisions

- The seeding behavior itself stays — an empty editable table with no row is a
  dead end for the user. The defect is that it lies outside the pipeline every
  other insertion goes through.

## Constraints

- Routing it through `Command::InsertRows` would put a table-construction side
  effect into the undo history, which is arguably worse: the user would be able
  to undo a row they never created, back into an empty table that immediately
  re-seeds. Weigh three options explicitly before implementing — (a) route
  through the command pipeline, (b) keep the direct push but call
  `on_row_inserted`, (c) keep it as-is and document it — and record why the
  others lost.
- Whatever is chosen, the interaction with "undo of a delete-last-row" must be
  stated, since that is the sequence that produces a visibly wrong row count.

## Phases

### Phase 1: Settle and implement the seed-row contract

Decide among the three options above, implement it, and document the behavior on
`Renderer::new`. If the callback is added, note in the Result that viewers now
receive an `on_row_inserted` they previously did not — that is an observable
behavior change for existing consumers.
