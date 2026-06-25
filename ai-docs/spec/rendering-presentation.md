---
title: Rendering And Presentation
summary: Observable rendering, layout, styling, and input presentation behavior for egui data tables.
---

# Rendering And Presentation

`egui-data-table` renders a `DataTable<R>` as an egui widget. This spec covers
the visible table presentation and renderer-facing configuration, not the full
semantics of table commands or clipboard encoding.

## Renderer Widget Lifecycle {#260625-renderer-widget-lifecycle}

`Renderer` is an `egui::Widget`. Callers construct it from a mutable
`DataTable<R>` and a mutable `RowViewer<R>`, then either add it to egui through
`Ui::add` or call `show`.

Creating a renderer initializes or validates the table UI state for the current
viewer. When the table is empty and the viewer allows row insertion, renderer
construction inserts an initial empty row.

`show` renders the table and returns an `egui::Response`. When the renderer is
dropped, it writes the current UI state back into the `DataTable<R>`.

## Renderer Builder Configuration {#260625-renderer-builder-configuration}

The renderer exposes builder-style methods for presentation and runtime
configuration:

- `with_style` replaces the renderer style.
- `with_style_modify` mutates the current style in place.
- `with_table_row_height` sets fixed row height behavior.
- `with_max_undo_history` sets the undo history limit.
- `with_translator` replaces the translator used for built-in labels.

Each method returns the renderer so callers can chain configuration before
rendering.

## Style-Driven Presentation {#260625-style-driven-presentation}

`Style` controls the table's presentation. Callers can configure selected and
highlighted cell colors, selected-highlight foreground color, drag-selection
foreground color, undo history size, fixed or heterogeneous row height,
single-click edit mode, cell alignment, focused-row stroke, scroll auto-shrink,
and scrollbar visibility.

The renderer applies these options when building the table, painting selection
and focus affordances, opening editors, and configuring egui scroll behavior.

## Column And Header Presentation {#260625-column-and-header-presentation}

Column headers are driven by `RowViewer` column hooks. The viewer supplies
column names, per-column render configuration, and sortability.

The header displays visible columns, keeps hidden columns available through the
header context menu, and shows sortable state with a stable sort-indicator area.
Primary-clicking a sortable header cycles its sort state. Dragging a header can
reorder columns.

The header context menu lets callers hide or show columns and clear sorting
through built-in table commands.

## Table Layout And Scroll Behavior {#260625-table-layout-and-scroll-behavior}

The table is rendered inside a horizontal scroll area. Its layout includes a row
header column, the currently visible table columns, and placeholder capacity for
hidden columns.

The body uses striped rows, configured cell alignment, a bounded maximum scroll
height, no drag-to-scroll behavior, and the style-configured auto-shrink and
scrollbar visibility settings.

## Cell Display And Editor Presentation {#260625-cell-display-and-editor-presentation}

Read-only cell content is displayed through `RowViewer::show_cell_view`.

When a cell enters edit mode, the renderer uses `RowViewer::show_cell_editor` in
a floating editor window. The active editing cell suppresses its normal
read-only display so the editor is the visible editing surface.

Cell editing begins only for editable cells. With single-click edit mode
enabled, a primary click can start editing immediately; otherwise the cell must
already be the interactive cell.

## Selection And Focus Visuals {#260625-selection-and-focus-visuals}

The renderer paints selected cells, the current interactive cell, drag-selection
outlines, focused-row strokes, and row editing background according to style and
state.

For heterogeneous row heights, the renderer measures row content, updates the
cached row height, and requests repaint when a measured height changes.

## Context Menu And Keyboard Event Surface {#260625-context-menu-and-keyboard-event-surface}

The table body exposes built-in context menu entries for visible table actions
such as copy, cut, clear, fill, paste, paste-insert, duplicate row, delete row,
undo, and redo. Entries are shown or enabled according to the current table
state and viewer permissions.

The renderer also consumes egui copy, cut, and paste events and maps
viewer-provided keyboard shortcuts to table actions. This spec describes the
presentation/input surface; command effects are specified by the editing and
commands behavior.

## Drag-Drop Response Forwarding {#260625-drag-drop-response-forwarding}

For each displayed cell, the renderer forwards the cell response to
`RowViewer::on_cell_view_response`. If the viewer returns a replacement row
value from that callback, the renderer applies the value only when the target
cell is editable.

## Translated Presentation Labels {#260625-translated-presentation-labels}

Built-in presentation labels, including context-menu labels, are translated
through the renderer's `Translator`.

`EnglishTranslator` provides the default English labels for known keys and
falls back to returning the key string when a translation is not available.
Callers can replace the translator with `with_translator`.

## Clipboard Presentation Bridge {#260625-clipboard-presentation-bridge}

The renderer bridges table clipboard actions to egui system clipboard events.
When table commands produce clipboard text, the renderer asks egui to copy that
text. When egui provides pasted text and the viewer can create a decode codec,
the renderer imports the pasted text into table clipboard state.

The text format and codec behavior are specified by the clipboard and TSV
interoperability behavior.
