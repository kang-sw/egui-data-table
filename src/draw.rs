use std::mem::{replace, take};

use egui::{
    Align, Color32, Event, Layout, PointerButton, Rect, Response, RichText, Sense, Stroke, Widget,
};
use egui_extras::Column;
use tap::prelude::{Pipe, Tap};

use crate::{
    viewer::{EmptyRowCreateContext, RowViewer},
    DataTable, UiAction,
};

use self::state::*;

use format as f;

pub(crate) mod state;
mod tsv;

/* -------------------------------------------- Style ------------------------------------------- */

/// Style configuration for the table.
// TODO: Implement more style configurations.
#[derive(Default, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Style {
    /// Background color override for selection. Default uses `visuals.selection.bg_fill`.
    pub bg_selected_cell: Option<egui::Color32>,

    /// Background color override for selected cell. Default uses `visuals.selection.bg_fill`.
    pub bg_selected_highlight_cell: Option<egui::Color32>,

    /// Foreground color for cells that are going to be selected when mouse is dropped.
    pub fg_drag_selection: Option<egui::Color32>,

    /* ·························································································· */
    /// Maximum number of undo history. This is applied when actual action is performed.
    pub max_undo_history: usize,

    /// If specify this as [`None`], the heterogeneous row height will be used.
    pub table_row_height: Option<f32>,

    /// When enabled, single click on a cell will start editing mode. Default is `false` where
    /// double action(click 1: select, click 2: edit) is required.
    pub single_click_edit_mode: bool,
}

/* ------------------------------------------ Rendering ----------------------------------------- */

pub struct Renderer<'a, R, V: RowViewer<R>> {
    table: &'a mut DataTable<R>,
    viewer: &'a mut V,
    state: Option<Box<UiState<R>>>,
    style: Style,
}

impl<R, V: RowViewer<R>> egui::Widget for Renderer<'_, R, V> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        self.show(ui)
    }
}

