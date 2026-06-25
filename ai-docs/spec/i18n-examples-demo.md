---
title: Internationalization And Demos
summary: Translator behavior and observable demo/example behavior for the crate.
---

# Internationalization And Demos

`egui-data-table` includes a small translator interface for built-in labels and
example applications that demonstrate common table usage patterns in native and
wasm contexts.

## Translator API And Fallback {#260626-translator-api-and-fallback}

`Translator` maps string keys to display strings through `translate`.

Renderers use `EnglishTranslator` by default. Callers can replace the renderer
translator with `Renderer::with_translator`. The default English translator
returns English strings for known built-in keys and falls back to returning the
key itself when no translation is available.

## Translated Built-In Labels {#260626-translated-built-in-labels}

Built-in renderer labels go through the active translator. Header context-menu
labels and body context-menu labels use translation keys for actions such as
hide column, hidden columns, clear sort, copy, cut, clear, fill, paste,
paste-insert, duplicate row, delete row, undo, and redo.

## Internationalization Example {#260626-internationalization-example}

The internationalization example demonstrates custom translation by defining
hardcoded English and Spanish translators. Its language selector switches
between `en_US` and `es_ES`.

Changing language updates both the renderer translator and the viewer translator
used for column labels. The native entrypoint opens a centered window titled
`Translator demo`.

The example shows how richer translation systems can be adapted through the
`Translator` trait, but the crate does not provide locale-file loading,
pluralization, or interpolation APIs.

## Main Spreadsheet Demo {#260626-main-spreadsheet-demo}

The main demo presents a spreadsheet-like table with generated rows. It includes
name filtering, sortable columns except the student flag column, editable cells,
row-locked edit blocking, row-protection behavior for student rows, drag/drop
cell replacement, a hotkeys panel, theme controls, style toggles, row shuffling,
and a modification-flag clear UI.

The native entrypoint opens a centered window titled `Spreadsheet Demo`.

## Main Demo Codec Data Behavior {#260626-main-demo-codec-data-behavior}

The main demo supplies a row codec for clipboard behavior. It encodes and
decodes name, age, student flag, grade, row-locked flag, and optional gender.

Gender text accepts `Male`, `Female`, or blank/whitespace for no gender. Invalid
age, boolean, grade, or row-locked values skip the decoded row.

## Main Demo Events {#260626-main-demo-events}

The main demo logs highlight, highlight-change, row update, row insertion, and
row removal events with `log::trace`.

It also exposes a draggable payload. Dropping that payload onto an editable cell
creates a predefined replacement row value for that cell.

## Partially Editable Demo {#260626-partially-editable-demo}

The partially editable example exposes `Part` and `PartWithState` data types and
demonstrates row-specific edit permissions.

Manufacturer and MPN cells display read-only labels. The Processes column is
editable through checkboxes. Row insertion and deletion are disabled by default
and can be toggled from the example UI. The bottom panel displays the table
modification flag and provides a clear action.

The native entrypoint opens a centered window titled `Partially editable demo`.

## Wasm Demo Wrapper {#260626-wasm-demo-wrapper}

The `demo` crate builds the main spreadsheet demo from `examples/demo.rs` for
the hosted wasm demo.

The wasm entrypoint initializes web logging, locates the canvas with id
`the_canvas_id`, starts `eframe::WebRunner`, removes loading text after a
successful start, and replaces the loading text with a crash message before
panicking if startup fails.

`demo/index.html` provides the full-page canvas and loading shell used by Trunk
to run the wasm demo.
