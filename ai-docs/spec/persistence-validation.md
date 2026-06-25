---
title: Persistence And Validation
summary: Observable cache invalidation, persisted UI state, and validation repair behavior for rendered data tables.
---

# Persistence And Validation

`egui-data-table` keeps renderer UI state alongside table rows. This spec covers
the caller-visible effects of dirty/cache invalidation, opt-in persistence, and
validation repair. It does not define a stable serialized persistence schema or
public validation API.

## Dirty Cache Invalidation APIs {#260626-dirty-cache-invalidation-apis}

`DataTable<R>` exposes `is_dirty` to report whether attached renderer caches
need refresh. It returns false before a table has attached UI state.

Programmatic row changes invalidate or drop attached UI cache state:

- `take` empties the table and marks attached UI state dirty.
- `replace` swaps row data and marks attached UI state dirty.
- `retain` marks attached UI state dirty only when it removes rows.
- mutable row access through `DerefMut` marks attached UI state dirty before
  returning mutable access.
- `Extend` appends rows and drops the cached UI state.
- `Clone` clones row data and starts with no cached UI state.

The deprecated `clear_dirty_flag` method is retained for compatibility and does
not change table state.

## User Modification Flag {#260626-user-modification-flag}

User-driven row changes made through the rendered table set a user-modification
flag. Cell writes, row-value updates, row insertions, and row removals set this
flag.

Callers observe the flag with `has_user_modification` and clear it with
`clear_user_modification_flag`.

## Persistence Opt-In {#260626-persistence-opt-in}

The default `persistency` Cargo feature enables serde support required for
persisted UI state.

Persistence is opt-in per viewer. `RowViewer::persist_ui_state` defaults to
false; when a viewer returns true, the renderer participates in egui's persisted
data mechanism. The persistence key is the current egui `Ui` id used while
rendering the table.

## Persisted UI State Behavior {#260626-persisted-ui-state-behavior}

On the first render with persistence enabled, the renderer attempts to load
previous table UI state from egui persisted data.

Loaded state is accepted only when its stored column count matches the current
viewer column count. Persisted sort entries for columns that are no longer
sortable are pruned. When persistent state changes after rendering, the renderer
writes the current persistent UI state back to egui persisted data.

The serialized payload is an internal detail and is not a stable public schema.

## Viewer Identity Validation {#260626-viewer-identity-validation}

Renderer creation validates that the cached UI state still belongs to the same
viewer type and column count.

When the viewer identity or column count no longer matches, the renderer resets
the UI state, initializes all current columns as visible, and marks the cache
dirty. When the viewer filter hash changes for the same viewer identity, the
renderer marks the cache dirty so filtered rows are rebuilt.

## Filter And Sort Cache Rebuild {#260626-filter-and-sort-cache-rebuild}

When renderer caches are dirty, validation rebuilds the visible row cache from
the current rows. It includes rows accepted by `RowViewer::filter_row`, applies
stored sort criteria through `RowViewer::compare_cell`, resets row-height cache
entries, and rebuilds lookup data for visible row positions.

If caches are already clean, validation preserves the current row cache except
for pending desired-selection handling and bounds repair.

## Visible Column And Sort State Validation {#260626-visible-column-and-sort-state-validation}

Column visibility and sort state are part of renderer UI state. Header actions
can hide, show, or reorder visible columns; applying those actions updates the
stored visible-column state and validates the current interactive cell.

Header sort actions update stored sort state. Clearing sort removes stored sort
criteria. Sort validation keeps only columns that the viewer still reports as
sortable.

## Selection And Cursor Bounds Repair {#260626-selection-and-cursor-bounds-repair}

Validation applies pending desired selections and repairs selection/cursor state
against the current visible row and column bounds.

Out-of-range selection cursors are dropped or clamped to valid bounds. Non-cell
cursor state can be replaced with an empty selection when it is no longer valid.
The current interactive cell is clamped to available row and column bounds.

## Observable Validation Limits {#260626-observable-validation-limits}

Validation is renderer behavior, not a public API. The crate does not expose
public validation functions or a validation-specific callback contract.

Validation repair may change internal UI state before the next render. Normal
selection and row mutation callbacks are specified by the editing, selection,
and commands behavior.
