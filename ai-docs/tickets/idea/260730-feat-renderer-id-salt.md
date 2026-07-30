---
title: "Expose an id salt so multiple tables can coexist in one Ui"
related-mental-model:
  - render-pipeline
  - persistence
---

# Expose an id salt so multiple tables can coexist in one Ui

## Background

`Renderer` derives everything from `ui.id()` and never sets
`TableBuilder::id_salt`. That single id simultaneously keys:

- the persisted `PersistData` blob (`ctx.memory` get/insert in
  `validate_persistency`),
- the per-cell editor windows (`ui_id.with(row_id).with(column)` in
  `src/draw/body.rs`),
- and, through `egui_extras`' fixed internal salt, the column widths and scroll
  offset.

Sibling `Ui`s share an id unless given different salts, so rendering a list of
tables in a loop without `ui.push_id(..)` makes all of the above collide at once:
persisted layouts overwrite each other, and edit/selection state leaks between
tables. There is no builder method to disambiguate.

The crate already acknowledges a neighbouring multi-table hazard — the
column-reorder drag payload matches by type, so a drag between two tables
delivers a valid-looking index to the wrong table (see the comment on the
reorder command in `src/draw/state/command.rs`).

Found while forging `ai-docs/mental-model/render-pipeline.md` and
`ai-docs/mental-model/persistence.md`.

## Decisions

- A builder method is the right shape: it matches the existing `with_*` chain and
  does not force callers into `ui.push_id`, which they must currently discover on
  their own.

## Constraints

- The salt must reach `TableBuilder::id_salt` **and** the persistence key **and**
  the editor window ids, or the collision only partially goes away. Passing it to
  one of the three is worse than not adding it, because it looks solved.
- Changing the default persistence key would orphan already-persisted layouts.
  Keep the no-salt behavior byte-identical to today.
- The drag-payload cross-table confusion is a separate mechanism (egui's DnD
  payload typing) that a salt does not fix; note it as out of scope rather than
  implying the feature closes it.

## Phases

### Phase 1: Add the salt and thread it through every id derivation

Add the builder method, apply the salt to all three id derivations, and verify
that two tables rendered as siblings keep independent persisted layouts, column
widths, scroll offsets and edit state. Document the multi-table requirement where
consumers will find it.
