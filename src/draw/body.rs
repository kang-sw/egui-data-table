use super::*;

impl<R, V: RowViewer<R>> Renderer<'_, R, V> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn impl_show_body(
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
        let no_rounding = egui::CornerRadius::ZERO;

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
                                    if viewer.allow_row_insertions() {
                                        actions.push(UiAction::PasteInsert)
                                    }
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
                            StrokeKind::Inside,
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
                        .color = if is_interactive_cell {
                        self.style
                            .fg_selected_highlight_cell
                            .unwrap_or(visual.strong_text_color())
                    } else {
                        visual.strong_text_color()
                    };

                    // FIXME: After egui 0.27, now the widgets spawned inside this closure
                    // intercepts interactions, which is basically natural behavior(Upper layer
                    // widgets). However, this change breaks current implementation which relies on
                    // the previous table behavior.
                    ui.add_enabled_ui(false, |ui| {
                        if !(is_editing && is_interactive_cell) {
                            viewer.show_cell_view(ui, &table.rows[row_id.0], col.0);
                        }
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
                            color: self
                                .style
                                .focused_row_stroke
                                .unwrap_or(visual.warn_fg_color.gamma_multiply(0.5)),
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

                let editable = viewer.is_editable_cell(vis_col.0, vis_row.0, &table.rows[row_id.0]);

                if editable
                    && (resp.clicked_by(PointerButton::Primary)
                        && (self.style.single_click_edit_mode || is_interactive_cell))
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
                            min = min.min(sel.0.0);
                            max = max.max(sel.1.0);
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

                    let context_menu_items = [
                        Some((
                            selected,
                            "🖻",
                            "context-menu-selection-copy",
                            UiAction::CopySelection,
                        )),
                        Some((
                            selected,
                            "🖻",
                            "context-menu-selection-cut",
                            UiAction::CutSelection,
                        )),
                        Some((
                            selected,
                            "🗙",
                            "context-menu-selection-clear",
                            UiAction::DeleteSelection,
                        )),
                        Some((
                            sel_multi_row,
                            "🗐",
                            "context-menu-selection-fill",
                            UiAction::SelectionDuplicateValues,
                        )),
                        None,
                        Some((
                            clip,
                            "➿",
                            "context-menu-clipboard-paste",
                            UiAction::PasteInPlace,
                        )),
                        Some((
                            clip && viewer.allow_row_insertions(),
                            "🛠",
                            "context-menu-clipboard-insert",
                            UiAction::PasteInsert,
                        )),
                        None,
                        Some((
                            viewer.allow_row_insertions(),
                            "🗐",
                            "context-menu-row-duplicate",
                            UiAction::DuplicateRow,
                        )),
                        Some((
                            viewer.allow_row_deletions(),
                            "🗙",
                            "context-menu-row-delete",
                            UiAction::DeleteRow,
                        )),
                        None,
                        Some((b_undo, "⎗", "context-menu-undo", UiAction::Undo)),
                        Some((b_redo, "⎘", "context-menu-redo", UiAction::Redo)),
                    ];

                    for opt in context_menu_items {
                        if let Some((icon, key, action)) =
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

                                let label = self.translator.translate(key);
                                let btn = egui::Button::new(label)
                                    .shortcut_text(hotkey.unwrap_or_else(|| "🗙".into()));
                                let r = ui.centered_and_justified(|ui| ui.add(btn)).inner;

                                if r.clicked() {
                                    actions.push(action);
                                }
                            });

                            n_sep_menu += 1;
                        } else if n_sep_menu > 0 {
                            n_sep_menu = 0;
                            draw_sep = true;
                        }
                    }
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
                        let mut values = vec![(row_id, *col, RowSlabIndex(0))];

                        values.retain(|(row, col, _slab_id)| {
                            viewer.is_editable_cell(col.0, row.0, &table.rows[row.0])
                        });

                        commands.push(Command::SetCells {
                            slab: vec![*new_value].into_boxed_slice(),
                            values: values.into_boxed_slice(),
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
                    .frame(egui::Frame::NONE.corner_radius(egui::CornerRadius::same(3)))
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
            let modifier = ctx.input(|i| {
                let m = i.modifiers;
                if m.is_none() {
                    SelectionModifier::None
                } else if m.command_only() {
                    SelectionModifier::Toggle
                } else if m.shift_only() {
                    SelectionModifier::Extend
                } else {
                    SelectionModifier::None
                }
            });
            if let Some(sel) = s.cci_take_selection(modifier).filter(|_| !edit_started) {
                commands.push(Command::CcSetSelection(sel));
            }
        }

        // Control overall focus status.
        if let Some(resp) = resp_total.clone() {
            let clicked_elsewhere = resp.clicked_elsewhere();
            // IMPORTANT: cannot use `resp.contains_pointer()` here
            let response_rect_contains_pointer = resp.rect.contains(pointer_interact_pos);

            if resp.clicked() | resp.dragged() {
                s.cci_has_focus = true;
            } else if clicked_elsewhere && !response_rect_contains_pointer {
                s.cci_has_focus = false;
                if s.is_editing() {
                    commands.push(Command::CcCommitEdit)
                }
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
                    ctx.copy_text(new_content);
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

                    s.push_new_command(
                        table,
                        viewer,
                        cmd,
                        if self.style.max_undo_history == 0 {
                            100
                        } else {
                            self.style.max_undo_history
                        },
                    );
                }
            }
        }

        // Total response
        resp_total.unwrap()
    }
}
