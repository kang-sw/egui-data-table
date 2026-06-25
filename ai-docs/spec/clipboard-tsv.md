---
title: Clipboard And TSV Interoperability
summary: Clipboard import/export, row codec, and TSV text behavior for table copy, cut, paste, and fill actions.
---

# Clipboard And TSV Interoperability

`egui-data-table` supports internal table clipboard operations and optional
system clipboard interchange through caller-provided row codecs. TSV text is the
observable interchange format when system clipboard import or export is
available.

## Codec Availability Contract {#260625-codec-availability-contract}

`RowViewer::try_create_codec(is_encoding)` controls whether table data can be
encoded to or decoded from system clipboard text.

When `is_encoding` is true, the renderer is requesting a codec for clipboard
export. When `is_encoding` is false, the renderer is requesting a codec for
clipboard import. The default implementation returns no codec, so callers must
provide a codec to enable system clipboard interchange.

## RowCodec Decode Encode Contract {#260625-row-codec-decode-encode-contract}

`RowCodec<R>` defines text conversion for row cells:

- `create_empty_decoded_row` creates the row value used as the decode target.
- `encode_column` returns text for one row column.
- `decode_column` writes text into one row column and returns success or a
  `DecodeErrorBehavior`.

Decode errors have three caller-visible outcomes:

- `Abort` rejects the whole import.
- `SkipCell` omits the failed cell.
- `SkipRow` drops the row being decoded.

## TSV Write Format {#260625-tsv-write-format}

System clipboard export writes selected table cells as TSV text. Cells in the
same row are separated by tab characters and rows are separated by newline
characters.

Cell text is escaped before writing:

- empty cell text is written as a single space;
- tab is written as `\t`;
- newline is written as `\n`;
- carriage return is written as `\r`;
- backslash is written as `\\`.

## TSV Parse Format {#260625-tsv-parse-format}

System clipboard import parses TSV-like text by splitting cells on tabs and rows
on newlines.

Raw carriage returns are ignored. Backslash escapes decode `\t`, `\n`, `\r`,
and `\\` to their corresponding characters. Unknown backslash escapes are
preserved as the escaped character rather than rejecting the text.

## Internal Clipboard Model {#260625-internal-clipboard-model}

Copy and cut capture selected cells into the table's internal clipboard. The
clipboard stores cloned row values and paste entries ordered by row and column
offset from the selected source area.

The internal clipboard is available for paste and fill behavior even when no
system clipboard codec exists.

## System Clipboard Export {#260625-system-clipboard-export}

When copy or cut runs and an encode codec is available, the table serializes the
selected cells to TSV and asks egui to copy the resulting text to the system
clipboard.

If no encode codec is available, copy and cut still update the internal
clipboard but do not produce system clipboard text.

## System Clipboard Import {#260625-system-clipboard-import}

When egui provides non-empty pasted text and a decode codec is available, the
table attempts to decode the text as TSV into the internal clipboard before
running paste behavior.

Import requires an applicable selection target and acceptable decoded width. If
decoding aborts or no decode codec is available, the existing internal clipboard
remains available for paste behavior.

During decode, `Abort` rejects the import, `SkipCell` omits the failed cell, and
`SkipRow` drops the row currently being decoded.

## Paste And Paste-Insert Placement {#260625-paste-and-paste-insert-placement}

Paste applies clipboard entries starting from the interactive row and selected
target column while preserving stored row and column offsets. The table selects
the affected pasted cells after applying the paste.

Paste-insert creates new rows from clipboard data and fills decoded or copied
columns into those rows. The renderer exposes paste-insert only when row
insertion is allowed.

## Fill Cut And Clear Data Behavior {#260625-fill-cut-and-clear-data-behavior}

Fill copies values from the interactive row across selected cells.

Cut first copies the selected cells into the clipboard, then clears the
selection. Clear writes deletion-default values into selected cells. Clear and
cut use the clear write context when applying deletion-default values.

## Write Confirmation For Clipboard Mutations {#260625-write-confirmation-for-clipboard-mutations}

Clipboard-driven mutations respect the same write controls as other table UI
writes. Paste, fill, and clear check cell editability and
`RowViewer::confirm_cell_write_by_ui` before mutating cells.

Committed clipboard mutations update row values through the viewer and trigger
the row update behavior.
