---
title: "PasteInsert hotkeys bypass allow_row_insertions"
related-mental-model:
  - command-undo
  - viewer-contract
---

# PasteInsert hotkeys bypass allow_row_insertions

## Background

`UiAction::PasteInsert` produces `Command::InsertRows` without consulting
`vwr.allow_row_insertions()`, unlike its sibling `UiAction::DuplicateRow` which
gates on it explicitly (both in `src/draw/state/action.rs`).

The permission is enforced at two of the three trigger surfaces: the OS paste
event with Shift held, and the context-menu "insert" item's visibility (both in
`src/draw/body.rs`). The paste-insert keyboard shortcuts from `default_hotkeys`
are consumed with no such gate, so a viewer that returns `false` from
`allow_row_insertions()` can still have rows inserted.

Found while forging `ai-docs/mental-model/command-undo.md`; confirmed by reading
all three trigger sites.

## Decisions

- Gate inside `try_apply_ui_action`, where `DuplicateRow` and `DeleteRow` already
  gate. Adding a third ad-hoc check at the hotkey site would leave the same class
  of drift for the next action.
- This is a permission bypass, not a missing feature: `allow_row_insertions`
  already documents itself as the switch, and `partially_editable.rs` exposes it
  as a user-facing toggle, so the current behavior contradicts a shipped example.

## Constraints

- The gate must not silently swallow the paste: decide whether a blocked
  paste-insert falls back to paste-in-place or does nothing. Doing nothing is the
  conservative reading of the permission, but state the choice explicitly since
  the user pressed a paste key and will expect *something*.

## Phases

### Phase 1: Gate PasteInsert on allow_row_insertions

Add the check at the action level and audit the remaining `UiAction` arms for the
same drift — the general rule is that only the `CcSetCells`-routed paths inherit
filtering from `push_new_command`; `InsertRows`/`RemoveRow` producers must gate
themselves. Cover it with a test that drives `try_apply_ui_action` with a viewer
denying insertions; no test currently calls `try_apply_ui_action` at all.