impl<'a, R, V: RowViewer<R>> Renderer<'a, R, V> {
    pub fn new(table: &'a mut DataTable<R>, viewer: &'a mut V) -> Self {
        if table.rows.is_empty() {
            table.push(viewer.new_empty_row_for(EmptyRowCreateContext::InsertNewLine));
        }

        Self {
            state: Some(table.ui.take().unwrap_or_default().tap_mut(|state| {
                state.validate_identity(viewer);
            })),
            table,
            viewer,
            style: Default::default(),
        }
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn with_style_modify(mut self, f: impl FnOnce(&mut Style)) -> Self {
        f(&mut self.style);
        self
    }

    pub fn with_table_row_height(mut self, height: f32) -> Self {
        self.style.table_row_height = Some(height);
        self
    }

    pub fn with_max_undo_history(mut self, max_undo_history: usize) -> Self {
        self.style.max_undo_history = max_undo_history;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> Response {
        egui::ScrollArea::horizontal()
            .show(ui, |ui| self.impl_show(ui))
            .inner
    }

    fn impl_show(mut self, ui: &mut egui::Ui) -> Response {
        let ctx = &ui.ctx().clone();
        let ui_id = ui.id();
        let style = ui.style().clone();
        let painter = ui.painter().clone();
        let visual = &style.visuals;
        let viewer = &mut *self.viewer;
        let s = self.state.as_mut().unwrap();
        let mut resp_total = None::<Response>;
        let mut resp_ret = None::<Response>;
        let mut commands = Vec::<Command<R>>::new();
        let ui_layer_id = ui.layer_id();

        // NOTE: unlike RED and YELLOW which can be acquirable through 'error_bg_color' and
        // 'warn_bg_color', there's no 'green' color which can be acquired from inherent theme.
        // Following logic simply gets 'green' color from current background's brightness.
        let green = if visual.window_fill.g() > 128 {
            Color32::DARK_GREEN
        } else {
            Color32::GREEN
        };

        let mut builder = egui_extras::TableBuilder::new(ui).column(Column::auto());

        let iter_vis_cols_with_flag = s
            .vis_cols()
            .iter()
            .enumerate()
            .map(|(index, column)| (column, index + 1 == s.vis_cols().len()));

        for (column, flag) in iter_vis_cols_with_flag {
            builder = builder.column(viewer.column_render_config(column.0, flag));
        }

        if replace(&mut s.cci_want_move_scroll, false) {
            let interact_row = s.interactive_cell().0;
            builder = builder.scroll_to_row(interact_row.0, None);
        }

        builder
            .columns(Column::auto(), s.num_columns() - s.vis_cols().len())
            .drag_to_scroll(false) // Drag is used for selection;
            .striped(true)
            .max_scroll_height(f32::MAX)
            .sense(Sense::click_and_drag().tap_mut(|s| s.focusable = true))
            .header(20., |mut h| {
                h.col(|_ui| {
                    // TODO: Add `Configure Sorting` button
                });

                let has_any_hidden_col = s.vis_cols().len() != s.num_columns();

                for (vis_col, &col) in s.vis_cols().iter().enumerate() {
                    let vis_col = VisColumnPos(vis_col);
                    let mut painter = None;
                    let (col_rect, resp) = h.col(|ui| {
                        ui.horizontal_centered(|ui| {
                            if let Some(pos) = s.sort().iter().position(|(c, ..)| c == &col) {
                                let is_asc = s.sort()[pos].1 .0 as usize;

                                ui.colored_label(
                                    [green, Color32::RED][is_asc],
                                    RichText::new(format!("{}{}", ["↘", "↗"][is_asc], pos + 1,))
                                        .monospace(),
                                );
                            } else {
                                ui.monospace(" ");
                            }

                            egui::Label::new(viewer.column_name(col.0))
                                .selectable(false)
                                .ui(ui);
                        });

                        painter = Some(ui.painter().clone());
                    });

                    // Set drag payload for column reordering.
                    resp.dnd_set_drag_payload(vis_col);

                    if resp.dragged() {
                        egui::popup::show_tooltip_text(
                            ctx,
                            ui_layer_id,
                            "_EGUI_DATATABLE__COLUMN_MOVE__".into(),
                            viewer.column_name(col.0),
                        );
                    }

                    if resp.hovered() && viewer.is_sortable_column(col.0) {
                        if let Some(p) = &painter {
                            p.rect_filled(
                                col_rect,
                                egui::Rounding::ZERO,
                                visual.selection.bg_fill.gamma_multiply(0.2),
                            );
                        }
                    }

                    if viewer.is_sortable_column(col.0) && resp.clicked_by(PointerButton::Primary) {
                        let mut sort = s.sort().to_owned();
                        match sort.iter_mut().find(|(c, ..)| c == &col) {
                            Some((_, asc)) => match asc.0 {
                                true => asc.0 = false,
                                false => sort.retain(|(c, ..)| c != &col),
                            },
                            None => {
                                sort.push((col, IsAscending(true)));
                            }
                        }

                        commands.push(Command::SetColumnSort(sort));
                    }

                    if resp.dnd_hover_payload::<VisColumnPos>().is_some() {
                        if let Some(p) = &painter {
                            p.rect_filled(
                                col_rect,
                                egui::Rounding::ZERO,
                                visual.selection.bg_fill.gamma_multiply(0.5),
                            );
                        }
                    }

                    if let Some(payload) = resp.dnd_release_payload::<VisColumnPos>() {
                        commands.push(Command::CcReorderColumn {
                            from: *payload,
                            to: vis_col
                                .0
                                .pipe(|v| v + (payload.0 < v) as usize)
                                .pipe(VisColumnPos),
                        })
                    }

                    resp.context_menu(|ui| {
                        if ui.button("Hide").clicked() {
                            commands.push(Command::CcHideColumn(col));
                            ui.close_menu();
                        }

                        if !s.sort().is_empty() && ui.button("Clear Sort").clicked() {
                            commands.push(Command::SetColumnSort(Vec::new()));
                            ui.close_menu();
                        }

                        if has_any_hidden_col {
                            ui.separator();
                            ui.label("Hidden");

                            for col in (0..s.num_columns()).map(ColumnIdx) {
                                if !s.vis_cols().contains(&col)
                                    && ui.button(viewer.column_name(col.0)).clicked()
                                {
                                    commands.push(Command::CcShowColumn {
                                        what: col,
                                        at: vis_col,
                                    });
                                    ui.close_menu();
                                }
                            }
                        }
                    });
                }

                // Account for header response to calculate total response.
                resp_total = Some(h.response());
            })
            .tap_mut(|table| {
                table.ui_mut().separator();
            })
            .body(|body: egui_extras::TableBody<'_>| {
                resp_ret = Some(
                    self.impl_show_body(body, painter, commands, ctx, &style, ui_id, resp_total),
                );
            });

        resp_ret.unwrap_or_else(|| ui.label("??"))
    }

    #[allow(clippy::too_many_arguments)]
    fn impl_show_body(
        &mut self,
        body: egui_extras::TableBody<'_>,
        mut _painter: egui::Painter,
        mut commands: Vec<Command<R>>,
        ctx: &egui::Context,
        style: &egui::Style,
        ui_id: egui::Id,
        mut resp_total: Option<Response>,
    ) -> Response {
        let viewer = &mut *self.viewer;
        let s = self.state.as_mut().unwrap();
        let table = &mut *self.table;
        let visual = &style.visuals;
        let visible_cols = s.vis_cols().clone();
        let no_rounding = egui::Rounding::ZERO;

        let mut actions = Vec::<UiAction>::new();
        let mut edit_started = false;
        let hotkeys = viewer.hotkeys(&s.ui_action_context());

        // Preemptively consume all hotkeys.
        'detect_hotkey: {
            // Detect hotkey inputs only when the table has focus. While editing, let the
            // editor consume input.
            if !s.cci_has_focus {
                break 'detect_hotkey;
            }

            if !s.is_editing() {
                ctx.input_mut(|i| {
                    i.events.retain(|x| {
                        match x {
                            Event::Copy => actions.push(UiAction::CopySelection),
                            Event::Cut => actions.push(UiAction::CutSelection),

                            // Try to parse clipboard contents and detect if it's compatible
                            // with cells being pasted.
                            Event::Paste(clipboard) => {
                                if !clipboard.is_empty() {
                                    // If system clipboard is not empty, try to update the internal
                                    // clipboard with system clipboard content before applying
                                    // paste operation.
                                    s.try_update_clipboard_from_string(viewer, clipboard);
                                }

                                if i.modifiers.shift {
                                    actions.push(UiAction::PasteInsert)
                                } else {
                                    actions.push(UiAction::PasteInPlace)
                                }
                            }

                            _ => return true,
                        }
                        false
                    })
                });
            }

            for (hotkey, action) in &hotkeys {
                ctx.input_mut(|inp| {
                    if inp.consume_shortcut(hotkey) {
                        actions.push(*action);
                    }
                })
            }
        }

        // Validate persistency state.
        #[cfg(feature = "persistency")]
        if viewer.persist_ui_state() {
            s.validate_persistency(ctx, ui_id, viewer);
        }

        // Validate ui state. Defer this as late as possible; since it may not be
        // called if the table area is out of the visible space.
        s.validate_cc(&mut table.rows, viewer);

        // Checkout `cc_rows` to satisfy borrow checker. We need to access to
        // state mutably within row rendering; therefore, we can't simply borrow
        // `cc_rows` during the whole logic!
        let cc_row_heights = take(&mut s.cc_row_heights);

        let mut row_height_updates = Vec::new();
        let vis_row_digits = s.cc_rows.len().max(1).ilog10();
        let row_id_digits = table.len().max(1).ilog10();

        let body_max_rect = body.max_rect();
        let has_any_sort = !s.sort().is_empty();

        let pointer_interact_pos = ctx.input(|i| i.pointer.latest_pos().unwrap_or_default());
        let pointer_primary_down = ctx.input(|i| i.pointer.button_down(PointerButton::Primary));

        s.cci_page_row_count = 0;

        /* ----------------------------- Primary Rendering Function ----------------------------- */
        // - Extracted as a closure to differentiate behavior based on row height
        //   configuration. (heterogeneous or homogeneous row heights)

        let render_fn = |mut row: egui_extras::TableRow| {
            s.cci_page_row_count += 1;

            let vis_row = VisRowPos(row.index());
            let row_id = s.cc_rows[vis_row.0];
            let prev_row_height = cc_row_heights[vis_row.0];

            let mut row_elem_start = Default::default();

            // Check if current row is edition target
            let edit_state = s.row_editing_cell(row_id);
            let mut editing_cell_rect = Rect::NOTHING;
            let interactive_row = s.is_interactive_row(vis_row);

            let check_mouse_dragging_selection = {
                let s_cci_has_focus = s.cci_has_focus;
                let s_cci_has_selection = s.has_cci_selection();

                move |rect: &Rect, resp: &egui::Response| {
                    let cci_hovered: bool = s_cci_has_focus
                        && s_cci_has_selection
                        && rect
                            .with_max_x(resp.rect.right())
                            .contains(pointer_interact_pos);
                    let sel_drag = cci_hovered && pointer_primary_down;
                    let sel_click = !s_cci_has_selection && resp.hovered() && pointer_primary_down;

                    sel_drag || sel_click
                }
            };

            /* -------------------------------- Header Rendering -------------------------------- */

            // Mark row background filled if being edited.
            row.set_selected(edit_state.is_some());

            // Render row header button
            let (head_rect, head_resp) = row.col(|ui| {
                // Calculate the position where values start.
                row_elem_start = ui.max_rect().right_top();

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.separator();

                    if has_any_sort {
                        ui.monospace(
                            RichText::from(f!(
                                "{:·>width$}",
                                row_id.0,
                                width = row_id_digits as usize
                            ))
                            .strong(),
                        );
                    } else {
                        ui.monospace(
                            RichText::from(f!("{:>width$}", "", width = row_id_digits as usize))
                                .strong(),
                        );
                    }

                    ui.monospace(
                        RichText::from(f!(
                            "{:·>width$}",
                            vis_row.0 + 1,
                            width = vis_row_digits as usize
                        ))
                        .weak(),
                    );
                });
            });

