---
domain: clipboard-tsv
description: "Internal clipboard ordering invariant, the system-clipboard bridge, and the TSV wire format"
sources:
  - src/draw/
related:
  command-undo: "copy/cut/paste actions build commands; CcUpdateSystemClipboard is intercepted before them"
  viewer-contract: "RowCodec is reached only via try_create_codec and is optional"
  render-pipeline: "egui paste events enter here from impl_show_body"
---

# Clipboard / TSV

## Entry Points

- `src/draw/state/clipboard.rs` — encode/decode against `RowCodec`, plus the
  design notes at the top of the file.
- `src/draw/tsv.rs` — the wire format: escaping writer and `ParsedTsv`.

## Module Contracts

- The internal `Clipboard<R>` holds a row slab plus a `pastes` list that **must
  be ordered by `(row, column)`**. `CopySelection`/`CutSelection` establish it
  twice over — `collect_selection` returns a `BTreeSet`, and the constructor
  still calls `Clipboard::sort` (`action.rs:86`). The system-clipboard decode
  path relies purely on `ParsedTsv::iter_rows()` emission order and calls no
  sort. {#260625-internal-clipboard-model}
- `try_dump_clipboard_content` consumes that invariant without checking it — the
  code says so itself (`clipboard.rs:138`: "clipboard MUST be sorted before
  dumping; XXX: add assertion?"). It groups with `chunk_by` and advances row and
  column cursors monotonically, so out-of-order input produces missing separators
  and silently concatenated cell values rather than a panic.
  {#260625-system-clipboard-export}
- Codec availability is the gate for the whole system-clipboard bridge: no
  `try_create_codec` means the table only ever pastes from its own internal
  clipboard (`clipboard.rs:45,140`). {#260625-codec-availability-contract}
  {#260625-row-codec-decode-encode-contract}
- TSV escaping covers `\t`, `\n`, `\r`, `\\` per character. Parsing drops
  unescaped `\r` (normalizing CRLF from external sources) and re-emits unknown
  escape sequences verbatim; `ParsedTsv::parse` has no failure mode.
  {#260625-tsv-write-format} {#260625-tsv-parse-format}
- Rows may be ragged. `iter_rows` yields exactly the cells present per row with
  no padding to `calc_table_width()`, so a ragged paste writes only the cells
  that exist rather than clearing the rest of the target rectangle — unlike
  typical spreadsheet paste semantics. {#260625-paste-and-paste-insert-placement}

## Coupling

- `body.rs:43-58` handles `Event::Paste` and **discards** the `bool` returned by
  `try_update_clipboard_from_string` (`body.rs:48`), then queues the paste action
  regardless. Every decode-failure branch leaves `self.clipboard` untouched, so a
  failed system paste silently pastes the previously copied internal content
  instead of failing or no-op'ing. {#260625-system-clipboard-import}
- `CopySelection`/`CutSelection` set `self.clipboard = None` *before* the
  empty-selection early return (`action.rs:55-61`), so copying with nothing
  selected discards the previous clipboard. Without a codec there is no system
  fallback to recover it.
- `Command::CcUpdateSystemClipboard` is produced only when
  `try_dump_clipboard_content` succeeds, and must be intercepted in
  `body.rs:578-582`; it is `unreachable!()` inside the command pipeline.
  {#260625-clipboard-command-behavior}
- Cell writes from paste/fill/clear are filtered by `is_editable_cell` and
  `confirm_cell_write_by_ui` inside `push_new_command`'s `CcSetCells` arm, not
  here. {#260625-write-confirmation-for-clipboard-mutations}
  {#260625-fill-cut-and-clear-data-behavior}

## Extension Points & Change Recipes

- **Change the wire format**: both directions live in `tsv.rs`; note that
  `write_content` substitutes a single space for an empty cell (`tsv.rs:14-17`),
  and the parser does not normalize it back. Round-tripping an empty selected
  cell through the system clipboard yields a one-space cell.
- **Add a clipboard producer**: sort `pastes` explicitly. Nothing downstream
  will tell you if you forget.
- **Change `iter_rows` ordering**: this silently breaks the export path; add the
  missing assertion first.

## Common Mistakes

- Treating the file-header design note as a specification. It describes empty
  cells as `""`-quoted (`clipboard.rs:29-31`); the implementation uses a space
  and no quoting at all. The note is aspirational.
- Assuming the `bool` from `try_update_clipboard_from_string` gates the paste.
- Assuming `RowCodec for ()`'s `unimplemented!()` bodies are unreachable by
  construction — they are unreachable only while `try_create_codec` returns
  `None`. Returning `Some(())` compiles and panics on first use.
- Assuming the column cursor arithmetic at `clipboard.rs:167-173` is defensive.
  It is monotonic-only: it emits separators while advancing forward and has no
  branch for going backwards, which is exactly why the unenforced sort invariant
  matters.

## Technical Debt

- **Column bounds check is off by one.** `clipboard.rs:97` rejects
  `col_idx > vis_cols.len()`, but the last valid index is `len() - 1`. A paste
  landing near the last column drives `col_idx` to exactly `len()`, which is then
  passed to `decode_column` and later to `set_cell_value`/`is_editable_cell` as an
  out-of-range column. Whether it panics depends on the viewer implementation.
  The earlier width guard (`clipboard.rs:76`) checks the raw pasted width only,
  not width plus selection offset.
- Precedence between a populated system clipboard and a populated internal
  clipboard is an acknowledged open question (`clipboard.rs:17-19`); the current
  "system wins if it decodes" behavior is a stopgap.
- Test coverage is one parser unit test (`tsv.rs:184-214`). Nothing exercises
  `clipboard.rs` at all — not the escaping round trip, not the
  `DecodeErrorBehavior` branches, not the bounds check above.
