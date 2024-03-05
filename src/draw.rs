use std::{iter::repeat_with, mem::take};

use egui::{Align, Color32, Event, Layout, PointerButton, Rect, Response, RichText, Sense, Stroke};
use egui_extras::Column;
use tap::prelude::{Pipe, Tap};

use crate::{
    viewer::{EditorAction, MoveDirection, RowViewer, TrivialConfig},
    DataTable, UiAction,
};

use self::state::*;

use format as f;

pub(crate) mod state;

/* ------------------------------------------ Rendering ----------------------------------------- */

pub struct Renderer<'a, R, V: RowViewer<R>> {
    table: &'a mut DataTable<R>,
    viewer: &'a mut V,
    state: Option<Box<UiState<R>>>,

    config: TrivialConfig,
}

impl<'a, R, V: RowViewer<R>> egui::Widget for Renderer<'a, R, V> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        self.show(ui)
    }
}

impl<'a, R, V: RowViewer<R>> Renderer<'a, R, V> {
    pub fn new(table: &'a mut DataTable<R>, viewer: &'a mut V) -> Self {
        Self {
            state: Some(table.ui.take().unwrap_or_default().tap_mut(|x| {
                if table.rows.is_empty() {
                    table.rows.push(viewer.row_empty());
                    x.force_mark_dirty();
                }

                x.validate_identity(viewer);
            })),
            table,
            config: viewer.trivial_config(),
            viewer,
        }
    }

    pub fn with_table_row_height(mut self, height: f32) -> Self {
        self.config.table_row_height = Some(height);
        self
    }

    pub fn with_max_undo_history(mut self, max_undo_history: usize) -> Self {
        self.config.max_undo_history = max_undo_history;
        self
    }

