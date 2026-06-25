# ws Workflow Guide

This guide is copied to `ai-docs/WORKFLOW.md` by bootstrap so a
maintainer can preserve the project shape when ws skills or MCP tools are not
available. It is an explanation and manual fallback only: editing this file does
not change MCP parser behavior, plugin/runtime semantics, ticket status logic,
spec indexing, or any other machine contract.

When this guide and installed ws tooling disagree, treat the installed plugin,
runtime, and bundled conventions as canonical. Update the upstream bootstrap
template rather than relying on a project-local guide override.

## Authority Files

- `AGENTS.md` is the canonical root workflow context for agents.
- `CLAUDE.md` exists only for Claude compatibility and should contain
  `@AGENTS.md` when the project has migrated to the host-neutral context.
- `ai-docs/WORKFLOW.md` is this pinned guide for plugin-less
  maintenance. Keep root context short; put durable project context in
  `ai-docs/_index.md` and workflow-system changes in upstream tooling.

## `ai-docs/` Layout

- `_index.md` is the session-start memory and active focus. Prune aggressively:
  completed work belongs in Git history, not the index.
- `_index.md` should keep compact orientation: summary, stack, workspace,
  conventions, build/test commands, operational pitfalls, current focus, and
  short session notes.
- `_index.local.md` is machine-local memory and should be ignored by Git.
- `tickets/` stores work by status directory: `idea/`, `todo/`, `ready/`,
  `.done/`, and `.dropped/`.
- `spec/` stores caller-visible behavior specs with stable stem anchors.
- `mental-model.md` stores the mental-model index and optional project reading
  map; `mental-model/` stores modification-relevant operational knowledge and
  domain rules.
- `ref/` stores static references that are not active workflow state.
- `.old/` stores tracked project archive material kept only as possible future
  reference and hidden from default listings.
- `WORKFLOW.md` is this human-readable fallback guide.

## Tickets

- Reference tickets by stem, never by path; stems stay stable when tickets move
  between status directories.
- `idea/` is rough intake, `todo/` is accepted backlog, and `ready/` is the
  spec-addressed implementation-ready status.
- `_index.md` `## Ticket Focus` lists selected active attention items. Only
  `ready/` entries are direct implementation targets; do not list `.done/` or
  `.dropped/` tickets there.
- Actionable tickets use `## Phases` with stable `### Phase N: <title>`
  headings. Research tickets may use freeform topic sections.
- After a phase has a `### Result` section, treat its plan text and existing
  result entries as frozen. Add later implementation tweaks as a
  `#### Edition` entry under that Result area.
- Move tickets with `git mv` when possible so history preserves status changes.

## Specs

- Specs describe caller-visible behavior, not implementation details that can
  change without changing behavior.
- Each behavior entry uses a stable `{#YYMMDD-slug}` anchor. The anchor stem is
  the identifier used in tickets, commits, and mental-model cross-references.
- Contract-first planned behavior uses `🚧` markers on headings or planned
  callouts. Other planned work stays in ticket `## Spec Impact` until
  implementation closeout. Remove the marker only after verifying the behavior
  is implemented.
- If stem-generation or duplicate-anchor tools are unavailable, choose a clear
  date-prefixed stem manually, search the spec tree for duplicates, and verify
  with ws tooling when it becomes available.

## Mental Models

- Mental models capture knowledge needed to safely modify the project: module
  contracts, coupling, extension recipes, common mistakes, and technical debt.
- The root `mental-model.md` may include a compact project reading map that
  routes task/topic intents to specs, mental-model docs, references, or lookup
  guidance. It must not become a current build inventory.
- Domain-scoped user rules belong in `## Domain Rules` inside the matching
  mental-model document, not in root `AGENTS.md`.
- If a domain has nested documents, read the parent `index.md` before any child
  document so inherited domain rules are visible.
- Include relevant spec stems in mental-model prose so future agents can trace
  operational guidance back to caller-visible behavior.

## Index Health

When candidates exist, bootstrap reports `_index.md` scope-drift candidates as
an advisory health note and asks whether to clean up now, defer cleanup, or
route semantic follow-up. The first pass reads `_index.md` only; it does not
load the full spec or mental-model corpus and does not move semantic content.

Common drift candidates:

- deep source trees, file-by-file roles, type listings, or implementation inventory;
- long behavior inventories that belong in specs or a linked "what works" doc;
- data-flow narratives, lifecycle descriptions, extension recipes, common
  mistakes, audit rules, or logging rules that belong in mental models;
- dependency API notes, archived design excerpts, or external-reference summaries;
- done/dropped ticket history, completed milestones, or stale session chronology;
- stable task/topic reading maps mixed into `_index.md`;
- long duplicated spec, mental-model, module, or ticket indexes.

When a maintainer approves `_index.md` cleanup:

1. Preserve the memory-policy comment.
2. Keep project summary, stack, top-level workspace, build/test commands,
   read-before-edit pointers, active inventory, `Ticket Focus`, and compact
   session notes.
3. Compact deep sections into links only when a clear owning document already
   exists.
4. Keep unique project direction, active priorities, and unresolved operational
   caveats in `_index.md`.
5. Do not author or semantically update specs, mental models, tickets, or refs
   during index cleanup.
6. Compact source-derived detail to source pointers, static material to
   `ai-docs/ref/` or API-doc pointers, work history to Git or ticket archives,
   and duplicated maps to start-here pointers.
7. Route deeper semantic work through the owning workflow: behavior to
   `ws:lead-forge-spec` or `ws:lead-write-spec`, modification knowledge to
   `ws:lead-forge-mental-model`, Ticket Focus membership, ordering, and
   readiness/status wording to `ws:lead-write-ticket`, and ambiguous direction
   to `ws:lead-discuss`.

## Commit Traceability

- Every AI-authored commit should include `## AI Context` explaining why the
  approach was chosen and what alternatives or constraints mattered.
- Ticket-driven commits may include `## Ticket Updates` with forward-facing
  findings for future phases.
- Behavior-changing commits should include `## Spec` entries naming affected
  spec stems. If a spec anchor is renamed, record
  `renamed-spec: <old-stem> -> <new-stem>`.

## Manual Fallback

When ws skills, MCP tools, or Claude compatibility commands are unavailable:

1. Read `AGENTS.md`, `_index.md`, this guide, and the relevant current docs.
2. Use existing nearby tickets, specs, and mental models as formatting examples.
3. Prefer conservative, append-only changes when parser behavior is uncertain.
4. Keep generated AI docs and commit messages in English unless a human-facing
   product string requires another language.
5. Verify with plain Git and shell commands, then re-run ws verification tools
   when they become available.
