# AGENTS.md - egui-data-table

## Project Memory

Read at every session start, before other action:

1. **Preamble** - read `ai-docs/_index.md`; keep only context a session must not re-derive.
2. **Local** - read `ai-docs/_index.local.md` if present; it is .gitignored machine context.
3. **Project arc** - run `git log --oneline --graph -50`.
4. **Recent history** - run `git log -10` for `## AI Context` rationale.

## Response Discipline

- **Evidence before claims.** Run verification and read output before stating success.
- **No performative agreement.** Restate the requirement, verify, then act or push back.
- **Actions over words.** Prefer "Fixed. [what changed]" or the diff. Skip filler.

## Code Standards

<!-- Project-wide code quality rules. -->

1. **Simplicity.** Write the simplest complete implementation that satisfies the spec.
2. **Surgical changes.** Change only what the task requires; follow existing Rust and egui style.
3. **Responsibility check.** Keep module roles clean; split when responsibility drifts.
4. **Testability.** Prefer explicit dependencies, minimal hidden state, pure logic over side effects.
5. **Compilable checkpoints.** Keep each committed logical unit compiling.
6. **Formatting.** Run `cargo fmt` before finishing code changes.
7. **Commit style.** Use the conventional-commit prefixes already present in history, such as `fix`, `feat`, `refactor`, `test`, `docs`, `ci`, and `proj`.

## Workflow

### Approval Protocol

- **Auto-proceed:** bug fixes, pattern-following additions, tests, boilerplate, single-module refactors.
- **Ask first:** new components/protocols, architecture changes, cross-module interfaces, observable behavior changes.
- **Always ask:** deleting functionality, changing protocol/API semantics, modifying persistence schema.

### Commit Rules

Auto-create one commit per logical unit. Include `## AI Context` explaining why the approach was chosen.

```text
<type>(<scope>): <summary>

<what changed - brief>

## AI Context
- <decision rationale, rejected alternatives, user directives, etc.>

## Ticket Updates                          # optional - ticket-driven only
- <ticket-stem>[: <optional-label>]
  > Forward: <future-phase finding>

## Spec                                    # optional - omit when none
- <spec-stem>
```

When a spec heading `{#slug}` changes, include `renamed-spec: <old-stem> -> <new-stem>`.

### Context Window Discipline

- Source code is ground truth; load only docs relevant to the task.
- Update drifted docs on contact.

## Architecture Rules

<!-- Project-wide invariants the AI must never violate. -->

1. **Headless-testable state.** Keep table state and data operations testable without a display; egui rendering code should stay thin around framework-agnostic state behavior.
2. **egui version alignment.** Keep direct `egui`, `egui_extras`, and demo `eframe` dependencies on the same upstream minor version unless there is an explicit compatibility reason not to.

## Project Knowledge

- Project state and cross-session context live in `ai-docs/`.
- Workflow shape and plugin-less maintenance guidance live in `ai-docs/WORKFLOW.md`; it is explanatory and does not override ws runtime or MCP parser behavior.
- Before creating or editing tickets, load the write-ticket workflow skill for conventions.
- Reference tickets by stem only, never full path; stems survive status moves.
- Check `## Ticket Focus` in `ai-docs/_index.md` before starting implementation; it may include non-ready attention items, but only `ready/` entries are direct implementation targets.
- To check ticket completion or prior phase results, use `git log --grep=<ticket-stem>` and inspect `## Ticket Updates`.
- Claude Code compatibility is `CLAUDE.md` containing `@AGENTS.md`.
- **Language:** AI-authored docs, plans, commits, tickets, and code comments are English. Human-facing UI strings are exempt.

<!-- Inclusion test: if breaking this rule makes a skill produce wrong results
     AND it applies everywhere, keep it here. Domain-scoped rules belong in
     `ai-docs/mental-model/<domain>.md ## Domain Rules` via `ws:lead-add-rule`.
     Context goes in `_index.md`; process goes in skills. -->

<!-- Template Version: v0041 -->
