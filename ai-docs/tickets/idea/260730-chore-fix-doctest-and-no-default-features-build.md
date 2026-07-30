---
title: "cargo test and --no-default-features builds are broken"
related-mental-model:
  - demo-examples
  - persistence
---

# cargo test and --no-default-features builds are broken

## Background

Two documented verification commands fail today, both reproduced during the
mental-model forge on 2026-07-30:

1. **`cargo test` fails.** `README.md` is included as crate documentation, so its
   minimal example runs as a doctest. It still calls
   `TextEdit::multiline(..).show(ui).response`, which under the pinned egui 0.34
   yields `AtomLayoutResponse` where a `Response` is expected (E0308). The
   library's 32 unit tests pass; only the doctest fails. `examples/demo.rs`
   already uses the correct double-`.response` form, so the fix is known.

2. **`cargo check --example demo --no-default-features` fails** with E0407:
   `examples/demo.rs` implements `persist_ui_state` unconditionally, but the
   trait method only exists under `#[cfg(feature = "persistency")]`. No CI job
   runs this combination, so it rotted undetected.

Both are trivially fixable; the value of the ticket is that a broken `cargo test`
makes every future change harder to verify.

## Decisions

- Fix the README example rather than removing it from the doctest run. It is the
  first thing a new consumer copies, so it being compilable is the point.

## Constraints

- The README example must stay minimal — it is onboarding material, not a
  feature showcase. Resist expanding it while fixing the call.
- The example fix and the CI gap are one unit of work: fixing `demo.rs` without
  adding a job that builds without default features just resets the clock on the
  same rot.

## Phases

### Phase 1: Restore green verification commands

Fix the README example against egui 0.34, add the missing `#[cfg]` on the
example's `persist_ui_state`, and add CI coverage for the no-default-features
build so both stay fixed. Verify with `cargo test --workspace`,
`cargo check --example demo --no-default-features`, and
`cargo check --workspace --all-targets`.