            if check_mouse_dragging_selection(&head_rect, &head_resp) {
                s.cci_sel_update_row(vis_row);
            }

            /* -------------------------------- Columns Rendering ------------------------------- */

            // Overridable maximum height
            let mut new_maximum_height = 0.;

            // Render cell contents regardless of the edition state.
            for (vis_col, col) in visible_cols.iter().enumerate() {
                let vis_col = VisColumnPos(vis_col);
                let linear_index = vis_row.linear_index(visible_cols.len(), vis_col);
                let selected = s.is_selected(vis_row, vis_col);
                let cci_selected = s.is_selected_cci(vis_row, vis_col);
                let is_editing = edit_state.is_some();
                let is_interactive_cell = interactive_row.is_some_and(|x| x == vis_col);
                let mut response_consumed = s.is_editing();

                let (rect, resp) = row.col(|ui| {
                    let ui_max_rect = ui.max_rect();

                    if cci_selected {
                        ui.painter().rect_stroke(
                            ui_max_rect,
                            no_rounding,
                            Stroke {
                                width: 2.,
                                color: self
                                    .style
                                    .fg_drag_selection
                                    .unwrap_or(visual.selection.bg_fill),
                            },
                        );
                    }

                    if is_interactive_cell {
                        ui.painter().rect_filled(
                            ui_max_rect.expand(2.),
                            no_rounding,
                            self.style
                                .bg_selected_highlight_cell
                                .unwrap_or(visual.selection.bg_fill),
                        );
                    } else if selected {
                        ui.painter().rect_filled(
                            ui_max_rect.expand(1.),
                            no_rounding,
                            self.style
                                .bg_selected_cell
                                .unwrap_or(visual.selection.bg_fill.gamma_multiply(0.5)),
                        );
                    }

                    // Actual widget rendering happens within this line.

                    // ui.set_enabled(false);
                    ui.style_mut()
                        .visuals
                        .widgets
                        .noninteractive
                        .fg_stroke
                        .color = visual.strong_text_color();

                    // FIXME: After egui 0.27, now the widgets spawned inside this closure
                    // intercepts interactions, which is basically natural behavior(Upper layer
                    // widgets). However, this change breaks current implementation which relies on
                    // the previous table behavior.
                    ui.add_enabled_ui(false, |ui| {
                        viewer.show_cell_view(ui, &table.rows[row_id.0], col.0);
                    });

                    #[cfg(any())]
                    if selected {
                        ui.painter().rect_stroke(
                            ui_max_rect,
                            no_rounding,
                            Stroke {
                                width: 1.,
                                color: visual.weak_text_color(),
                            },
                        );
                    }

                    if interactive_row.is_some() && !is_editing {
                        let st = Stroke {
                            width: 1.,
                            color: visual.warn_fg_color.gamma_multiply(0.5),
                        };

                        let xr = ui_max_rect.x_range();
                        let yr = ui_max_rect.y_range();
                        ui.painter().hline(xr, yr.min, st);
                        ui.painter().hline(xr, yr.max, st);
                    }

                    if edit_state.is_some_and(|(_, vis)| vis == vis_col) {
                        editing_cell_rect = ui_max_rect;
                    }
                });

                new_maximum_height = rect.height().max(new_maximum_height);

                // -- Mouse Actions --
                if check_mouse_dragging_selection(&rect, &resp) {
                    // Expand cci selection
                    response_consumed = true;
                    s.cci_sel_update(linear_index);
                }

                if resp.clicked_by(PointerButton::Primary)
                    && (self.style.single_click_edit_mode || is_interactive_cell)
                {
                    response_consumed = true;
                    commands.push(Command::CcEditStart(
                        row_id,
                        vis_col,
                        viewer.clone_row(&table.rows[row_id.0]).into(),
                    ));
                    edit_started = true;
                }

                /* --------------------------- Context Menu Rendering --------------------------- */

                (resp.clone() | head_resp.clone()).context_menu(|ui| {
                    response_consumed = true;
                    ui.set_min_size(egui::vec2(250., 10.));

                    if !selected {
                        commands.push(Command::CcSetSelection(vec![VisSelection(
                            linear_index,
                            linear_index,
                        )]));
                    } else if !is_interactive_cell {
                        s.set_interactive_cell(vis_row, vis_col);
                    }

                    let sel_multi_row = s.cursor_as_selection().is_some_and(|sel| {
                        let mut min = usize::MAX;
                        let mut max = usize::MIN;

                        for sel in sel {
                            min = min.min(sel.0 .0);
                            max = max.max(sel.1 .0);
                        }

                        let (r_min, _) = VisLinearIdx(min).row_col(s.vis_cols().len());
                        let (r_max, _) = VisLinearIdx(max).row_col(s.vis_cols().len());

                        r_min != r_max
                    });

                    let cursor_x = ui.cursor().min.x;
                    let clip = s.has_clipboard_contents();
                    let b_undo = s.has_undo();
                    let b_redo = s.has_redo();
                    let mut n_sep_menu = 0;
                    let mut draw_sep = false;

                    [
                        Some((selected, "🖻", "Selection: Copy", UiAction::CopySelection)),
                        Some((selected, "🖻", "Selection: Cut", UiAction::CutSelection)),
                        Some((selected, "🗙", "Selection: Clear", UiAction::DeleteSelection)),
                        Some((
                            sel_multi_row,
                            "🗐",
                            "Selection: Fill",
                            UiAction::SelectionDuplicateValues,
                        )),
                        None,
                        Some((clip, "➿", "Clipboard: Paste", UiAction::PasteInPlace)),
                        Some((clip, "🛠", "Clipboard: Insert", UiAction::PasteInsert)),
                        None,
                        Some((true, "🗐", "Row: Duplicate", UiAction::DuplicateRow)),
                        Some((true, "🗙", "Row: Delete", UiAction::DeleteRow)),
                        None,
                        Some((b_undo, "⎗", "Undo", UiAction::Undo)),
                        Some((b_redo, "⎘", "Redo", UiAction::Redo)),
                    ]
                    .map(|opt| {
                        if let Some((icon, label, action)) =
                            opt.filter(|x| x.0).map(|x| (x.1, x.2, x.3))
                        {
                            if draw_sep {
                                draw_sep = false;
                                ui.separator();
                            }

                            let hotkey = hotkeys
                                .iter()
                                .find_map(|(k, a)| (a == &action).then(|| ctx.format_shortcut(k)));

                            ui.horizontal(|ui| {
                                ui.monospace(icon);
                                ui.add_space(cursor_x + 20. - ui.cursor().min.x);

                                let btn = egui::Button::new(label)
                                    .shortcut_text(hotkey.unwrap_or_else(|| "🗙".into()));
                                let r = ui.centered_and_justified(|ui| ui.add(btn)).inner;

                                if r.clicked() {
                                    actions.push(action);
                                    ui.close_menu();
                                }
                            });

                            n_sep_menu += 1;
                        } else if n_sep_menu > 0 {
                            n_sep_menu = 0;
                            draw_sep = true;
                        }
                    });
                });

                // Forward DnD event if not any event was consumed by the response.

                // FIXME: Upgrading egui 0.29 make interaction rectangle of response object
                // larger(in y axis) than actually visible column cell size. To deal with this,
                // I've used returned content area rectangle instead, expanding its width to
                // response size.

                let drop_area_rect = rect.with_max_x(resp.rect.max.x);
                let contains_pointer = ctx
                    .pointer_hover_pos()
                    .is_some_and(|pos| drop_area_rect.contains(pos));

                if !response_consumed && contains_pointer {
                    if let Some(new_value) =
                        viewer.on_cell_view_response(&table.rows[row_id.0], col.0, &resp)
                    {
                        commands.push(Command::SetCells {
                            slab: vec![*new_value].into_boxed_slice(),
                            values: vec![(row_id, *col, RowSlabIndex(0))].into_boxed_slice(),
                        });
                    }
                }
            }

