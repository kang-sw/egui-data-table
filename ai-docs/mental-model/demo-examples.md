---
domain: demo-examples
description: "How examples/ and the demo/ wasm wrapper relate, which example owns which RowViewer feature, and what actually builds"
sources:
  - examples/
  - demo/
related:
  viewer-contract: "examples/demo.rs is the reference implementation for nearly every RowViewer hook"
  public-api: "the README example is compiled as a doctest"
  persistence: "demo.rs implements persist_ui_state without a cfg guard"
---

# Demo & Examples

## Entry Points

- `examples/demo.rs` — the kitchen-sink viewer, plus both native and wasm `main`.
- `demo/Cargo.toml` — a wrapper crate whose only `[[bin]]` points at that file.
- `.github/workflows/pages.yml` — the Trunk build that publishes the demo.

## Module Contracts

- `demo/` has no sources of its own; it exists so `examples/demo.rs` can be built
  as a real binary crate for wasm. The reason it must be a separate crate is
  `getrandom`: `demo/Cargo.toml` pins `getrandom` with the `js` feature, the root
  package does not, so `cargo check --target wasm32-unknown-unknown` on the root
  package's examples fails outright. Only the `demo` crate has a working wasm
  dependency graph. {#260626-wasm-demo-wrapper}
- `demo/index.html` declares the Trunk rust link with no `data-bin`, which works
  only while `demo/Cargo.toml` has exactly one `[[bin]]`.
- `egui` and `egui_extras` (root `[dependencies]`) plus `eframe` and the
  example-only crates (root `[dev-dependencies]`) are all declared a second time
  in `demo/[dependencies]`, pinned by hand to the same versions. Nothing enforces the alignment; a one-sided bump surfaces as
  duplicate-crate type errors, not a version-mismatch message. Commit `111d64c`
  is the precedent for bumping both plus all three examples together.
- `internationalization.rs` and `partially_editable.rs` are native-only; neither
  has a wasm `main`.

## Coupling

Which example is the reference implementation for what — i.e. what must be
updated when a `RowViewer` hook changes:

- `examples/demo.rs` — sole reference for `RowCodec`/`try_create_codec`,
  `is_sortable_column`/`compare_cell`, `row_filter_hash`/`filter_row`,
  `confirm_cell_write_by_ui`/`confirm_row_deletion_by_ui`, every `on_*` lifecycle
  callback, `on_cell_view_response` drag-drop, custom `hotkeys`, and
  `persist_ui_state`. It is also what the hosted demo runs, so it is the default
  update target for any trait change. {#260626-main-spreadsheet-demo}
  {#260626-main-demo-codec-data-behavior} {#260626-main-demo-events}
- `examples/internationalization.rs` — sole reference for `Translator`; update it
  whenever a translation key is added in `draw/mod.rs` or `draw/body.rs`. A
  missing key does not fail to compile, it renders the raw key.
  {#260626-internationalization-example} {#260626-translator-api-and-fallback}
  {#260626-translated-built-in-labels}
- `examples/partially_editable.rs` — sole reference for user-toggled
  `allow_row_insertions`/`allow_row_deletions` and for a `show_cell_editor` that
  returns `None` on some columns. {#260626-partially-editable-demo}
- `README.md` is a fourth de-facto reference: it is included as crate docs
  (`src/lib.rs:1`) and therefore compiled as a doctest.

## Extension Points & Change Recipes

- **Verify example changes**: `cargo check --workspace --all-targets` covers the
  lib, the `demo` bin and all three examples. It passes today.
- **Verify the wasm path**: `trunk build --release --dist demo/dist
  ./demo/index.html` — the same command CI runs, minus `--public-url`. A native
  `cargo check` cannot see wasm-only breakage.
- **Bump egui**: edit root `Cargo.toml` and `demo/Cargo.toml` together, adapt all
  three examples, and re-run the Trunk build.

## Common Mistakes

- Running `trunk build` from the repo root without `--dist demo/dist` — output
  lands in `/dist`, which `.gitignore` does not cover (only `/demo/dist` is
  ignored).
- Bumping egui on one manifest only.
- Adding a feature-gated trait method and implementing it in an example without
  the matching `#[cfg]`.

## Technical Debt

- `examples/demo.rs:420-422` implements `persist_ui_state` with no
  `#[cfg(feature = "persistency")]`, so `cargo check --example demo
  --no-default-features` fails with E0407. No CI job runs that combination, so it
  stays latent.
- `pages.yml`'s path filter covers `demo/**`, `examples/**` and `src/**` but not
  the manifests, so a dependency-only bump does not redeploy the demo.
- `Cargo.lock` is gitignored, so neither the published crate nor the demo build
  is reproducible across runs.
- `demo/index.html` still carries eframe-template boilerplate: the default
  `<title>` and a service-worker registration for a `sw.js` that does not exist.
