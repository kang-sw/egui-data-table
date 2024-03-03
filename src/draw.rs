use std::{any::Any, collections::VecDeque, hash::Hasher, mem::replace, sync::Arc};

use egui::{ahash::AHasher, mutex::Mutex, Layout, RichText};
use egui_extras::Column;
use indexmap::IndexSet;
use tap::prelude::{Pipe, Tap};

use crate::{
    viewer::{CellUiState, RowViewer},
    Spreadsheet,
};

use format as f;

/* ------------------------------------------ Indexing ------------------------------------------ */

type ColumnIndex = usize;
type RowIndex = usize;
type LinearCellIndex = usize;
type AnyRowValue = Box<dyn Any + Send>;

#[derive(Clone, Copy)]
struct IsAscending(bool);

#[derive(Default)]
struct UiState {
    /// Cached sheet id. If sheet id mismatches, the UiState is invalidated.
    sheet_id: u64,

    /// Cached number of columns.
    num_columns: usize,

    /// Unique hash of the viewer. This is to prevent cache invalidation when the viewer
    /// state is changed.
    viewer_hash: u64,

    /// Visible columns selected by user.
    visible_cols: Vec<ColumnIndex>,

    /// Sort option - column index and direction.
    sort_by: Option<(ColumnIndex, IsAscending)>,

    /// Row selections.
    selections: IndexSet<LinearCellIndex>,

    /// When activated, previous row value is cached here
    ///
    /// TODO: undo / redo
    active_cell_source_value: Option<(bool, RowIndex, Box<dyn Any + Send>)>,

    /// Any modification is stored here. We assume this is transient(putting this as field
    /// of UiState), as it's brittle to external modification to the spreadsheet.
    undo_stack: VecDeque<Command>,

    /// Any redo will push back to undo stack. This will be cleared when any modification
    /// is made.
    redo_stack: Vec<Command>,

    /// TODO: Clipboard
    ///
    /// -

    /*

        SECTION: Cache - Rendering

    */
    /// Cached rows.
    cc_rows: Vec<(RowIndex, f32)>,

    /// Spreadsheet is modified during the last validation.
    cc_dirty: bool,
}

#[derive(Clone)]
enum Command {
    HideColumn(usize),
    ShowColumn(usize),
    SortColumn(usize, IsAscending),
    CancelSort,
}

impl UiState {
    fn validate_identity<R: Send, V: RowViewer<R>>(&mut self, id: u64, vwr: &mut V) {
        let num_columns = vwr.num_columns();
        let vwr_hash = AHasher::default().pipe(|mut x| {
            std::hash::Hash::hash(vwr, &mut x);
            x.finish()
        });

        if self.sheet_id == id && self.num_columns == num_columns && self.viewer_hash == vwr_hash {
            return;
        }

        // Clear the cache
        *self = Default::default();
        self.sheet_id = id;
        self.viewer_hash = vwr_hash;
        self.num_columns = num_columns;

        self.visible_cols.extend(0..num_columns);
        self.cc_dirty = true;
    }

    fn validate_cc<R: Send, V: RowViewer<R>>(&mut self, sheet: &mut Spreadsheet<R>, vwr: &mut V) {
        if !replace(&mut self.cc_dirty, false) {
            return;
        }

        // TODO: Boost performance with `rayon`

        // We should validate the entire cache.
        let mut it_all_rows = sheet
            .iter()
            .enumerate()
            .filter_map(|(i, x)| vwr.filter_row(x).then_some(i));

        for (idx, (cc_row, _)) in self.cc_rows.iter_mut().enumerate() {
            let Some(row) = it_all_rows.next() else {
                // Clear the rest of the cache.
                self.cc_rows.drain(idx..);
                break;
            };

            *cc_row = row;
        }

        // If there are more rows left, we should add them.
        for row in it_all_rows {
            self.cc_rows.push((row, 10.)); // Just neat default value.
        }

        // TODO: Sort by column
    }
}

/* ------------------------------------------ Rendering ----------------------------------------- */

impl<R: Send> Spreadsheet<R> {
    /// Show the spreadsheet.
    ///
    /// You should be careful on assigning `ui_id` to this spreadsheet. If it is
    /// duplicated with any other spreadsheet that is actively rendered, it'll constantly
    /// invalidate the index table cache which results in a performance hit.
    pub fn show<V>(&mut self, ui: &mut egui::Ui, ui_id: impl Into<egui::Id>, viewer: &mut V)
    where
        R: Send,
        V: RowViewer<R>,
    {
        let ui_id = ui_id.into();
        let ui_state_ptr = ui
            .memory(|x| x.data.get_temp::<Arc<Mutex<UiState>>>(ui_id))
            .unwrap_or_default();

        let mut ui_state = ui_state_ptr.lock();
        ui_state.validate_identity(self.unique_id, viewer);

        ui.push_id(ui_id, |ui| {
            self.show_impl(ui, &mut ui_state, viewer);
        });

        // Reset memory.
        drop(ui_state);
        ui.memory_mut(|x| x.data.insert_temp(ui_id, ui_state_ptr));
    }

