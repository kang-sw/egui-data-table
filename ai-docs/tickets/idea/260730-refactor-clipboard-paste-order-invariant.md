---
title: "Enforce the clipboard (row, column) ordering invariant instead of relying on construction order"
related-mental-model:
  - clipboard-tsv
---

# Enforce the clipboard (row, column) ordering invariant instead of relying on construction order

## Background

`Clipboard.pastes` must be ordered by `(row, column)`. `try_dump_clipboard_content`
consumes that ordering with `chunk_by` plus monotonically advancing row and
column cursors, and the source says so itself — search
`clipboard MUST be sorted before dumping` in `src/draw/state/clipboard.rs`, which
already asks "add assertion?".

Today the invariant holds only by construction: the copy/cut path builds from a
`BTreeSet` *and* calls `Clipboard::sort`, while the system-clipboard decode path
relies purely on `ParsedTsv::iter_rows()` emission order and never sorts. Violate
it and there is no panic — the cursors simply never emit the missing separator,
so cells from different rows or columns are concatenated into one TSV field.
Silent data corruption in copied text.

The codebase already uses `debug_assert!` for the analogous `RemoveRow` ordering
invariant, so the inconsistency is in rigor, not in style.

Found while forging `ai-docs/mental-model/clipboard-tsv.md`.

## Decisions

- Assert at the consumer, not only at the producers. Producers can be added; the
  dump path is the single place that must be able to trust the order.
- Match the existing `RemoveRow` precedent (`debug_assert!`) rather than
  inventing a new error path — this is an internal invariant, not user input.

## Constraints

- The system-clipboard decode path currently satisfies the invariant implicitly.
  Either call `Clipboard::sort` there too, or document that `iter_rows()`
  ordering is load-bearing — leaving it implicit is what makes a future change to
  the parser silently dangerous.

## Phases

### Phase 1: Assert and normalize clipboard ordering

Add the assertion at the dump path, make the decode path's ordering explicit, and
cover the export path with tests: multi-row multi-column copy, a copy with hidden
columns, and a ragged decode. `clipboard.rs` has no tests today.
