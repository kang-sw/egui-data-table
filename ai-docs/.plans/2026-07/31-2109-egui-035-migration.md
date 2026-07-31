# Plan: egui-035-migration

## Relevant Ticket Contract

- Migrate the direct `egui`, `egui_extras`, and demo `eframe` requirements from
  0.34 to aligned 0.35 versions; resolve migration-caused compatibility errors
  and verify the workspace.
- Preserve the existing public API intent. PR #57 handling, unrelated refactors,
  and intentional public-API changes are excluded.
- Required verification is formatting, workspace tests/all targets,
  documentation tests, and relevant workspace checks discovered during
  implementation.
- The upstream 0.35.0 workspace sets `rust-version = "1.92"`; its `egui`,
  `egui_extras`, and `eframe` packages inherit that value. The already-resolved
  upstream 0.34.3 workspace sets the same floor, so Rust 1.75 is unsupported
  before as well as after this migration.

## Out of Scope

- PR #57 and the Pages workflow's manifest path-filter gap in
  `.github/workflows/pages.yml`.
- Refactoring table state, renderer behavior, examples, or public traits except
  where the 0.35 compiler/API contract requires it.
- Changing package or release versioning policy beyond declaring the existing
  actual Rust 1.92 requirement through Cargo metadata.
- Committing generated dependency resolution: `Cargo.lock` is ignored.

## Codebase Findings

- `Cargo.toml` declares the published library's direct `egui` and
  `egui_extras` dependencies and example-only `eframe` dependency; it has no
  Cargo `rust-version` field. `demo/Cargo.toml` repeats all three requirements
  for the wasm wrapper. The duplicate declarations must remain on one upstream
  minor, as required by the project architecture and public re-exports.
- `src/lib.rs` re-exports `egui`, and `src/viewer.rs` re-exports
  `egui_extras::Column` as `TableColumnConfig`. A split minor version produces
  incompatible consumer types, so do not solve migration errors with a second
  egui line or a compatibility shim.
- `src/draw/mod.rs` (`Renderer::impl_show`) and `src/draw/body.rs`
  (`Renderer::impl_show_body`) own the library's direct `egui_extras`
  `TableBuilder`/`TableBody` use. Preserve their rendering and interaction
  contracts; make only 0.35-required API adaptations.
- `examples/demo.rs` is the demo wrapper's binary and exercises native and wasm
  `eframe` entry points. `examples/internationalization.rs` and
  `examples/partially_editable.rs` independently exercise native
  `eframe::App` and `RowViewer` paths. Commit `111d64c` is the local precedent:
  update both manifests, then make only framework-compiler-required example
  changes.
- `README.md` is included as crate documentation by `src/lib.rs`. Its
  `RowViewer::show_cell_editor` doctest is already invalid on 0.34: the
  `TextEdit::show` result needs one further response extraction. The three
  examples already use the established form. This is not a new 0.35 behavior,
  but repairing it is necessary to satisfy the accepted documentation-test
  boundary.
- `README.md` claims MSRV 1.75, but the crate is edition 2024 and history
  records that the prior egui upgrade required Rust 1.85 (`e3b8e07`). Both the
  resolved 0.34.3 and target 0.35.0 upstream workspaces declare 1.92. The
  stated 1.75 floor is therefore already stale; the 0.35 migration does not
  raise the actual upstream MSRV. Upstream tags `0.34.3` and `0.35.0` are the
  evidence sources.
- `.github/workflows/cargo-publish.yml` runs `cargo build` and `cargo test` on
  the current stable toolchain; it does not enforce an MSRV. The Pages workflow
  builds the wasm wrapper with Trunk, so a native-only check cannot cover the
  hosted demo's dependency graph.

## Implementation Plan

1. **Make Cargo metadata the sole MSRV authority before changing
   dependencies.** Define `rust-version = "1.92"` once in the root
   `[workspace.package]` metadata, then inherit it in each workspace package
   with `rust-version.workspace = true`. Remove the obsolete README MSRV claim
   and the stale historical MSRV parenthetical in `CHANGELOG.md`; documentation
   must not duplicate the Cargo declaration. This corrects an already-false
   packaging claim, rather than creating a new 0.35 compatibility break.

2. **Move the aligned dependency set together.** In `Cargo.toml`
   `[dependencies]`/`[dev-dependencies]` and `demo/Cargo.toml`
   `[dependencies]`, change the direct `egui`, `egui_extras`, and `eframe`
   requirements to the 0.35 minor while preserving the existing feature sets.
   The governing compatibility mechanism is the root re-exports in `src/lib.rs`
   and `src/viewer.rs`; do not add duplicate or pinned transitive egui packages
   as a workaround.

3. **Adapt only compiler-identified framework API changes.** Compile the
   renderer through `Renderer::impl_show` in `src/draw/mod.rs` and
   `Renderer::impl_show_body` in `src/draw/body.rs`, preserving the existing
   `TableBuilder` column, selection, scrolling, drag-and-drop, persistence, and
   response-aggregation behavior. Then compile each `eframe::App` implementation
   and native/wasm startup path in the three example files. Follow the existing
   framework usage patterns; do not change table state, callback semantics, or
   user-visible layout merely to silence a warning.

4. **Make the documentation test represent the supported egui response
   contract.** In `README.md`, update the `RowViewer::show_cell_editor` minimal
   example to return the final `egui::Response` from the text editor, matching
   the working response-extraction pattern in `examples/demo.rs`,
   `examples/internationalization.rs`, and
   `examples/partially_editable.rs`. Keep the public trait signature and the
   rest of the tutorial unchanged.

## Verification Plan

- Post-implementation, run `cargo fmt --all --check`, `cargo check --workspace
  --all-targets`, `cargo test --workspace --all-targets`, and `cargo test
  --doc`.
- Verify the declared support floor with Rust 1.92: run the workspace check and
  documentation test using the 1.92 toolchain. This closes the gap left by the
  current CI, which tests only whatever stable is current.
- Inspect the resolved workspace tree after the update to confirm that the
  direct egui-family packages resolve on the 0.35 minor and there is no second
  egui minor caused by the duplicated manifests.
- Run the Pages-equivalent wasm build when the wasm target and Trunk are
  available: `trunk build --release --verbose --public-url /egui-data-table
  --dist demo/dist ./demo/index.html`.

## Escalations

- **Resolved MSRV policy:**
  The user approved Cargo metadata as the single MSRV authority and removal of
  redundant documentation statements. Rust 1.92 is the declared floor because
  both resolved 0.34.3 and target 0.35.0 upstream packages require it.
