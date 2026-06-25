---
title: Editing, Selection, And Commands
summary: Observable table editing, selection, command routing, undo/redo, and mutation behavior.
---

# Editing, Selection, And Commands

`egui-data-table` exposes editing and command behavior through the renderer UI,
`UiAction`, `RowViewer` hooks, callbacks, clipboard effects, and `DataTable`
state flags. The internal command and selection state types are not public API.

## Public Action Surface {#260625-public-action-surface}

`UiAction` names the table actions callers can bind to keyboard shortcuts or
invoke through built-in input surfaces. The action surface includes edit
commit/cancel/start actions, cursor movement, selection movement, copy, cut,
paste, paste-insert, undo, redo, row insertion/deletion, fill, clear, and
select-all behavior.

The renderer maps these actions to observable table behavior. It does not expose
the internal command queue or selection state as public API.

## Edit Start, Commit, And Cancel {#260625-edit-start-commit-and-cancel}

Editing starts only for editable cells. A caller can start editing through table
actions or by interacting with a cell according to the configured edit mode.
When `Style::single_click_edit_mode` is enabled, a primary click can start
editing immediately; otherwise the cell must already be the interactive cell.

The editor uses `RowViewer::show_cell_editor` and holds an editable row value.
Committing the edit writes the row value back to the table. Canceling the edit
discards the editor value. Clicking outside the table commits an active edit,
except when the click is still within the table response area used by embedded
widgets.

## Write Permission Checks {#260625-write-permission-checks}

Writes initiated by table UI actions respect caller-provided write controls.
Before non-editor writes mutate a cell, the table checks both whether the cell
is editable and whether `RowViewer::confirm_cell_write_by_ui` accepts the write.

Clear, paste, and fill operations identify the write context through
`CellWriteContext` so callers can allow or reject the operation according to its
source.

## Selection Behavior {#260625-selection-behavior}

The table supports rectangular cell selection, drag selection, row-header
selection, command-modifier toggling, and shift-extension from the current
selection. Selection state drives the visible selected cells and current
interactive cell.

When selection changes, the table invokes highlight callbacks so callers can
observe the highlighted cell and changed highlighted rows.

## Keyboard And Mouse Action Routing {#260625-keyboard-and-mouse-action-routing}

The renderer routes keyboard and mouse input to table actions only when the
table interaction state allows it. Viewer-provided hotkeys are consumed while
the table has focus. System copy, cut, and paste events are mapped to table
clipboard actions when the table is not actively editing a cell.

The table body context menu exposes available actions with labels and shortcut
text. Menu entries are shown or enabled according to selection state, clipboard
state, undo/redo availability, and viewer permissions.

## Undo And Redo {#260625-undo-and-redo}

Mutating table commands record restore operations so callers can undo and redo
observable table mutations. Starting a new mutating command clears redo history.

The undo history is capped by renderer style. A maximum undo history of `0`
uses the default capacity of `100`.

## Row Operations {#260625-row-operations}

The table can duplicate selected rows, insert pasted rows, and delete selected
rows through UI actions. Row insertion and deletion respect
`RowViewer::allow_row_insertions`, `RowViewer::allow_row_deletions`, and
`RowViewer::confirm_row_deletion_by_ui`.

Row update, insertion, and removal callbacks run for command-driven mutations,
including undo and redo mutations.

## Clipboard Command Behavior {#260625-clipboard-command-behavior}

Copy, cut, paste, and fill operate through the table clipboard model. Copy and
cut capture selected cells. Fill writes clipboard-derived values across the
selected area. Paste writes clipboard-derived values starting at the current
selection target, and paste-insert can insert rows when row insertion is
allowed.

When a compatible codec exists, the renderer also bridges these commands to the
system clipboard. The detailed text format and codec rules are specified by the
clipboard and TSV interoperability behavior.

## Observable Mutation Outputs {#260625-observable-mutation-outputs}

Table mutations can produce observable effects outside the internal command
state:

- system clipboard writes;
- highlighted-cell and highlight-change callbacks;
- row update, insertion, and removal callbacks;
- the `DataTable::has_user_modification` flag.

These outputs let callers react to user-driven table changes without depending
on internal renderer state.
