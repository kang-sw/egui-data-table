---
domain: command-undo
description: "The Command protocol, the single mutation gate, and where permission checks actually happen"
sources:
  - src/draw/state/
related:
  table-state: "commands are the only writers of the cc_* caches and column/sort state"
  viewer-contract: "permission hooks are consulted here, at command construction time"
  render-pipeline: "commands are collected during rendering and applied in one loop at frame end"
  clipboard-tsv: "CcUpdateSystemClipboard must be intercepted before reaching this pipeline"
---

# Command / Undo

## Entry Points

- `src/draw/state/command.rs` — `Command<R>`, `push_new_command`, `cmd_apply`,
  `undo`, `redo`.
- `src/draw/state/action.rs` — `try_apply_ui_action`: `UiAction` → commands.

## Module Contracts

- `Command<R>` has two families. `Cc*` variants are transient: each has an early
  `return` arm in `push_new_command`'s match that either mutates UI state
  directly or rewrites itself into a real command and **recurses**. The six real
  variants (`SetVisibleColumns`, `SetColumnSort`, `SetRowValue`, `SetCells`,
  `InsertRows`, `RemoveRow`) are the only ones that produce a `restore` list,
  enter `undo_queue`, and are legal in `cmd_apply`. Every `Cc*` arm at the bottom
  of `cmd_apply` is `unreachable!()` (`command.rs:354-362`) — the split is
  enforced by convention plus a panic, not by types.
  {#260625-public-action-surface}
- `push_new_command` is the single mutation gate. `cmd_apply` performs **zero**
  permission checks: it never consults `is_editable_cell`,
  `confirm_cell_write_by_ui`, `confirm_row_deletion_by_ui`, `allow_row_insertions`
  or `allow_row_deletions`. Anything reaching it is treated as pre-authorized.
  {#260625-write-permission-checks}
- Because undo/redo call `cmd_apply` directly (`command.rs:374-405`), replay
  skips every permission hook but fires the same data callbacks
  (`on_row_updated` / `on_row_inserted` / `on_row_removed`) as first application,
  and re-raises `DataTable::dirty_flag`. {#260625-undo-and-redo}
  {#260625-observable-mutation-outputs}
- Pending edits auto-commit: any command other than `CcCommitEdit`/`CcCancelEdit`
  arriving while editing recurses through `CcCommitEdit` first
  (`command.rs:50-53`). The recursion terminates because `try_take_edition`
  leaves `cc_cursor` in `Select` before its own recursive push. One user gesture
  that both closes an edit and mutates therefore lands **two** undo entries.
  {#260625-edit-start-commit-and-cancel}
- Queue order in `push_new_command` (`command.rs:259-277`): drain redo entries
  (they live at indices `0..undo_cursor`), trim from the tail to capacity, reset
  the cursor, apply, then `push_front`. Capacity 0 does not disable history — it
  still keeps the just-applied entry; `body.rs:597-601` maps a `Style` value of 0
  to 100 anyway.
- `RemoveRow` requires **unique ascending** `RowIdx` values. Both the contiguous
  chunking that builds the inverse (`command.rs:230-251`) and the
  `binary_search`-based retain in `cmd_apply` (`command.rs:346-350`) depend on it,
  and it is guarded only by `debug_assert!`. {#260625-row-operations}
- `SetCells`' inverse dedups **consecutive** row ids (`command.rs:180-189`), so
  its `values` must already be grouped by row. Every current producer satisfies
  this via `BTreeSet` ordering or `Clipboard::sort`.

## Coupling

- `try_apply_ui_action` is where per-action permission gating lives, and the
  coverage is uneven: `DuplicateRow` checks `allow_row_insertions`, `DeleteRow`
  checks `allow_row_deletions` plus per-row confirmation, the `CcSetCells` paths
  (paste-in-place, fill, clear) inherit filtering from `push_new_command`, and
  navigation/selection actions check nothing.
- Each gated action must also be gated at all three trigger surfaces — the
  hotkey table in `default_hotkeys`, the clipboard-event handling in
  `body.rs:43-58`, and context-menu item visibility in `body.rs:385-406`. These
  drift independently.
- `CcUpdateSystemClipboard` must be intercepted in `body.rs:578-583` before the
  generic loop; it is `unreachable!()` in both `push_new_command` and `cmd_apply`.

## Extension Points & Change Recipes

- **Add a `Cc*` variant**: add the early-`return` arm in `push_new_command` *and*
  the `unreachable!()` arm in `cmd_apply`. Missing the former makes it enter the
  undo queue and panic on the first apply or redo.
- **Add an undoable variant**: compute the inverse in `push_new_command`, mutate
  in `cmd_apply`, set `cc_dirty` there if the row set or column layout changed
  (`command.rs:324,339` are the pattern), and add a no-op guard if an empty
  payload is possible.
- **Add a `UiAction`**: wire it into `default_hotkeys` and/or a context-menu push;
  there is no automatic registration. Gate it explicitly unless it routes through
  `CcSetCells`.

## Common Mistakes

- Building `Command::RemoveRow` from visual order. `UiAction::DeleteRow` maps
  `collect_selected_rows()` (ascending `VisRowPos`) through `cc_rows`
  (`action.rs:214-218`); under an active sort that mapping is not monotonic in
  `RowIdx`, so multi-row deletion while sorted violates the ascending invariant —
  a `debug_assert` panic in debug builds, a wrong `binary_search` result in
  release. Untested: the only `RemoveRow` test uses a single index.
- Assuming a rejected write leaves no trace. `SetRowValue`, `SetCells` and
  `InsertRows` have no no-op guard (unlike `SetVisibleColumns`, `SetColumnSort`
  and `RemoveRow`), so a paste or clear whose cells are all filtered out still
  drains the redo stack, consumes an undo slot, and sets `dirty_flag`.
- Calling `cmd_apply` directly to "skip the ceremony" — that bypasses the
  auto-commit contract and leaves `cc_cursor` in `Edit` while data moves.
- Re-checking permissions at apply time. They are evaluated once, at construction.

## Technical Debt

- `UiAction::PasteInsert` never calls `allow_row_insertions()`
  (`action.rs:141-175`), unlike `DuplicateRow`. Two of its three trigger sites
  gate it (`body.rs:51-54`, `body.rs:385`), but the paste-insert hotkeys are
  consumed ungated, so a viewer that forbids insertions can still receive rows.
- The `RemoveRow` sortedness invariant is `debug_assert`-only despite being
  reachable from ordinary UI interaction (see above).
- `tests.rs` covers `SetRowValue`, single-index `RemoveRow`, `InsertRows`,
  `SetColumnSort`, `SetVisibleColumns` and redo-stack clearing, but never
  constructs `SetCells`/`CcSetCells` and never calls `try_apply_ui_action` — so
  none of the permission wiring above is under test.
