---
title: "Persisted UI state has no schema guard and reapplies stale column indices"
related-mental-model:
  - persistence
---

# Persisted UI state has no schema guard and reapplies stale column indices

## Background

`PersistData` carries the column count, visible-column list and sort config, with
no version field and no serde defaults. Two silent failure shapes follow:

1. **Schema change wipes state silently.** Add or rename a field and previously
   saved blobs fail to deserialize; `get_persisted(..).unwrap_or_default()`
   substitutes an empty `PersistData`, whose zero column count then fails the
   load guard, so the table quietly starts from defaults. Nothing in this crate
   logs or surfaces it.
2. **Column count is the only identity check.** `vis_cols` and `sort` are
   positional indices, and the guard only compares the count (search
   `Data should only be copied when column count matches` in
   `src/draw/state/validation.rs`). Reorder or swap columns while keeping the
   count and the old layout is reapplied to different columns — sorting by a
   different column than the user chose, with no error.

Found while forging `ai-docs/mental-model/persistence.md`. The feature has had no
CHANGELOG entry since it landed in 0.2.2, and there is no test coverage at all.

## Decisions

- Both problems are the same missing concept — a schema/identity guard — so
  solve them together rather than patching only the serde side.

## Constraints

- Any guard must degrade to "discard persisted state", never to an error the host
  app has to handle: persistence is an optional convenience.
- Column identity cannot be derived from `RowViewer` today without adding a trait
  method (`column_name` exists but is `egui`-side and free-form). Decide whether
  a stable per-column key is worth a trait addition, or whether a schema version
  plus the existing count check is the honest limit — and record why.
- Adding a required trait method is a breaking change for implementers; a
  defaulted method is not. Prefer the defaulted shape.
- Changing the serialized shape invalidates existing user data once. That is
  acceptable for a guard that prevents silently wrong state, but say so in the
  Result and CHANGELOG.

## Phases

### Phase 1: Add a schema guard for persisted UI state

Introduce a version (or equivalent identity) that makes an incompatible blob
discard cleanly, give every field a serde default so future additions do not
invalidate old data, and settle the column-identity question above. Add the
first tests for this path, covering: a matching load, a count mismatch, an
incompatible schema, and a sort entry whose column is no longer sortable.
