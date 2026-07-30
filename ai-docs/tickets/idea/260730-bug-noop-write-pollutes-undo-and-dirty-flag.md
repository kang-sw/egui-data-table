---
title: "Fully rejected writes still consume an undo slot, drain redo, and set the modification flag"
related-mental-model:
  - command-undo
  - public-api
---

# Fully rejected writes still consume an undo slot, drain redo, and set the modification flag

## Background

`push_new_command` short-circuits on no-op input for `SetVisibleColumns`,
`SetColumnSort` and empty `RemoveRow`, but not for `SetRowValue`, `SetCells` or
`InsertRows`.

The visible consequence is on the `CcSetCells` path: it filters its values
through `is_editable_cell` and `confirm_cell_write_by_ui` and can end up with an
empty list, yet still falls through to the generic push. That drains the redo
stack, consumes an undo slot with an empty `restore`, and `cmd_apply`
unconditionally sets `table.dirty_flag = true` and resets
`cc_num_frame_from_last_edit`. So pressing Delete over a read-only selection
makes `has_user_modification()` report true, destroys the redo history, and
costs the user one Undo press that does nothing visible.

Found while forging `ai-docs/mental-model/command-undo.md`.

## Decisions

- Guard at command construction, consistent with the variants that already do.
  Making `cmd_apply` conditional instead would split the "everything reaching
  `cmd_apply` is pre-authorized and applied" contract that undo/redo replay
  depends on.
- `has_user_modification()` is documented as user-driven modification; raising it
  for a write that changed nothing is simply wrong, not a debatable trade-off.

## Constraints

- An identical-value `SetRowValue` is a different question from an *empty*
  `SetCells`: detecting the former needs row comparison the crate cannot perform
  generically (`R` has no `PartialEq` bound). Scope this ticket to empty payloads
  only, and record the value-equality case as deliberately out of scope rather
  than half-solving it.

## Phases

### Phase 1: Skip empty-payload writes

Add no-op guards for empty `SetCells`/`InsertRows` payloads (and the
`CcSetCells` path that produces them), leaving value-equality detection out of
scope. Verify that a clear/paste over a fully non-editable selection leaves the
undo queue, redo stack and `has_user_modification()` untouched — none of which
is currently tested, since no test constructs `SetCells`.
