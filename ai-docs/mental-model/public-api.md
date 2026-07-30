---
domain: public-api
description: "Consumer-facing surface: DataTable ownership, the Renderer builder, and the two independent dirty flags"
sources:
  - src/
related:
  render-pipeline: "Renderer::new/Drop own the UiState handoff that the per-frame pipeline mutates"
  table-state: "DataTable's mutation APIs decide how much of UiState survives"
  viewer-contract: "RowViewer identity (TypeId + num_columns) decides whether UiState survives a frame"
  persistence: "the persistency feature is default-on and changes nothing about this surface except serde derives"
---

# Public API

## Entry Points

- `src/lib.rs` — `DataTable<R>`, the crate re-exports, and both dirty flags.
- `src/draw/mod.rs:87-147` — `Renderer::new` plus the builder chain.

## Module Contracts

- `DataTable<R>` guarantees the `UiState<R>` cache (undo queue, selection, sort,
  visible columns, clipboard) survives across frames — but only because
  `Renderer::new` takes it out (`draw/mod.rs:94`) and `Drop for Renderer` puts it
  back (`draw/mod.rs:362-366`). For the entire lifetime of a live `Renderer`,
  `table.ui` is `None`. Any refactor that moves `state` out early must restore it
  before drop, or every frame silently starts from a default `UiState`.
  {#260625-renderer-widget-lifecycle}
- `Renderer::new` mutates the table as a side effect: when `rows` is empty and
  `allow_row_insertions()` is true it pushes one row through `DerefMut`
  (`draw/mod.rs:89-91`). That row bypasses the command pipeline entirely — no
  undo entry, no `on_row_inserted`, no `dirty_flag`. Consumers that assign IDs in
  `on_row_inserted` will miss exactly this row.
  {#260625-data-table-row-container}
- `pub extern crate egui` (`lib.rs:10`) and `pub use egui_extras::Column as
  TableColumnConfig` (`viewer.rs:4`) make the egui version part of the public
  contract: a consumer on a different egui minor gets unrelatable type errors.
  {#260625-crate-export-surface}

## Coupling

- Two flags with different lifetimes and different owners, easy to conflate
  {#260625-dirty-and-user-modification-state} {#260626-dirty-cache-invalidation-apis}:
  - `is_dirty()` reads `UiState::cc_dirty`, which `validate_cc` **consumes**
    (`state/validation.rs:86`). It is a "cache needs rebuild" signal, not a
    change log — after one render it flips back to false on its own.
    `clear_dirty_flag()` is a deprecated no-op precisely because manual clearing
    would fight that contract.
  - `has_user_modification()` reads `DataTable::dirty_flag`, written **only**
    inside `cmd_apply` (`state/command.rs:299,307,325,340`). It never
    auto-clears — `clear_user_modification_flag()` is the only reset — and
    because undo/redo replay reuses `cmd_apply`, undoing an edit re-raises it.
    {#260626-user-modification-flag}
  - Programmatic mutation (`push`, `retain`, `replace`, `take`, `extend`) never
    touches `dirty_flag`. `has_user_modification()` answers "did the widget
    change the data", never "did anything change the data".
- Mutation APIs invalidate at two very different strengths
  {#260625-data-table-data-replacement-and-filtering}:
  - `take` / `replace` / `retain` / `DerefMut` → `mark_dirty()` only. Undo
    history, sort, visible columns and selection all survive.
  - `Extend::extend` → `self.ui = None` (`lib.rs:130`). This discards undo
    history, clipboard, sort and column layout. The doc comment only mentions the
    "index table cache", which understates it. Replacing a `for r in rows {
    table.push(r) }` loop with `table.extend(rows)` is a behavior change, not an
    optimization.
  - `Clone` drops `ui` but copies `dirty_flag` (`lib.rs:154-163`).
- `DerefMut` marks dirty unconditionally, so holding `&mut DataTable` in a hot
  path forces a full re-filter/re-sort every frame.

## Extension Points & Change Recipes

- **Add a builder option**: `with_style` *replaces* `self.style` wholesale
  (`draw/mod.rs:104-107`), so `.with_table_row_height(h).with_style(s)` silently
  drops `h`. `with_style_modify` is the composable form. Any new builder method
  must decide which side of that it lives on. {#260625-renderer-builder-configuration}
- **Add a `Style` field**: `Style` is `#[non_exhaustive]` and explicitly marked as
  growing (`draw/mod.rs:28`). Keep `Default` meaningful; consumers construct it
  via `..Default::default()`.
- `max_undo_history == 0` means "use the built-in 100" (`draw/body.rs:597-601`),
  not "no history". There is currently no way to request zero undo depth.
- There is no `id_salt`/`with_id` escape hatch on `Renderer`. Two tables rendered
  as siblings in one `Ui` share `ui.id()`, which is simultaneously the
  persistence key and the editor-window id seed — callers must wrap each in
  `ui.push_id(...)` themselves.

## Common Mistakes

- Using `has_user_modification()` to detect programmatic edits → always false.
- Using `is_dirty()` as a change flag across frames → self-clears on render.
- Switching to `table.extend(..)` for bulk loading → wipes the user's undo
  history and column layout.
- Rendering the same `DataTable` through two different `RowViewer` **types** →
  `validate_identity` compares `TypeId`, so every alternation resets all UI
  state (`state/validation.rs:10,48`).
- Adding a builder method after `with_style` in a doc example → readers copy the
  clobbering order.

## Technical Debt

- The `Renderer::new` auto-insert path is undocumented and unreachable by undo;
  it is the only row insertion that skips `on_row_inserted`.
- `Extend`'s doc comment understates its blast radius.
- `README.md` is included as crate docs (`lib.rs:1`) and its minimal example no
  longer compiles under egui 0.34 — `cargo test` fails on that doctest alone
  (lib unit tests pass). See `demo-examples` for the exact error.
