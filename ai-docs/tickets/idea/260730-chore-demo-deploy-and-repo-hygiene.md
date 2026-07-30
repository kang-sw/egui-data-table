---
title: "Demo deployment and repository hygiene gaps"
related-mental-model:
  - demo-examples
  - render-pipeline
  - table-state
---

# Demo deployment and repository hygiene gaps

## Background

A cluster of small, independently trivial issues found while forging
`ai-docs/mental-model/demo-examples.md` and the state/render domain docs. None
is urgent; each costs a future session time when hit cold.

Deployment and repository:

- `.github/workflows/pages.yml` filters on `demo/**`, `examples/**` and `src/**`,
  but not the manifests. A dependency-only bump does not redeploy the demo.
- `.gitignore` covers `/demo/dist` but not a bare `/dist`. Running Trunk from the
  repo root without `--dist demo/dist` writes multi-megabyte build output to an
  untracked-but-not-ignored directory that `git add -A` will happily commit.
- egui/egui_extras/eframe versions are pinned by hand in both the root manifest
  and `demo/Cargo.toml`, with nothing enforcing alignment; a one-sided bump shows
  up as duplicate-crate type errors rather than a version mismatch.
- `demo/index.html` still carries eframe-template boilerplate: the default
  `<title>` and a service-worker registration for a `sw.js` that does not exist.

Source-level staleness:

- The comment above `validate_cc`'s call site in `src/draw/body.rs` justifies
  deferring validation because the body closure "may not be called if the table
  area is out of the visible space". With the pinned `egui_extras` 0.34 the body
  closure runs every frame; only rows are virtualized. The comment invites a
  wrong assumption about when validation happens.
- `DataTable`'s `Extend` doc comment says the operation invalidates "the index
  table cache", but it drops the whole `UiState` — undo history, clipboard, sort
  and column layout included.
- `VisSelection::_from_row_col` is dead code, never called anywhere.

## Constraints

- The manifest-alignment problem has two shapes — detect drift in CI, or remove
  the duplication (workspace dependency inheritance). Pick one deliberately;
  adding a CI check while keeping the duplication is the weaker option but may be
  the cheaper one.
- Comment and doc corrections are behavior-neutral and can land together; the
  Pages workflow change cannot be verified without a push, so state how it was
  checked.

## Phases

### Phase 1: Deployment and repository hygiene

The Pages path filter, the `/dist` ignore, the manifest-alignment decision, and
the `index.html` boilerplate.

### Phase 2: Source comment and doc-comment corrections

The stale virtualization comment, the `Extend` doc comment, and removal of the
dead `_from_row_col` helper. Depends on Phase 1 only for commit ordering, not
technically.
