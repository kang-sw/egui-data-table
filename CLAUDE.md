# CLAUDE.md

## Project Overview

`egui-data-table` is a generic, feature-rich data table widget for [egui](https://github.com/emilk/egui). Users implement the `RowViewer<R>` trait to define per-row rendering and editing behavior; the library provides undo/redo, column reordering/visibility, row insertion/deletion, keyboard navigation, and clipboard support out of the box.

- Crate: `egui-data-table` (published on crates.io)
- Rust edition 2024, MSRV 1.75
- Workspace members: root lib crate + `demo/` (eframe/wasm demo app)
- Demo hosted via GitHub Pages: https://kang-sw.github.io/egui-data-table/

## Work Guidelines

1. **Commit at each compilable logical unit.** Every commit must compile (`cargo build`). Group related changes into a single commit; do not leave the tree in a broken state between commits.
2. **Run `cargo fmt` before finishing.** Always run `cargo fmt` after completing work to ensure consistent formatting.
3. **Commit message convention:** Use conventional-commit style prefixes (`fix`, `feat`, `refactor`, `test`, `docs`, `ci`, `proj`, etc.) as seen in the git log.

## Build & Test

```sh
cargo build          # build the library
cargo test           # run all tests (unit + doctests)
cargo fmt            # format code
cargo clippy         # lint
```

### Demo (wasm)

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve demo/index.html
```
