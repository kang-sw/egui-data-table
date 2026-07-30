---
title: "State-layer test coverage gaps around clipboard, persistence and UI actions"
related-mental-model:
  - table-state
  - command-undo
  - clipboard-tsv
  - persistence
---

# State-layer test coverage gaps around clipboard, persistence and UI actions

## Background

The library has 32 unit tests, all in the state layer, and they cover the
undoable command round-trips reasonably well. The gaps are not random — they line
up exactly with the areas where the mental-model forge found suspected defects,
which is why those defects survived.

Untested today:

- **`clipboard.rs` end to end.** No test encodes a selection to TSV and decodes
  it back. The suspected column-bounds off-by-one and the paste ordering
  assumption both live in code no test exercises.
- **Persistence.** `validate_persistency` and `PersistData` have zero coverage;
  the `persistency` feature is only ever built, never behaviorally tested.
- **`try_apply_ui_action`.** The dispatch from UI action to command — including
  the permission hooks that run at construction time — is untested, so a hook
  that stops being consulted (see the paste-insert permission ticket) fails
  silently.
- **`SetCells`.** Every other undoable command has a round-trip test; this one
  does not, despite being the multi-cell write path and the paste target.
- **Cursor retention across column-visibility changes.** The stride mismatch in
  `validate_cc` is a pure-state behavior with no display dependency, and no test
  pins it.

The architecture rule that state must be headless-testable is already satisfied
by `DataModelOps` — these paths are testable today, they simply are not tested.

## Constraints

- Tests here are only worth writing if they pin *behavior a defect would break*.
  Do not add coverage that asserts the current buggy result and thereby freezes
  it; where a suspected defect overlaps, either write the test as part of that
  ticket's fix or write it to the intended behavior and mark it ignored with a
  reference to the defect ticket.
- Persistence tests must not require a live `egui::Context` beyond what the
  existing test scaffolding already does; if they would, say so and scope down to
  `PersistData` round-trips.
- No display dependency in any new test — that is the point of the split.

## Phases

### Phase 1: Clipboard and SetCells coverage

TSV encode/decode round-trips including single-cell, multi-column, and
boundary-column selections, plus a `SetCells` undo/redo round-trip.

### Phase 2: UI action dispatch and persistence coverage

`try_apply_ui_action` dispatch with permission hooks denying and allowing, and
`PersistData` save/load including the mismatch paths. Sequenced second because
both overlap open defect tickets whose fixes may change the intended behavior.
