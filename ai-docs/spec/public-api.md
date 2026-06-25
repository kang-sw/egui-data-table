---
title: Public API
summary: Public Rust API surface for constructing, rendering, and extending egui data tables.
---

# Public API

`egui-data-table` exposes a Rust library API for storing table rows, rendering
tables in egui, and letting callers define row-specific behavior through traits
and callbacks.

## Crate Export Surface {#260625-crate-export-surface}

The crate exposes the `draw` and `viewer` modules and re-exports the primary
types callers need for typical use:

- `Renderer` renders a `DataTable<R>` through a `RowViewer<R>`.
- `Style` configures table presentation.
- `RowViewer` defines the row-specific behavior contract.
- `UiAction` names built-in table actions.
- `egui` is re-exported so callers can align with the egui version used by the
  crate.

## DataTable Row Container {#260625-data-table-row-container}

`DataTable<R>` owns the caller's row collection and the table UI state associated
with that collection. Callers can construct it with `new`, `Default`,
`FromIterator`, or `Extend`, and can inspect or mutate rows through `Deref` and
`DerefMut` to the underlying `Vec<R>`.

Cloning a `DataTable<R>` clones the rows and starts with no attached UI state.
Its debug representation reports the table rows without exposing internal UI
state.

Mutable row access through `DerefMut` marks the attached UI state dirty before
the caller mutates the rows.

## DataTable Data Replacement And Filtering {#260625-data-table-data-replacement-and-filtering}

`DataTable<R>` lets callers replace or filter the row collection without
directly manipulating the underlying vector:

- `take` returns the current rows and leaves the table empty.
- `replace` swaps in a new row vector and returns the old rows.
- `retain` removes rows for which the caller predicate returns false.

When a UI state is attached, `take` and `replace` mark it dirty. `retain` marks
it dirty only when it removes at least one row.

## Dirty And User-Modification State {#260625-dirty-and-user-modification-state}

`DataTable<R>` exposes two observable state flags:

- `is_dirty` reports whether attached UI caches need refresh. It returns false
  when the table has not yet been rendered and no UI state is attached.
- `has_user_modification` reports whether table UI actions have modified rows
  since the flag was last cleared.

`clear_user_modification_flag` clears the user-modification flag. The deprecated
`clear_dirty_flag` method is retained for compatibility and performs no action.

## RowViewer Extension Contract {#260625-row-viewer-extension-contract}

Callers implement `RowViewer<R>` to adapt their row type to the table. The
required methods define the column count, read-only cell rendering, editable
cell rendering, cell assignment, and creation of a new empty row.

Optional methods let callers customize column labels, per-column layout, sort
eligibility, editability, row insertion/deletion permissions, comparison,
filtering, hotkeys, and feature-gated UI state persistence.

Default implementations are permissive and minimal: columns are named from
their index, cells are sortable and editable, rows may be inserted and deleted,
filtering includes every row, and persistence is disabled unless the caller
overrides it.

## Write Permissions And Row Lifecycle Hooks {#260625-write-permissions-and-row-lifecycle-hooks}

`RowViewer<R>` can approve or reject user-driven writes before they mutate row
data:

- `confirm_cell_write_by_ui` receives the row, column, and write context such as
  paste or clear.
- `confirm_row_deletion_by_ui` receives the row proposed for deletion.

Callers can customize row creation and duplication through `new_empty_row_for`,
`clone_row`, and `clone_row_as_copied_base`.

The table reports user-visible lifecycle events through optional callbacks:
highlighted cell changes, selection changes, row updates, row insertions, and
row removals. The update/insert/remove callbacks also run for undo and redo
operations.

## Clipboard Codec Contract {#260625-clipboard-codec-contract}

`RowCodec<R>` defines how row cells are encoded to and decoded from text for
clipboard-oriented data exchange. A `RowViewer<R>` may create a codec for
encoding, decoding, or both through `try_create_codec`.

Decoding failures are controlled by `DecodeErrorBehavior`:

- `Abort` stops the decode operation.
- `SkipCell` leaves the individual cell unchanged.
- `SkipRow` skips the row being decoded.

The unit `()` implementation is a placeholder and does not provide a usable
codec for callers. Clipboard behavior that needs serialization should supply a
real `RowCodec<R>`.

## UI Actions And Default Hotkeys {#260625-ui-actions-and-default-hotkeys}

The public action model exposes table commands through `UiAction`, movement
directions through `MoveDirection`, and current interaction state through
`UiActionContext` and `UiCursorState`.

`default_hotkeys` returns a ready-made keyboard shortcut map for common table
actions such as cursor movement, editing, undo/redo, clipboard operations,
selection, row insertion/deletion, and escape behavior. Callers can return this
map unchanged, extend it, or replace it from `RowViewer::hotkeys`.
