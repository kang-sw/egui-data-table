use std::{any::Any, num::NonZeroUsize, sync::Arc};

use egui::mutex::Mutex;
use egui_extras::Column;
use indexmap::IndexSet;

use crate::{RowSlotId, Spreadsheet};

pub enum UiAction {
    ActivateSelectedCell,
    CancelEdit,
    CommitEdit,
    Undo,
    Redo,
}

pub trait RowViewer<R: Send> {
    /// Number of columns
    const COLUMNS: usize;

    fn column_name(&mut self, column: usize) -> &str;

    fn is_sortable_column(&mut self, column: usize) -> bool;

    fn compare_column(&mut self, row_l: &R, row_r: &R, column: usize) -> std::cmp::Ordering;

    /// Should return true if the column is modified. Otherwise, it won't be updated.
    ///
    /// When it's activated, the `active` flag is set to true. You can utilize this to
    /// expand editor, show popup, etc.
    fn draw_column_edit(&mut self, ui: &mut egui::Ui, row: &mut R, column: usize, active: bool);

    fn empty_row(&mut self) -> R;

    fn clone_column(&mut self, src: &R, dst: &mut R, column: usize);

    fn clone_column_arbitrary(
        &mut self,
        src: &R,
        src_column: usize,
        dst: &mut R,
        dst_column: usize,
    ) -> bool {
        debug_assert!(src_column != dst_column);
        let _ = (src, dst, src_column, dst_column);

        // Simply does not support arbitrary column cloning.
        false
    }

    fn clone_column_smart(&mut self, src: &R, dst: &mut R, column: usize, offset: usize) {
        let _ = offset;
        self.clone_column(src, dst, column);
    }

    fn clone_row(&mut self, src: &R) -> R;

    fn clear_column(&mut self, row: &mut R, column: usize);

    fn detect_hotkey(&mut self, ui: &egui::InputState) -> Option<UiAction> {
        // TODO: F2, Ctrl+C, Ctrl+V, Ctrl+D, Ctrl+E
        None
    }
}

/* ------------------------------------------ Indexing ------------------------------------------ */

type VisibleRowIndex = usize;
type VisibleColumnIndex = usize;

#[derive(Default)]
struct UiState {
    sheet_id: u64,

    visible_cols: Vec<usize>,
    visible_rows: Vec<(RowSlotId, f32)>,

    /// Sorting column
    sort_by: Option<NonZeroUsize>,

    /// Row selections.
    selections: IndexSet<usize>,

    /// When activated, previous row value is cached here
    ///
    /// TODO: undo / redo
    active_cell_source_value: Option<Box<dyn Any + Send>>,

    /// Spreadsheet is modified during
    is_invalid: bool,
}

enum HistoryArg {}

impl UiState {
    fn clear(&mut self, id: u64, n_column: usize) {
        // Clear the cache
        self.sheet_id = id;
        self.is_invalid = true;
    }

    fn column(&self, column: usize) -> Option<usize> {
        todo!()
    }

    fn validate<R: Send, V: RowViewer<R>>(&mut self, sheet: &mut Spreadsheet<R>, vwr: &mut V) {
        if !self.is_invalid {
            return;
        }
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
        V: RowViewer<R>,
    {
        let ui_id = ui_id.into();
        let ui_state_ptr = ui
            .memory(|x| x.data.get_temp::<Arc<Mutex<UiState>>>(ui_id))
            .unwrap_or_default();
        let mut ui_state = ui_state_ptr.lock();

        if ui_state.sheet_id != self.unique_id || ui_state.visible_cols.len() != V::COLUMNS {
            ui_state.clear(self.unique_id, V::COLUMNS);
        }

        ui.push_id(ui_id, |ui| {
            self.show_impl(ui, &mut ui_state, viewer);
        });

        // Reset memory.
        drop(ui_state);
        ui.memory_mut(|x| x.data.insert_temp(ui_id, ui_state_ptr));
    }

    fn show_impl<V>(&mut self, ui: &mut egui::Ui, s: &mut UiState, viewer: &mut V)
    where
        V: RowViewer<R>,
    {
        egui_extras::TableBuilder::new(ui)
            .columns(Column::auto().resizable(true), s.visible_cols.len())
            .drag_to_scroll(false) // Drag is used for selection
            .header(20., |mut h| {
                for &col in &s.visible_cols {
                    h.col(|ui| {
                        ui.label(viewer.column_name(col));
                    });
                }
            })
            .body(|body| {
                // Validate ui state
                s.validate(self, viewer);

                let mut row_len_updates = Vec::new();

                body.heterogeneous_rows(s.visible_rows.iter().map(|(_, a)| *a), |mut row| {
                    let row_index = row.index();
                    let (row_slot_id, row_height_prev) = s.visible_rows[row_index];

                    for &col in &s.visible_cols {
                        let linear_index = row_index * s.visible_cols.len() + col;
                        let is_col_selected = is_row_selected && s.selected_cols.contains(&col);

                        let (rect, resp) = row.col(|ui| {
                            viewer.draw_column_edit(
                                ui,
                                &mut self.rows[row_slot_id].data,
                                col,
                                todo!(),
                            );
                        });

                        if row_height_prev != rect.height() {
                            row_len_updates.push((row_index, rect.height()));
                        }
                    }

                    todo!();
                });

                // Update height caches

                for (row_index, row_height) in row_len_updates {
                    s.visible_rows[row_index].1 = row_height;
                }
            });
    }
}
