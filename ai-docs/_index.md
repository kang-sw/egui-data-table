<!-- Memory policy: prune aggressively as project advances. Completed
     work belongs in git history, not here. Keep only what an AI session
     needs to orient itself and pick up work. If it's derivable from
     code or git log, delete it from this file. -->

# egui-data-table AI Index

## Project Summary

`egui-data-table` is a generic data table widget for `egui`. Users implement
`RowViewer<R>` to define row rendering and editing behavior; the library
provides table state, undo/redo, row insertion/deletion, column
reordering/visibility, keyboard navigation, and clipboard support.

## Tech Stack

- Rust edition 2024.
- Main crate: `egui-data-table`.
- GUI stack: `egui`, `egui_extras`, and demo `eframe`.
- Workspace members: root library crate and `demo/`.
- Published crate target: crates.io.
- Demo site: https://kang-sw.github.io/egui-data-table/

## Workspace

- `src/` contains the library implementation.
- `examples/` contains native/web demo examples.
- `demo/` wraps `examples/demo.rs` for the hosted wasm demo.
- `demo/index.html` is the Trunk entry point.

## Build And Test

```sh
cargo build
cargo test
cargo fmt
cargo clippy
cargo check --workspace --all-targets
```

For the wasm demo:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve demo/index.html
```

## Read Before Edit

- For table rendering or egui integration, start with `src/draw/`.
- For framework-agnostic table state behavior, start with `src/draw/state/`.
- For public user extension points, start with `src/viewer.rs`.
- For demo behavior, start with `examples/demo.rs` and `demo/Cargo.toml`.

## Ticket Focus

No active tickets yet.

## Session Notes

- Bootstrapped `AGENTS.md`-based ws workflow context from the prior `CLAUDE.md`.
- Specs and mental models have not been forged yet; use `ws:lead-forge-spec`
  and `ws:lead-forge-mental-model` when durable behavioral or modification
  baselines are needed.
