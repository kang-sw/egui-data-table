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

## Optional Local Validation

These non-publishing checks are optional confidence checks before pushing the
release tag. Tag-triggered CI is authoritative for the required release
validation and publication.

- `cargo fmt --all --check`
- `cargo check -p egui-data-table --all-targets`
- `cargo test -p egui-data-table --all-targets`
- `cargo test -p egui-data-table --doc`
- `cargo +1.92 check -p egui-data-table --all-targets`
- `cargo +1.92 test -p egui-data-table --doc`

## Optional Local Package Validation

- `cargo package -p egui-data-table`
- `cargo publish -p egui-data-table --dry-run`

## Publish

CI owns publication. After a matching `v<version>` tag is pushed, the publish
workflow verifies that the tag matches the root package version, builds and
tests the root crate (including its doctest), packages it, and performs a dry
run before publishing `egui-data-table` to crates.io.

## Tag

Format: `v<version>`

Release order: prepare and commit the version/package-boundary changes, then
optionally run the local validation above and push the matching tag. The
tag-triggered CI workflow performs the required validation and publication; do
not publish locally.
