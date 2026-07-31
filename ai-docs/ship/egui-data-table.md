# Ship: egui-data-table

## Version Strategy

- Follow pre-1.0 semantic versioning: a compatibility-breaking upstream change
  increments the minor version.
- For this release, change the root `egui-data-table` package version from
  `0.10.0` to `0.11.0` in `Cargo.toml` and add the corresponding stable release
  entry to `CHANGELOG.md`.
- Set `demo/Cargo.toml` to `publish = false`; the demo is not a self-contained
  crates.io package and is not a release target.
- Set the root package `include` allowlist to `/src/**`, `/examples/**`, and
  `/README.md`. Cargo supplies the manifest, lockfile, license, normalized
  manifest, and VCS metadata automatically.
- Commit the version and package-boundary changes before creating the tag.

## Pre-flight

- `cargo fmt --all --check`
- `cargo check -p egui-data-table --all-targets --locked`
- `cargo test -p egui-data-table --all-targets --locked`
- `cargo test -p egui-data-table --doc --locked`
- `cargo +1.92 check -p egui-data-table --all-targets --locked`
- `cargo +1.92 test -p egui-data-table --doc --locked`

## Build

- `cargo package -p egui-data-table --locked`
- `cargo publish -p egui-data-table --dry-run --locked`

## Publish

- `cargo publish -p egui-data-table --locked`

## Tag

Format: `v<version>`
Push: yes