    pub fn show(mut self, ui: &mut egui::Ui) -> Response {
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

        let green = if visual.window_fill.g() > 128 {
            Color32::DARK_GREEN
        } else {
            Color32::GREEN
        };

        let mut builder = egui_extras::TableBuilder::new(ui).column(Column::auto());

        for &column in s.vis_cols.iter() {
            builder = builder.column(viewer.column_render_config(column.0));
        }

        if s.is_editing() {
            let interact_row = s.interactive_cell().0;
            builder = builder.scroll_to_row(interact_row.0, None);
        }

        builder
            .columns(Column::auto(), s.num_columns() - s.vis_cols.len())
            .drag_to_scroll(false) // Drag is used for selection;
            .striped(true)
            .max_scroll_height(f32::MAX)
            .sense(Sense::click_and_drag().tap_mut(|x| x.focusable = true))
            .header(20., |mut h| {
                h.set_selected(s.cci_has_focus);
                h.col(|ui| {
                    ui.centered_and_justified(|ui| {
                        ui.monospace("POS / ID");
                    });
                });
                h.set_selected(false);

                let has_any_hidden_col = s.vis_cols.len() != s.num_columns();

                for (vis_col, &col) in s.vis_cols.iter().enumerate() {
                    let vis_col = VisColumnPos(vis_col);
                    let mut painter = None;
                    let (_, resp) = h.col(|ui| {
                        ui.horizontal_centered(|ui| {
                            if let Some(pos) = s.sort().iter().position(|(c, ..)| c == &col) {
                                let asc = &s.sort()[pos].1;

                                ui.colored_label(
                                    if !asc.0 { Color32::RED } else { green },
                                    RichText::new(format!(
                                        "{}{}",
                                        if asc.0 { "↗" } else { "↘" },
                                        pos + 1,
                                    ))
                                    .monospace(),
                                );
                            } else {
                                ui.monospace(" ");
                            }

                            ui.label(viewer.column_name(col.0));
                        });

                        painter = Some(ui.painter().clone());
                    });

                    // Set drag payload for column reordering.
                    resp.dnd_set_drag_payload(vis_col);

                    if resp.dragged() {
                        egui::popup::show_tooltip_text(
                            ctx,
                            "_EGUI_DATATABLE__COLUMN_MOVE__".into(),
                            viewer.column_name(col.0),
                        );
                    }

                    if resp.hovered() && viewer.is_sortable_column(col.0) {
                        if let Some(p) = &painter {
                            p.rect_filled(
                                resp.rect,
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
                                resp.rect,
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
                                if !s.vis_cols.contains(&col)
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
                resp_ret =
                    Some(self.show_body(body, painter, commands, ctx, &style, ui_id, resp_total));
            });

        resp_ret.unwrap_or_else(|| ui.label("??"))
    }

    #[allow(clippy::too_many_arguments)]
    fn show_body(
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
        let visible_cols = s.vis_cols.clone();
        let no_rounding = egui::Rounding::ZERO;

        let mut actions = Vec::<UiAction>::new();
        let mut edit_started = false;

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

                            // TODO: Later try to parse clipboard contents and detect if
                            // it's compatible with cells being pasted.
                            Event::Paste(_) => {
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

            let context = &s.ui_action_context();
            actions.extend(
                repeat_with(|| ctx.input_mut(|i| viewer.detect_hotkey(i, context)))
                    .map_while(|x| x),
            )
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
        let body_widths = body.widths()[1..body.widths().len().max(1)].to_owned();

        let pointer_interact_pos = ctx.input(|i| i.pointer.latest_pos().unwrap_or_default());
        let pointer_primary_down = ctx.input(|i| i.pointer.button_down(PointerButton::Primary));

        /* ----------------------------- Primary Rendering Function ----------------------------- */
        // - Extracted as a closure to differentiate behavior based on row height
        //   configuration. (heterogeneous or homogeneous row heights)

        let render_fn = |mut row: egui_extras::TableRow| {
            let vis_row = VisRowPos(row.index());
            let row_id = s.cc_rows[vis_row.0];
            let prev_row_height = cc_row_heights[vis_row.0];

            let mut row_elem_start = Default::default();

            // Check if current row is edition target
            let edit_state = s.row_is_fresh_edit(row_id);
            let interactive_row = s.is_interactive_row(vis_row);

            let check_mouse_dragging_selection = {
                let s_cci_has_focus = s.cci_has_focus;
                let s_cci_has_selection = s.has_cci_selection();

                move |resp: &egui::Response| {
                    let cci_hovered: bool = s_cci_has_focus
                        && s_cci_has_selection
                        && resp.rect.contains(pointer_interact_pos);
                    let sel_drag = cci_hovered && pointer_primary_down;
                    let sel_click = !s_cci_has_selection && resp.hovered() && pointer_primary_down;

                    edit_state.is_none() && (sel_drag || sel_click)
                }
            };

            /* -------------------------------- Header Rendering -------------------------------- */

            // Mark row background filled if being edited.
            row.set_selected(edit_state.is_some());

            // Render row header button
            let (_, head_resp) = row.col(|ui| {
                // Calculate the position where values start.
                row_elem_start = ui.max_rect().right_top();

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.separator();

                    ui.monospace(
                        RichText::from(f!("{:·>width$}", row_id.0, width = row_id_digits as usize))
                            .strong(),
                    );

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

            if check_mouse_dragging_selection(&head_resp) {
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

                // Mark background filled if selected.
                row.set_selected(cci_selected);

                let (rect, resp) = row.col(|ui| {
                    let ui_max_rect = ui.max_rect();
                    ui.set_enabled(false);

                    if !cci_selected && selected {
                        ui.painter()
                            .rect_filled(ui_max_rect, no_rounding, visual.extreme_bg_color);
                    }

                    viewer.cell_view(ui, &table.rows[row_id.0], col.0);

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

                        if is_interactive_cell {
                            ui.painter().rect_stroke(
                                ui_max_rect,
                                no_rounding,
                                Stroke {
                                    width: 2.,
                                    color: visual.warn_fg_color,
                                },
                            );
                        }
                    }
                });

                new_maximum_height = rect.height().max(new_maximum_height);

                // -- Mouse Actions --
                if check_mouse_dragging_selection(&resp) {
                    // Expand cci selection
                    response_consumed = true;
                    s.cci_sel_update(linear_index);
                }

                if resp.clicked_by(PointerButton::Primary) && is_interactive_cell {
                    response_consumed = true;
                    commands.push(Command::CcEditStart(
                        row_id,
                        vis_col,
                        viewer.row_clone(&table.rows[row_id.0]).into(),
                    ));
                    edit_started = true;
                }

                (resp.clone() | head_resp.clone()).context_menu(|ui| {
                    response_consumed = true;

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

                        let (r_min, _) = VisLinearIdx(min).row_col(s.vis_cols.len());
                        let (r_max, _) = VisLinearIdx(max).row_col(s.vis_cols.len());

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
                        Some((
                            sel_multi_row,
                            "🗐",
                            "Selection: Duplicate",
                            UiAction::SelectionDuplicateValues,
                        )),
                        None,
                        Some((clip, "➿", "Paste: Replace", UiAction::PasteInPlace)),
                        Some((clip, "🛠", "Paste: Insert", UiAction::PasteInsert)),
                        None,
                        Some((true, "🗐", "Duplicate Row", UiAction::DuplicateRow)),
                        Some((true, "🗙", "Delete Row", UiAction::DeleteRow)),
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

                            ui.horizontal(|ui| {
                                ui.monospace(icon);
                                ui.add_space(cursor_x + 20. - ui.cursor().min.x);

                                let r = ui.centered_and_justified(|ui| ui.button(label)).inner;

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

                if !response_consumed && resp.contains_pointer() {
                    if let Some(new_value) =
                        viewer.cell_view_dnd_response(&table.rows[row_id.0], col.0, &resp)
                    {
                        commands.push(Command::SetCell(new_value, row_id, *col));
                    }
                }
            }

            /* -------------------------------- Editor Rendering -------------------------------- */
            // - TODO: Change to cell-based editor.

            if let Some(focus_column) = edit_state {
                // Column ui rectangles.
                let column_rects;

                // Editing window's rectangle.
                let edit_window_rect;

                {
                    // Calculate column rectangles ...
                    let mut rects = Vec::with_capacity(body_widths.len());
                    let mut width_acc = style.spacing.item_spacing.x * 0.5;

                    for &width in &body_widths {
                        let width = width + style.spacing.item_spacing.x;
                        let rect = Rect::from_min_size(
                            row_elem_start + egui::vec2(width_acc, 0.),
                            egui::vec2(width, prev_row_height),
                        );

                        width_acc += width;
                        rects.push(rect);
                    }

                    edit_window_rect = rects
                        .iter()
                        .fold(Rect::NOTHING, |acc, x| acc.union(*x))
                        .pipe(|x| x.translate(egui::vec2(0., style.spacing.item_spacing.y * 0.5)));

                    column_rects = rects;
                };

                let response = egui::Window::new("")
                    .id(ui_id.with("__Egui_DataTable_Window__").with(vis_row.0))
                    .title_bar(false)
                    .min_size(edit_window_rect.size())
                    .fixed_pos(edit_window_rect.min)
                    .auto_sized()
                    .constrain_to(body_max_rect)
                    .frame(egui::Frame::none().fill(visual.window_fill))
                    .show(ctx, |ui| {
                        let mut ui_columns = column_rects
                            .iter()
                            .cloned()
                            .take(visible_cols.len())
                            .enumerate()
                            .map(|(col, rect)| {
                                ui.child_ui_with_id_source(
                                    rect.shrink2(style.spacing.item_spacing * 0.5),
                                    Layout::top_down_justified(Align::LEFT),
                                    s.vis_cols[col].0,
                                )
                                .tap_mut(|x| {
                                    x.set_clip_rect(
                                        rect.with_max_y(f32::INFINITY).intersect(body_max_rect),
                                    )
                                })
                            })
                            .collect::<Vec<_>>();

                        for (vis_col, ui) in ui_columns.iter_mut().enumerate() {
                            let column = visible_cols[vis_col].0;
                            let action: EditorAction = viewer
                                .cell_editor(
                                    ui,
                                    s.unwrap_editing_row_data(),
                                    column,
                                    focus_column.map(|x| visible_cols[x.0].0),
                                )
                                .into();

                            match action {
                                EditorAction::Idle => (),
                                EditorAction::Commit => actions
                                    .push(UiAction::CommitEditionAndMove(MoveDirection::Down)),
                                EditorAction::Cancel => actions.push(UiAction::CancelEdition),
                            }
                        }

                        let ui_min_rect = ui_columns
                            .into_iter()
                            .fold(Rect::NOTHING, |acc, c_ui| acc.union(c_ui.min_rect()));

                        ui.advance_cursor_after_rect(
                            ui_min_rect.union(edit_window_rect).intersect(body_max_rect),
                        );

                        for mut rect in column_rects {
                            rect.min.y = ui_min_rect.min.y;
                            rect.max.y = ui_min_rect.max.y;

                            ui.painter().rect_stroke(
                                rect,
                                egui::Rounding::default(),
                                visual.window_stroke,
                            );
                        }

                        new_maximum_height = ui_min_rect.height().max(new_maximum_height);
                    });

                if let Some(response) = response {
                    let resp_tot = resp_total.as_mut().unwrap();
                    *resp_tot = resp_tot.union(response.response);
                }
            }

            // Accumulate response
            if let Some(resp) = &mut resp_total {
                *resp = resp.union(row.response());
            } else {
                resp_total = Some(row.response());
            }

            // Update row height cache if necessary.
            if self.config.table_row_height.is_none() && prev_row_height != new_maximum_height {
                row_height_updates.push((vis_row, new_maximum_height));
            }
        }; // ~ render_fn

        // Actual rendering
        if let Some(height) = self.config.table_row_height {
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
            if matches!(cmd, Command::CcCommitEdit) {
                // If any commit action is detected, release any remaining focus.
                ctx.memory_mut(|x| {
                    if let Some(fc) = x.focus() {
                        x.surrender_focus(fc)
                    }
                });
            }

            s.push_new_command(table, viewer, cmd, self.config.max_undo_history);
        }

        // Total response
        resp_total.unwrap()
    }
}

impl<'a, R, V: RowViewer<R>> Drop for Renderer<'a, R, V> {
    fn drop(&mut self) {
        self.table.ui = self.state.take();
    }
}
