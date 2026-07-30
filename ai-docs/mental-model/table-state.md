---
domain: table-state
description: "The three coordinate systems, the cc_* caches, and the validate_cc rebuild contract"
sources:
  - src/draw/state/
related:
  render-pipeline: "validate_cc runs at a fixed point of the frame and consumes cc_dirty"
  command-undo: "commands are the only user-driven writers of the caches; validation writes them too"
  viewer-contract: "filter_row/compare_cell reach the caches only through DataModelOps"
---

# Table State

## Entry Points

- `src/draw/state/types.rs` — the index newtypes and their conversions.
- `src/draw/state/validation.rs` — `validate_identity`, `validate_cc`.
- `src/draw/state/selection.rs` — selection algebra over visual coordinates.

## Module Contracts

Three coordinate spaces, bridged by exactly two caches:

- `RowIdx` indexes `table.rows` and is **not** a stable identity — it shifts on
  every insert/remove before it.
- `VisRowPos` / `VisColumnPos` index the filtered+sorted view and the visible
  column list. `cc_rows[VisRowPos] == RowIdx`; `cc_row_id_to_vis` is the reverse
  map, rebuilt only inside `validate_cc` (`validation.rs:119-125`).
- `VisLinearIdx` flattens `row * ncol + col`. **The stride is always
  `p.vis_cols.len()`, never `p.num_columns`** — every encode/decode site in
  `selection.rs`, `action.rs`, `clipboard.rs` and `mod.rs` uses the visible
  count. {#260625-selection-behavior}

`validate_cc` (`validation.rs:85-159`) rebuilds in a fixed order when `cc_dirty`
is set: filter → sort (keys applied in reverse, so `p.sort[0]` dominates) →
resize `cc_row_heights` to a flat 20.0 → rebuild `cc_row_id_to_vis` → apply
desired selection or repair the cursor → clamp the interactive cell. New derived
caches must be rebuilt inside that same branch and after both filter and sort.
{#260626-filter-and-sort-cache-rebuild}

`cc_dirty` is consumed, not observed: the first statement of `validate_cc` is
`replace(&mut self.cc_dirty, false)` (`validation.rs:86`). When it is clear,
`validate_cc` degenerates to `handle_desired_selection()` only.
{#260626-dirty-cache-invalidation-apis}

The deferred re-sort is a frame counter, not a timer: `cc_num_frame_from_last_edit`
resets to 0 on each data edit (`command.rs:298,306`), increments only while not
editing (`validation.rs:23-24`), and forces `cc_dirty` when it hits exactly 2 and
a sort is active. This exists so tabbing across cells never re-sorts mid-sequence.
{#260626-observable-validation-limits}

## Coupling

- `command.rs` is the only *user-driven* writer of `cc_dirty`, `p.vis_cols`,
  `p.sort`, `table.rows`, `cc_cursor` and `cc_interactive_cell`, applying
  synchronously inside `cmd_apply` at the end of the frame. It is not the only
  writer: `validate_identity` also sets `cc_dirty`, seeds `p.vis_cols` on reset
  and prunes `p.sort` (`validation.rs:17,28,33-42,53-54`); `validate_persistency`
  sets `cc_dirty` and prunes `p.sort` (`validation.rs:67,77`); `validate_cc`
  rewrites `cc_cursor` (`validation.rs:129-155`); `validate_interactive_cell` is
  also called from `action.rs:256`; and `Renderer::new` pushes a row into
  `table.rows` directly (`draw/mod.rs:90`). When hunting "who changed this", the
  validation path is the second suspect.
- `action.rs` reads the caches to build commands and additionally writes
  `clipboard` and `cc_desired_selection` (`action.rs:128-129`) directly.
- `body.rs` calls `validate_cc` before rendering, so within one frame rendering
  always sees a `cc_rows` consistent with `table.rows` as of frame start. The
  "rows changed but cache is stale" window is exactly one frame, closed by
  `cc_dirty` being set in the same `cmd_apply`.

## Extension Points & Change Recipes

- **Add a cached view of rows**: rebuild it in the `cc_dirty` branch of
  `validate_cc` after filter+sort, and encode any linear index with
  `p.vis_cols.len()`.
- **Add a command that changes the visible column count**: it must run
  `validate_interactive_cell`. Note the two existing call sites disagree about
  when: `command.rs:287` calls it *before* `p.vis_cols` is replaced, so the decode
  and re-encode both use the old stride while only the column component is
  clamped to the new width; `validation.rs:158` calls it again after the
  mutation, decoding with the new stride a value encoded with the old one. Pick a
  convention deliberately rather than copying either site.
- **Queue a selection**: `queue_select_rows` stores `RowIdx` values that
  `handle_desired_selection` resolves through `self.cc_row_id_to_vis[&row_id]`
  (`validation.rs:178`) — an unchecked `HashMap` index.
  {#260626-selection-and-cursor-bounds-repair}

## Common Mistakes

- Encoding a `VisLinearIdx` with `p.num_columns`. The one place that does this is
  the cursor-retention branch itself (see Technical Debt), not a pattern to copy.
- Expecting an edit to re-run `filter_row`. `SetRowValue`/`SetCells` never set
  `cc_dirty` (`command.rs:297-322`), so a filter that depends on cell content
  stays stale until the viewer bumps `row_filter_hash()` or an insert/remove/sort
  happens. This is the normal case, not the exception.
- Inverting sort precedence by appending to `p.sort` — keys are applied in
  reverse (`validation.rs:105`).
- Assuming per-row heights measured last frame survive a rebuild; `validate_cc`
  refills them with a flat default (`validation.rs:117`).

## Technical Debt

- **Selection retention uses the wrong stride.** `validate_cc`'s cursor-retention
  branch decodes with `cc_prev_n_columns` and re-encodes with `p.num_columns`
  (`validation.rs:132-149`), while every other site uses `p.vis_cols.len()`.
  While all columns are visible the two are equal, so this is invisible; once any
  column is hidden, a selection that survives a `cc_dirty` pass is re-encoded with
  the wrong stride and subsequently decodes to the wrong cells. No test covers a
  live selection across a visibility change.
- **`handle_desired_selection` can panic.** `InsertRows` unconditionally queues
  the inserted range (`command.rs:335`) and forces a re-filter. If `filter_row`
  rejects a freshly pasted or duplicated row, the next frame indexes a missing key
  at `validation.rs:178`. A `.get(&row_id)`-and-skip would close it.
- `VisSelection::_from_row_col` (`types.rs:90-92`) is dead code.
- `validation.rs:91-94` notes sorting is single-threaded pending a `Sync`
  comparator story.
