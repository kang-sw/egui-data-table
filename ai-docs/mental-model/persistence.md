---
domain: persistence
description: "The persistency cargo feature: what it gates, the PersistData load/save contract, and its silent fallbacks"
sources:
  - src/
related:
  table-state: "PersistData owns vis_cols and sort, which exist with or without the feature"
  render-pipeline: "validate_persistency must run before validate_cc and after the header is built"
  viewer-contract: "persist_ui_state is the per-viewer opt-in"
  public-api: "the persistence key is ui.id(), shared with editor windows and column widths"
---

# Persistence

## Entry Points

- `src/draw/state/validation.rs:57-83` — `validate_persistency`, the whole
  load/save path.
- `src/draw/state/mod.rs` — `PersistData` and the `is_p_loaded` flag.

## Module Contracts

- The feature is default-on and adds only `dep:serde` (`Cargo.toml:32-34`). What
  it gates: the `is_p_loaded` field and its initializer, the serde derives on
  `PersistData` and on the index newtypes it contains (`state/types.rs:26,28`),
  `validate_persistency` itself, `RowViewer::persist_ui_state`
  (`viewer.rs:269-272`), and the call site (`draw/body.rs:77-80`). Everything
  else — `PersistData` as a struct, `vis_cols`, `sort`, all validation and
  command handling — is unconditional and must keep compiling both ways. Both
  `cargo check` and `cargo check --no-default-features` pass today, tests
  included. {#260626-persistence-opt-in}
- Opt-in is per viewer and re-read every frame: `persist_ui_state()` defaults to
  false, and `body.rs:78` double-gates on the cfg and the call.
- Load happens once per `UiState` lifetime, guarded by `is_p_loaded`
  (`validation.rs:64`). That is weaker than once per session: `UiState` is
  rebuilt — and thus reloads — whenever `DataTable::extend` or `clone` drops the
  cache, or `validate_identity` detects a viewer-type/column-count change.
  {#260626-persisted-ui-state-behavior}
- The only identity check on load is `p.num_columns == self.p.num_columns`
  (`validation.rs:71`). Column names, types and order are not checked, and
  `vis_cols`/`sort` are positional indices — a refactor that keeps the column
  count but reorders columns silently reapplies the old layout to different
  columns. Persisted sort entries are then pruned against the live
  `is_sortable_column` (`validation.rs:77`).
  {#260626-visible-column-and-sort-state-validation}
- Saving is gated on `cc_dirty` (`validation.rs:79`), the same flag that drives
  cache rebuilds — so row insert/remove also trigger a redundant re-save, and any
  new command that changes `self.p` without setting `cc_dirty` will never be
  persisted. {#260626-viewer-identity-validation}

## Coupling

- The storage key is `ui.id()` (`draw/mod.rs:157`), which also seeds the cell
  editor windows and — through egui_extras' fixed salt — column widths and scroll
  offset. Two sibling tables in the same `Ui` overwrite each other's persisted
  layout with no error. Callers must `ui.push_id(..)` per table.
- Saving lags the action that caused it by one frame: `validate_persistency` runs
  near the top of `impl_show_body` (`body.rs:77-80`), but the commands that
  mutate `self.p` are applied at the bottom (`body.rs:578-605`). A "flush before
  shutdown" needs another frame, not just a `cc_dirty` check.
- The header is built before the load runs, so the first frame after a load shows
  the default column layout against already-sorted rows.

## Extension Points & Change Recipes

- **Add a persisted field**: add it to `PersistData`, gate any new nested type's
  derives the same way `types.rs:26-29` does, give the field a serde default so
  previously saved blobs still deserialize, add a validation pass against the
  live viewer if it references columns, and make sure whatever mutates it sets
  `cc_dirty`.
- **Verify a change**: run both `cargo check` and `cargo check
  --no-default-features`. No test exercises persistence at all, so CI will not
  catch a regression here.

## Common Mistakes

- Adding a field without a serde default → every old blob fails to deserialize,
  `unwrap_or_default()` (`validation.rs:69`) substitutes an empty `PersistData`,
  the `num_columns` guard then rejects it, and the table silently starts fresh.
  The only trace is egui's own warning log.
- Assuming this crate's `persistency` feature is sufficient for cross-session
  storage. It only makes `PersistData` serializable; the host application must
  also enable egui/eframe's own `persistence` feature for `ctx.memory()` to be
  written and restored at all. This repo does so for its examples
  (`Cargo.toml:26`) but the library does not request it for consumers.
- Treating `is_p_loaded` as "loaded once, ever".
- Implementing `persist_ui_state` in an example without the matching
  `#[cfg(feature = "persistency")]` — see `demo-examples`.

## Technical Debt

- `PersistData` has no version field and no serde defaults, so any schema change
  degrades to a silent reset.
- Nothing detects `ui.id()` collisions between tables, and no `id_salt` builder
  exists to disambiguate them.
- Zero test coverage; `state/tests.rs` never mentions persistence.
