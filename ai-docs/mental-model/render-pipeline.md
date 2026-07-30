---
domain: render-pipeline
description: "The per-frame call-order contract inside impl_show / impl_show_body and its coupling to egui_extras"
sources:
  - src/draw/
related:
  table-state: "validate_cc must run at a specific point in the frame or caches desync"
  command-undo: "every mutation this pipeline observes is queued and applied at the end of the frame"
  viewer-contract: "rendering callbacks are the egui-dependent half of RowViewer"
  persistence: "validate_persistency must precede validate_cc to see cc_dirty"
---

# Render Pipeline

## Entry Points

- `src/draw/mod.rs:155-359` — `impl_show`: column setup, header, `TableBuilder`.
- `src/draw/body.rs:5-609` — `impl_show_body`: input, rows, command application.

## Module Contracts

The per-frame order is a hard contract encoded only in file layout
{#260625-renderer-widget-lifecycle}:

1. `Renderer::new` → `validate_identity` (`state/validation.rs:4-55`). This is
   also where the deferred-sort frame counter advances.
2. `impl_show` builds **columns and header** from `s.vis_cols()` / `s.sort()`.
3. Inside `.body(...)`: hotkey and clipboard-event consumption
   (`body.rs:27-74`), then `validate_persistency`, then `validate_cc`
   (`body.rs:77-84`), then the per-row render closure, then selection commit,
   then focus update, then `try_apply_ui_action`, then the command loop
   (`body.rs:578-605`).

Consequences that break silently if the order is "cleaned up":

- `validate_persistency` **must** precede `validate_cc`, because `validate_cc`
  consumes `cc_dirty` (`validation.rs:86`) and the persistence save branch is
  gated on observing it (`validation.rs:79`). Swap them and saving nearly stops
  happening. {#260626-persisted-ui-state-behavior}
- Hotkey gating reads `cci_has_focus` and `is_editing()` as they stood at the
  **end of the previous frame** — `cci_has_focus` is only recomputed at
  `body.rs:543-557`. Anything that must react to an edit started this frame has
  to sit after the render loop, like the existing `edit_started` flag.
  {#260625-keyboard-and-mouse-action-routing}
- The header is built before `validate_persistency` runs, so on the very first
  frame of a persisted table the header shows the default column layout while
  the row order already reflects the loaded sort. It corrects on the next
  repaint. {#260625-column-and-header-presentation}
- `cc_row_heights` is *taken out* of `UiState` for the duration of rendering
  (`body.rs:89`, restored at `body.rs:560-568`); the field carries an explicit
  warning at `state/mod.rs:74-75`. Reading `s.cc_row_heights` inside the render
  closure yields an empty vec, not stale data.

## Coupling

- `egui_extras::TableBuilder` virtualizes **rows only** — the header and body
  closures themselves run every frame. `cci_page_row_count` therefore counts
  only rows rendered this frame and is deliberately used as the page size for
  `NavPageUp`/`NavPageDown` (`state/action.rs:243-244`), and the editor window
  exists only while its row is inside the virtualized range — which is why every
  `UiAction` sets `cci_want_move_scroll` and `impl_show` re-issues
  `scroll_to_row` (`draw/mod.rs:189-192`). {#260625-table-layout-and-scroll-behavior}
- `Renderer` never calls `TableBuilder::id_salt`, and `ui_id = ui.id()`
  (`draw/mod.rs:157`) simultaneously keys the persistence blob, the cell editor
  windows (`body.rs:478`) and — via egui_extras' fixed salt — column widths and
  scroll offset. Two sibling tables in one `Ui` collide on all of them at once.
- Column drag-reorder uses egui's DnD payload typed as `VisColumnPos`
  (`draw/mod.rs:251,302-310`). Payloads match by **type**, so a drag between two
  tables on screen delivers a structurally valid but wrong index; `command.rs:85-92`
  bounds-checks it defensively and says so in a comment.
- Header-originated commands and body-originated commands share one
  `Vec<Command<R>>` and are applied together in the single loop at the end of
  `impl_show_body`.

## Extension Points & Change Recipes

- **Add an interactive widget in a cell**: it will not receive clicks.
  `show_cell_view` is invoked inside `ui.add_enabled_ui(false, ...)`
  (`body.rs:253`) — deliberately, per the FIXME at `body.rs:249-252` describing
  the egui 0.27+ interaction interception. Route interaction through
  `on_cell_view_response` instead. {#260625-cell-display-and-editor-presentation}
- **Hit-test a cell**: use the manually expanded `drop_area_rect` pattern
  (`body.rs:450-453`), not `resp.contains_pointer()` — see the FIXME at
  `body.rs:445-448` about egui 0.29 response-rect sizing.
  {#260625-drag-drop-response-forwarding}
- **Add a context-menu entry or any UI string**: add both the `translate("key")`
  call site and the matching arm in `EnglishTranslator` (`draw/mod.rs:377-403`).
  There is no compile-time link; an unknown key silently renders as the raw key
  string. {#260625-translated-presentation-labels}
- **Add a hotkey-driven action**: a `UiAction` reaches `try_apply_ui_action` only
  if something produces it — a shortcut match at `body.rs:67-73` or a
  context-menu push at `body.rs:431`. There is no registry.
  {#260625-context-menu-and-keyboard-event-surface}
- **Style**: all selection/focus visuals come from `Style` fields consumed in
  `body.rs:200-284`; reuse them rather than adding ad-hoc colors.
  {#260625-style-driven-presentation} {#260625-selection-and-focus-visuals}

## Common Mistakes

- Mutating `UiState` directly from rendering code instead of queueing a
  `Command` → skips `cc_dirty` bookkeeping, `validate_interactive_cell`, and
  undo history.
- Assuming "double-click to edit" uses `Response::double_clicked()`. It does not:
  edit starts when the click lands on a cell that was already the interactive
  cell from a previous frame's selection commit (`body.rs:300-313`), or
  immediately if `single_click_edit_mode` is set.
- Reading `cc_row_heights` during rendering.
- Assuming committing an edit only affects this table — `CcCommitEdit` calls
  `surrender_focus` on the whole `egui::Context` (`body.rs:584-591`).
- Rendering several tables in a loop without `ui.push_id`.

## Technical Debt

- The comment at `body.rs:82-84` justifies deferring validation because the body
  closure "may not be called if the table area is out of the visible space". With
  the pinned `egui_extras` 0.34 the body closure does run every frame; only rows
  are virtualized. Treat the comment as stale rather than as a guarantee to build
  on.
- `draw/mod.rs:204-206` reserves the leftmost header cell for a planned
  "Configure Sorting" button; it renders nothing today.
- No `id_salt` builder exists for multi-table pages.
- Two egui-version workarounds (`body.rs:249`, `body.rs:445`) are pinned to
  observed behavior, not documented API — re-verify both on every egui bump.
