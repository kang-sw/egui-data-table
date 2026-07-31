# Plan: tag-triggered crates.io publishing

## Relevant Ticket Contract
- Replace the manual GitHub Actions crates.io publish path with an automatic
  trigger for pushed `v*` release tags.
- Keep crates.io authentication secret-based, publish only the root
  `egui-data-table` crate, and retain the release build/test checks.
- Update `ai-docs/ship/egui-data-table.md` to state that a tag push triggers
  CI-owned publication; do not publish, push a tag, or change the release
  version in this work.
- Validate workflow syntax/permissions and repository-relevant checks.

## Out of Scope
- Changing `Cargo.toml`'s version, the pending package-boundary settings in
  `ai-docs/ship/egui-data-table.md`, or the Cargo.lock tracking policy.
- Publishing a crate, creating or pushing any tag, changing the Pages workflow,
  or unrelated action-version/security modernization.

## Codebase Findings
- `.github/workflows/cargo-publish.yml#L1-L18` — the sole publish workflow is
  manual-only, performs `cargo login` with `secrets.CRATES_IO_TOKEN` as a
  command-line argument, then invokes unqualified `cargo publish`; its current
  build and test steps are the checks that must remain before publication.
- `Cargo.toml#L1-L17` — the published root package is also the non-virtual
  workspace root, with `demo` as its only member. Cargo currently defaults to
  the root package here, but an explicit `-p egui-data-table` is the durable
  enforcement of the one-crate release contract.
- `demo/Cargo.toml#L1-L30` — the demo remains a separately named workspace
  package and does not yet declare `publish = false`; workflow package selection
  must therefore exclude it directly rather than relying on a future manifest
  change.
- `.gitignore#L1-L10` and `ai-docs/ship/egui-data-table.md#L17-L33` —
  `Cargo.lock` is deliberately ignored while the recorded release commands use
  `--locked`. A clean GitHub checkout has no lockfile, so those flags would
  fail; do not copy them into the tag workflow without changing the separately
  out-of-scope lockfile policy.
- `ai-docs/ship/egui-data-table.md#L26-L38` — the ship configuration currently
  makes packaging, dry-run, publishing, and tag push manual steps, so it needs
  a single explicit order: pushed matching tag, CI gate/checks, package/dry-run,
  then CI publish.
- `ai-docs/_index.md#L64-L66` and
  `ai-docs/mental-model/demo-examples.md#L74-L78` — these documents still say
  the README doctest fails after the 0.34 transition, but the current
  `cargo test -p egui-data-table --doc` passes; this release-workflow change
  must not preserve that stale assumption by omitting the documentation test.

## Implementation Plan
1. In `.github/workflows/cargo-publish.yml`, replace `workflow_dispatch` with
   the `push` event restricted to `tags: ['v*']`, retain checkout and the Rust
   cache, and set top-level `permissions: { contents: read }`. Do not request
   GitHub write, packages, Pages, or OIDC permissions: checkout is the only
   `GITHUB_TOKEN` operation and crates.io uses its own secret.
2. Add an early release-identity gate in
   `.github/workflows/cargo-publish.yml` that compares `github.ref_name` with
   the root package version (the tag must be exactly `v<manifest version>`).
   This makes the broad required `v*` trigger safe against an accidental or
   malformed release tag before any registry credential is exposed.
3. In that same job, keep the root crate's build/test coverage ahead of every
   packaging or publish operation, explicitly selecting
   `-p egui-data-table`; include the root doctest release check. Follow it with
   `cargo package -p egui-data-table` and `cargo publish -p egui-data-table
   --dry-run` before the real publish. Do not use `--locked` while the project
   intentionally omits `Cargo.lock` from commits.
4. Make the final `.github/workflows/cargo-publish.yml` publish step the only
   step receiving `CARGO_REGISTRY_TOKEN: ${{ secrets.CRATES_IO_TOKEN }}` and run
   `cargo publish -p egui-data-table` from that environment. Remove `cargo
   login` so the token is neither passed as a command argument nor persisted in
   Cargo credentials; failure of any preceding gate must prevent this step.
5. Update `ai-docs/ship/egui-data-table.md` without changing its version
   strategy: describe the release order and responsibility boundary as
   commit/version preparation, push `v<version>`, CI validates the tag and runs
   pre-flight/package dry-run, then CI publishes the root crate. Replace the
   manual publish command with that CI-owned step and make the documented
   commands compatible with the ignored lockfile.

## Verification Plan
- Run `actionlint .github/workflows/cargo-publish.yml` when available (or an
  equivalent GitHub Actions-aware YAML validator) and inspect that the workflow
  has only the `push.tags: ['v*']` trigger, `contents: read` permissions, and
  no `cargo login` or broad `GITHUB_TOKEN` write scope.
- Run `cargo fmt --all --check`, `cargo check -p egui-data-table --all-targets`,
  `cargo test -p egui-data-table --all-targets`, and
  `cargo test -p egui-data-table --doc`; the current project passes these checks
  with deprecation warnings in examples only.
- Use a local non-publishing dry-run of the workflow's package-selection
  commands with an unpublished release version after the separately excluded
  version-preparation work lands; do not use the already-published current
  `0.10.0` version as a publish-dry-run success criterion.

## Escalations
- None.