            /* -------------------------------- Editor Rendering -------------------------------- */
            if let Some((should_focus, vis_column)) = edit_state {
                let column = s.vis_cols()[vis_column.0];

                egui::Window::new("")
                    .id(ui_id.with(row_id).with(column))
                    .constrain_to(body_max_rect)
                    .fixed_pos(editing_cell_rect.min)
                    .auto_sized()
                    .min_size(editing_cell_rect.size())
                    .max_width(editing_cell_rect.width())
                    .title_bar(false)
                    .frame(egui::Frame::none().rounding(egui::Rounding::same(3.)))
                    .show(ctx, |ui| {
                        ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                            if let Some(resp) =
                                viewer.show_cell_editor(ui, s.unwrap_editing_row_data(), column.0)
                            {
                                if should_focus {
                                    resp.request_focus()
                                }

                                new_maximum_height = resp.rect.height().max(new_maximum_height);
                            } else {
                                commands.push(Command::CcCommitEdit);
                            }
                        });
                    });
            }

            // Accumulate response
            if let Some(resp) = &mut resp_total {
                *resp = resp.union(row.response());
            } else {
                resp_total = Some(row.response());
            }

            // Update row height cache if necessary.
            if self.style.table_row_height.is_none() && prev_row_height != new_maximum_height {
                row_height_updates.push((vis_row, new_maximum_height));
            }
        }; // ~ render_fn

        // Actual rendering
        if let Some(height) = self.style.table_row_height {
            body.rows(height, cc_row_heights.len(), render_fn);
        } else {
            body.heterogeneous_rows(cc_row_heights.iter().cloned(), render_fn);
        }

        /* ----------------------------------- Event Handling ----------------------------------- */

        if ctx.input(|i| i.pointer.button_released(PointerButton::Primary)) {
            let mods = ctx.input(|i| i.modifiers);
            if let Some(sel) = s.cci_take_selection(mods).filter(|_| !edit_started) {
                commands.push(Command::CcSetSelection(sel));
            }
        }

        // Control overall focus status.
        if let Some(resp) = resp_total.clone() {
            if resp.clicked() | resp.dragged() {
                s.cci_has_focus = true;
            } else if resp.clicked_elsewhere() {
                s.cci_has_focus = false;
            }
        }

        // Check in borrowed `cc_rows` back to state.
        s.cc_row_heights = cc_row_heights.tap_mut(|values| {
            if !row_height_updates.is_empty() {
                ctx.request_repaint();
            }

            for (row_index, row_height) in row_height_updates {
                values[row_index.0] = row_height;
            }
        });

        // Handle queued actions
        commands.extend(
            actions
                .into_iter()
                .flat_map(|action| s.try_apply_ui_action(table, viewer, action)),
        );

        // Handle queued commands
        for cmd in commands {
            match cmd {
                Command::CcUpdateSystemClipboard(new_content) => {
                    ctx.output_mut(|x| {
                        x.copied_text = new_content;
                    });
                }
                cmd => {
                    if matches!(cmd, Command::CcCommitEdit) {
                        // If any commit action is detected, release any remaining focus.
                        ctx.memory_mut(|x| {
                            if let Some(fc) = x.focused() {
                                x.surrender_focus(fc)
                            }
                        });
                    }

                    s.push_new_command(table, viewer, cmd, self.style.max_undo_history);
                }
            }
        }

        // Total response
        resp_total.unwrap()
    }
}

impl<R, V: RowViewer<R>> Drop for Renderer<'_, R, V> {
    fn drop(&mut self) {
        self.table.ui = self.state.take();
    }
}