    fn show_impl<V>(&mut self, ui: &mut egui::Ui, s: &mut UiState, viewer: &mut V)
    where
        R: Send,
        V: RowViewer<R>,
    {
        let ctx = &ui.ctx().clone();
        let mut added_commands = Vec::<Command>::new();
        let mut undo = false;
        let mut redo = false;

        let mut builder = egui_extras::TableBuilder::new(ui)
            .column(Column::auto_with_initial_suggestion(10.).resizable(false));

        for &column in &s.visible_cols {
            builder = builder.column(viewer.column_config(column));
        }

        builder
            .column(Column::remainder())
            .drag_to_scroll(false) // Drag is used for selection
            .striped(true)
            .max_scroll_height(f32::MAX)
            .header(20., |mut h| {
                h.col(|ui| {
                    // TODO: Button to pop up context menu.
                    ui.centered_and_justified(|ui| {
                        ui.menu_button("⛭", |ui| {
                            // TODO:
                        });
                    });
                });

                for &col in &s.visible_cols {
                    let (rect, resp) = h.col(|ui| {
                        ui.horizontal_centered(|ui| {
                            // TODO: Sort indicator
                            let is_hover =
                                ui.rect_contains_pointer(ui.available_rect_before_wrap());

                            ui.label(viewer.column_name(col)).hovered();

                            if !is_hover {
                                return;
                            }

                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                let hide_resp = ui.selectable_label(
                                    false,
                                    RichText::new("✖").color(ui.visuals().error_fg_color),
                                );

                                if hide_resp.clicked() {
                                    dbg!()
                                }
                            });
                        });
                    });

                    resp.context_menu(|ui| {});
                }

                // Remainder
                h.col(|ui| {});
            })
            .tap_mut(|table| {
                table.ui_mut().separator();
            })
            .body(|body| {
                // Validate ui state. Defer this as late as possible; since it may not be
                // called if the table area is out of the visible space.
                s.validate_cc(self, viewer);

                let mut row_len_updates = Vec::new();
                let vis_row_digits = s.cc_rows.len().max(1).ilog10();
                let row_id_digits = self.len().max(1).ilog10();

                body.heterogeneous_rows(s.cc_rows.iter().map(|(_, a)| *a), |mut row| {
                    let table_row = row.index();
                    let (row_id, row_height_prev) = s.cc_rows[table_row];

                    // Render row header button
                    let (mut rect, mut resp) = row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.monospace(
                                RichText::from(f!(
                                    "{row_id:·>width$}",
                                    width = row_id_digits as usize
                                ))
                                .strong(),
                            );

                            ui.monospace(
                                RichText::from(f!(
                                    "{:·>width$}",
                                    table_row + 1,
                                    width = vis_row_digits as usize
                                ))
                                .weak(),
                            );
                        });
                    });

                    // Refresh height of the row.
                    let mut max_cell_height = rect.height();

                    for &col in &s.visible_cols {
                        let linear_index = table_row * s.visible_cols.len() + col;
                        let selected = s.selections.contains(&linear_index);

                        row.set_selected(selected);

                        (rect, resp) = row.col(|ui| {
                            let edit_state = if let Some((is_fresh, ..)) = s
                                .active_cell_source_value
                                .as_mut()
                                .filter(|(_, id, _)| *id == row_id)
                            {
                                if *is_fresh {
                                    *is_fresh = false;
                                    CellUiState::EditStarted
                                } else {
                                    CellUiState::Editing
                                }
                            } else {
                                CellUiState::View
                            };

                            ui.add_enabled_ui(edit_state.is_editing(), |ui| {
                                viewer.draw_cell(ui, &mut self.rows[row_id], col, edit_state);
                            });
                        });

                        max_cell_height = rect.height().max(max_cell_height);

                        // TODO: Create actions from response.
                        // - Highlight on click
                        // - Ctrl + Click, Shift + Click, Ctrl + A, Drag.
                        // -
                    }

                    if row_height_prev != max_cell_height {
                        row_len_updates.push((table_row, max_cell_height));
                    }

                    // Remainder
                    row.col(|_| {});
                });

                // Update height caches

                for (row_index, row_height) in row_len_updates {
                    s.cc_rows[row_index].1 = row_height;
                }
            });
    }
}
