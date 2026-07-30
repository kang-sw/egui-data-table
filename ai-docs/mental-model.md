# Mental Model Index

`egui-data-table` is a single library crate plus a wasm demo wrapper. All
modification-relevant knowledge is organized by the layer a change lands in:
the consumer surface, the user-implemented viewer contract, the per-frame
rendering pipeline, the cached table state, the command/undo pipeline, and two
cross-cutting concerns (clipboard, persistence).

## Domains

| Domain | Covers | Read it before touching |
|---|---|---|
| [public-api](mental-model/public-api.md) | `DataTable` ownership, `Renderer` builder, the two dirty flags | `src/lib.rs`, the builder methods, anything a consumer calls |
| [viewer-contract](mental-model/viewer-contract.md) | `RowViewer`/`RowCodec` invariants, `DataModelOps` narrowing | `src/viewer.rs`, `src/draw/state/model_ops.rs` |
| [render-pipeline](mental-model/render-pipeline.md) | per-frame call order, egui_extras coupling, styling and translation | `src/draw/mod.rs`, `src/draw/body.rs` |
| [table-state](mental-model/table-state.md) | coordinate systems, `cc_*` caches, `validate_cc` rebuild | `src/draw/state/{mod,types,validation,selection}.rs` |
| [command-undo](mental-model/command-undo.md) | `Command` protocol, undo/redo replay, permission gating | `src/draw/state/{command,action}.rs` |
| [clipboard-tsv](mental-model/clipboard-tsv.md) | internal clipboard ordering, system bridge, TSV format | `src/draw/state/clipboard.rs`, `src/draw/tsv.rs` |
| [persistence](mental-model/persistence.md) | the `persistency` feature and `PersistData` | anything touching `vis_cols`/`sort` or the feature gate |
| [demo-examples](mental-model/demo-examples.md) | examples as reference impls, wasm demo build | `examples/`, `demo/`, the Pages workflow |

## Cross-Domain Patterns

These hold across the whole crate; each domain doc assumes them.

- **One mutation gate.** Every data or layout change goes through
  `push_new_command`. Direct mutation of `UiState` from rendering code skips
  cache invalidation, interactive-cell repair and undo history. The two
  deliberate exceptions are `Renderer::new`'s seed row and the validation path.
- **Visible vs total columns.** Linear cell indices always use
  `p.vis_cols.len()` as the stride, never `p.num_columns`. See `table-state` for
  the one place that violates this.
- **Permission hooks run at construction, never at apply.** `cmd_apply` — shared
  by first application and undo/redo replay — performs no checks at all.
- **`cc_dirty` is consumed, not observed.** `validate_cc` clears it on entry; any
  consumer that needs it (persistence saving) must run first.
- **Compile-time headless split.** State logic takes `impl DataModelOps<R>`, so
  it cannot reach egui-dependent viewer methods. Keep new state logic on that
  side of the line — it is what makes `state/tests.rs` possible.
- **Ordering invariants are `debug_assert`-only or unenforced.** `RemoveRow`
  requires ascending row indices; the clipboard requires `(row, column)` order.
  Both fail silently in release builds.

## Project Reading Map

| Task or topic | Read first |
|---|---|
| Consumer-facing API change | `ai-docs/spec/public-api.md`, then `public-api` |
| New or changed `RowViewer` hook | `viewer-contract`, then `demo-examples` for which example must be updated |
| Rendering, styling, hotkey or context-menu work | `ai-docs/spec/rendering-presentation.md`, then `render-pipeline` |
| Selection, sorting, filtering, cache bugs | `ai-docs/spec/persistence-validation.md`, then `table-state` |
| Editing, undo/redo, row operations | `ai-docs/spec/editing-selection-commands.md`, then `command-undo` |
| Copy/paste or TSV work | `ai-docs/spec/clipboard-tsv.md`, then `clipboard-tsv` |
| Feature-gate or persisted-state work | `persistence` |
| Demo, examples, wasm or CI work | `ai-docs/spec/i18n-examples-demo.md`, then `demo-examples` |
