---
title: "Mental-model forge findings (2026-07-30)"
related-mental-model:
  - public-api
  - viewer-contract
  - render-pipeline
  - table-state
  - command-undo
  - clipboard-tsv
  - persistence
  - demo-examples
---

# Mental-model forge findings (2026-07-30)

## Context

Reconstructing `ai-docs/mental-model/` from scratch meant reading every source
module against its actual behavior rather than its documentation. That surfaced a
set of defects and gaps that were not the point of the exercise and were
deliberately not fixed in that pass — the forge documents behavior, it does not
change it.

This workset exists so that material survives the session it was found in. The
tickets are filed as `idea/` because none of them has been scoped against the
owner's priorities yet; the findings themselves are verified against source, but
the decisions about what to do are not made.

Two of the findings are reproduced command failures rather than reasoning:
`cargo test` fails on the README doctest under egui 0.34, and
`cargo check --example demo --no-default-features` fails with E0407. Both are in
`260730-chore-fix-doctest-and-no-default-features-build`.

## Tickets

Correctness — coordinate systems and invariants:

- `260730-bug-selection-stride-on-column-visibility`
- `260730-bug-desired-selection-panic-on-filtered-row`
- `260730-bug-multi-row-delete-under-sort`

Correctness — command pipeline and permissions:

- `260730-bug-paste-insert-ignores-insertion-permission`
- `260730-bug-noop-write-pollutes-undo-and-dirty-flag`
- `260730-refactor-seed-row-through-command-pipeline`

Clipboard:

- `260730-bug-clipboard-column-bounds-off-by-one`
- `260730-refactor-clipboard-paste-order-invariant`
- `260730-research-clipboard-source-precedence`

API surface:

- `260730-feat-renderer-id-salt`
- `260730-feat-persist-data-schema-guard`
- `260730-feat-undo-history-capacity-semantics`

Infrastructure:

- `260730-chore-fix-doctest-and-no-default-features-build`
- `260730-chore-state-test-coverage-gaps`
- `260730-chore-demo-deploy-and-repo-hygiene`

## Focus

`260730-chore-fix-doctest-and-no-default-features-build` comes first. Everything
else in this set is verified by running tests, and `cargo test` currently fails
before it reaches any of them.

After that, the three coordinate-system bugs are the highest-value group: they
share one root cause shape — visible-column stride versus total-column stride,
and ordering invariants enforced only by `debug_assert` — so fixing them apart
risks three inconsistent local patches instead of one consistent rule. The
stride ticket is the natural entry point.

`260730-chore-state-test-coverage-gaps` is deliberately not first. Its Phase 2
overlaps open defects, and writing tests against today's behavior would freeze
the bugs the rest of this set exists to fix.

## Exit Criteria

Every ticket listed above has been triaged out of `idea/` — promoted with a scope
the owner accepts, or closed with a recorded reason. This workset is a board
artifact for that triage, not an implementation target; it is complete when the
list is empty of untriaged entries, regardless of how many findings were acted
on.
