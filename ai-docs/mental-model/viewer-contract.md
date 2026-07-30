---
domain: viewer-contract
description: "What RowViewer implementers must keep consistent, and how DataModelOps narrows the trait for headless tests"
sources:
  - src/
related:
  public-api: "RowViewer identity decides whether the DataTable's cached UiState survives"
  command-undo: "permission hooks are consulted at command construction, never during replay"
  table-state: "row_filter_hash is the only signal that re-runs filter_row"
  clipboard-tsv: "RowCodec is reached only through try_create_codec"
---

# Viewer Contract

## Entry Points

- `src/viewer.rs` — `RowViewer<R>`, `RowCodec<R>`, `UiAction`, `default_hotkeys`.
- `src/draw/state/model_ops.rs` — `DataModelOps<R>`, the egui-free narrowing.

## Module Contracts

- `num_columns()` is the cache identity together with `TypeId::of::<V>()`
  (`state/validation.rs:10`). Any change nukes `UiState` wholesale, undo history
  included. A `num_columns()` derived from flapping state resets the table every
  frame it flips, silently. {#260626-viewer-identity-validation}
- `filter_row` re-runs on every cache rebuild, but `row_filter_hash()` is the
  **only** signal that detects a change in the filter predicate *itself*
  (`state/validation.rs:15-18`). Its default returns `&()`, a constant, so a
  stateful `filter_row` with the default hash appears frozen until something
  unrelated dirties the cache. `examples/demo.rs:402-408` is the correct pattern.
  {#260625-row-viewer-extension-contract}
- `is_sortable_column()` and `compare_cell()` are independent defaults (`false`
  and `Ordering::Equal`). Marking a column sortable without a comparator gives a
  live sort arrow and no reordering — a silent no-op, not an error.
- `compare_cell` is applied one sort key at a time via successive stable sorts in
  reverse key order (`state/validation.rs:105-114`), so it must be a strict weak
  order per column; there is no combined multi-column comparator hook.
- `is_sortable_column` is consulted from three places that must agree
  (`validation.rs:36-39`, `validation.rs:77`, header click handling in
  `draw/mod.rs:267,277`). If it is not a pure function of `column`, sort config
  gets silently pruned on the frames it returns false.
- `column_name` / `column_render_config` are called at two different points of
  the same frame (`draw/mod.rs:186` while building columns, `draw/mod.rs:217+`
  in the header closure). Depending on state that mutates between them makes
  layout and content disagree.
- `confirm_cell_write_by_ui` is **not** a universal write gate. It fires only for
  the `CcSetCells`-routed bulk paths — paste, fill, clear (`state/command.rs:149-178`).
  The inline editor commit (`CcCommitEdit` → `SetRowValue`) and the drag-drop
  write from `on_cell_view_response` (`draw/body.rs:455-468`) both bypass it; the
  latter is filtered by `is_editable_cell` only. {#260625-write-permission-checks}
- `confirm_row_deletion_by_ui` is consulted only at interactive `DeleteRow`
  (`state/action.rs:216`); undo/redo replay never calls it, by design.
  {#260625-write-permissions-and-row-lifecycle-hooks}
- `hotkeys()` is replace-not-extend: the default body returns `default_hotkeys(ctx)`
  wholesale, so an override that does not call it drops every built-in shortcut,
  and must handle both `UiCursorState` branches or edit start/commit/cancel become
  keyboard-unreachable. {#260625-ui-actions-and-default-hotkeys}

## Coupling

- `DataModelOps<R>` (`model_ops.rs:7-35`) is a hand-written subset of `RowViewer`
  containing only egui-free methods, blanket-implemented for every
  `V: RowViewer<R>`. `validation.rs` and `command.rs` take `impl DataModelOps<R>`
  and therefore *cannot* reach rendering methods — the headless-testability rule
  is compiler-enforced, not conventional.
- Adding a pure-data `RowViewer` method that state logic needs requires two hand
  edits in `model_ops.rs` (trait declaration + forwarding body). Both omissions
  are compile errors, so this is a chore, not a silent hazard.
- `tests.rs` implements `DataModelOps` **directly** on its mock
  (`state/tests.rs:34-114`), never through `RowViewer`. Every default body
  defined on `RowViewer` itself — `clone_row`, `column_name`, `compare_cell`,
  `row_filter_hash` — is therefore untested by `cargo test`.

## Extension Points & Change Recipes

- **Change clipboard copy semantics**: override `clone_row_as_copied_base`, not
  `clone_row`. All three clone variants default to `clone_row`
  (`viewer.rs:227-235`), so overriding the base one also changes undo snapshots,
  `SetCells` internals and edit-start snapshotting.
- **Add a RowCodec**: return it from `try_create_codec`. The blanket `()` impl
  (`viewer.rs:44-65`) is all `unimplemented!()` and is safe only because the
  default returns `None::<()>`; returning `Some(())` compiles and panics on first
  copy/paste. {#260625-clipboard-codec-contract}
- `CellWriteContext`, `EmptyRowCreateContext`, `UiActionContext` and `UiAction`
  are `#[non_exhaustive]`; `UiCursorState` is not — an intentional-looking
  asymmetry with no comment explaining it.

## Common Mistakes

- Stateful `filter_row` + default `row_filter_hash` → stale filtering.
- `is_sortable_column = true` without `compare_cell` → sort silently does nothing.
- Treating `confirm_cell_write_by_ui` as the single validation choke point →
  typed edits and drag-drop writes slip through.
- Overriding `clone_row` when only copy behavior was meant to change.
- `clone_row` is called far more often than "once per user action" — three times
  for a single `SetRowValue` (the inverse at `command.rs:142-147`, plus the
  old-row snapshot and the value copy at `command.rs:297-301`), plus once per row
  for insert/remove undo args. Side effects in it will fire repeatedly.
- Returning a row whose addressable column count disagrees with `num_columns()`
  — the default `clone_row` loops `0..num_columns()` calling `set_cell_value`
  with no bounds check anywhere in the crate.

## Technical Debt

- The `'static` bound (`viewer.rs:68-69`) is load-bearing for `TypeId`-based
  identity and blocks viewers that borrow non-`'static` data. The TODO waits on
  Rust language support.
- `model_ops.rs` duplicates 21 signatures by hand with no macro tying them to the
  trait.
- No test exercises the `false` branches of `is_editable_cell` /
  `confirm_cell_write_by_ui`; the mock always returns true.
