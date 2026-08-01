# Plan: Fix actionlint CI

## Relevant Ticket Contract
- Replace only the invalid `rhysd/actionlint` reference with a valid upstream release; prefer an immutable revision only if that matches repository convention.
- Verify the workflow succeeds after merge. Do not change crates.io publishing, release tags, unrelated workflows, or application code.

## Out of Scope
- `.github/workflows/cargo-publish.yml` and every release/publishing concern.
- Application and library Rust code, plus unrelated GitHub Actions maintenance.

## Codebase Findings
- `.github/workflows/actionlint.yml#L1-L21` — the dedicated workflow runs only on workflow-file changes and its sole invalid action reference is `rhysd/actionlint@v1` at line 21.
- `.github/workflows/cargo-publish.yml#L17-L18` and `.github/workflows/pages.yml#L20-L48` — all existing third-party actions use version tags; none uses a 40-character immutable SHA, so the established repository convention supports a release-tag reference.
- `.github/workflows/actionlint.yml#L20-L21` — checkout remains `actions/checkout@v4`; only the following action reference needs replacement. Upstream verification on 2026-08-01 found the latest `rhysd/actionlint` release is `v1.7.12`, resolving to `393031adb9afb225ee52ae2ccd7a5af5525e03e8`.

## Implementation Plan
1. In `.github/workflows/actionlint.yml`, replace only `rhysd/actionlint@v1` with the verified release tag `rhysd/actionlint@v1.7.12`, preserving the job, trigger, permissions, checkout step, and all other workflows unchanged.

## Verification Plan
- Review the workflow-only diff to confirm the action reference is the sole changed line.
- After the change merges to `master`, confirm the triggered `Validate GitHub Actions` run's `actionlint` job succeeds (for example, inspect the newest `actionlint.yml` run with `gh run list --workflow actionlint.yml --branch master --limit 1` and `gh run view <run-id>`).

## Escalations
- None.
