use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    hash::{Hash, Hasher},
    mem::{replace, take},
};

use egui::{
    ahash::{AHasher, HashMap, HashMapExt},
    Modifiers,
};
use tap::prelude::{Pipe, Tap};

use crate::{
    default,
    viewer::{MoveDirection, UiActionContext, UiCursorState},
    DataTable, RowViewer, UiAction,
};

macro_rules! int_ty {
(struct $name:ident ($($ty:ty),+); $($rest:tt)*) => {
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, PartialOrd, Ord)]
    pub(crate) struct $name($(pub(in crate::draw) $ty),+);

    int_ty!($($rest)*);
};
() => {}
}

int_ty!(
    struct VisLinearIdx(usize);
    struct VisSelection(VisLinearIdx, VisLinearIdx);
    struct RowSlabIndex(usize);
    struct ColumnIdx(usize);
    struct RowIdx(usize);
    struct VisRowPos(usize);
    struct VisRowOffset(usize);
    struct VisColumnPos(usize);
    struct IsAscending(bool);
);

impl VisSelection {
    pub fn contains(&self, ncol: usize, row: VisRowPos, col: VisColumnPos) -> bool {
        let (top, left) = self.0.row_col(ncol);
        let (bottom, right) = self.1.row_col(ncol);

        row.0 >= top.0 && row.0 <= bottom.0 && col.0 >= left.0 && col.0 <= right.0
    }

    pub fn contains_rect(&self, ncol: usize, other: Self) -> bool {
        let (top, left) = self.0.row_col(ncol);
        let (bottom, right) = self.1.row_col(ncol);

        let (other_top, other_left) = other.0.row_col(ncol);
        let (other_bottom, other_right) = other.1.row_col(ncol);

        other_top.0 >= top.0
            && other_bottom.0 <= bottom.0
            && other_left.0 >= left.0
            && other_right.0 <= right.0
    }

    pub fn from_points(ncol: usize, a: VisLinearIdx, b: VisLinearIdx) -> Self {
        let (a_r, a_c) = a.row_col(ncol);
        let (b_r, b_c) = b.row_col(ncol);

        let top = a_r.0.min(b_r.0);
        let bottom = a_r.0.max(b_r.0);
        let left = a_c.0.min(b_c.0);
        let right = a_c.0.max(b_c.0);

        Self(
            VisLinearIdx(top * ncol + left),
            VisLinearIdx(bottom * ncol + right),
        )
    }

    pub fn is_point(&self) -> bool {
        self.0 == self.1
    }

    pub fn union(&self, ncol: usize, other: Self) -> Self {
        let (top, left) = self.0.row_col(ncol);
        let (bottom, right) = self.1.row_col(ncol);

        let (other_top, other_left) = other.0.row_col(ncol);
        let (other_bottom, other_right) = other.1.row_col(ncol);

        let top = top.0.min(other_top.0);
        let left = left.0.min(other_left.0);
        let bottom = bottom.0.max(other_bottom.0);
        let right = right.0.max(other_right.0);

        Self(
            VisLinearIdx(top * ncol + left),
            VisLinearIdx(bottom * ncol + right),
        )
    }

    pub fn _from_row_col(ncol: usize, r: VisRowPos, c: VisColumnPos) -> Self {
        r.linear_index(ncol, c).pipe(|idx| Self(idx, idx))
    }
}

impl From<VisLinearIdx> for VisSelection {
    fn from(value: VisLinearIdx) -> Self {
        Self(value, value)
    }
}

impl VisLinearIdx {
    pub fn row_col(&self, ncol: usize) -> (VisRowPos, VisColumnPos) {
        let (row, col) = (self.0 / ncol, self.0 % ncol);
        (VisRowPos(row), VisColumnPos(col))
    }
}

impl VisRowPos {
    pub fn linear_index(&self, ncol: usize, col: VisColumnPos) -> VisLinearIdx {
        VisLinearIdx(self.0 * ncol + col.0)
    }
}

/// TODO: Serialization?
pub(crate) struct UiState<R> {
    /// Cached number of columns.
    num_columns: usize,

    /// Type id of the viewer.
    viewer_type: std::any::TypeId,

    /// Unique hash of the viewer. This is to prevent cache invalidation when the viewer
    /// state is changed.
    viewer_filter_hash: u64,

    /// Visible columns selected by user.
    pub vis_cols: Vec<ColumnIdx>,

    /// Column sorting state.
    sort: Vec<(ColumnIdx, IsAscending)>,

    /// Undo queue.
    ///
    /// - Push tasks front of the queue.
    /// - Drain all elements from `0..undo_cursor`
    /// - Pop overflow elements from the back.
    undo_queue: VecDeque<UndoArg<R>>,

    /// Undo cursor => increment by 1 on every undo, decrement by 1 on redo.
    undo_cursor: usize,

    /// Clipboard contents.
    clipboard: Option<Clipboard<R>>,

    /*

        SECTION: Cache - Rendering

    */
    /// Cached rows. Vector index is `VisRowPos`. Tuple is (row_id,
    /// cached_row_display_height)
    pub cc_rows: Vec<RowIdx>,

    /// Cached row heights. Vector index is `VisRowPos`.
    ///
    /// WARNING: DO NOT ACCESS THIS DURING RENDERING; as it's taken out for heterogenous
    /// row height support, therefore invalid during table rendering.
    pub cc_row_heights: Vec<f32>,

    /// Cached row id to visual row position table for quick lookup.
    cc_row_id_to_vis: HashMap<RowIdx, VisRowPos>,

    /// Spreadsheet is modified during the last validation.
    cc_dirty: bool,

    /// Row selections. First element's top-left corner is always 'highlight' row if
    /// editing row isn't present.
    cc_cursor: CursorState<R>,

    /// Number of frames from the last edit. Used to validate sorting.
    cc_num_frame_from_last_edit: usize,

    /// Cached previous number of columns.
    cc_prev_n_columns: usize,

    /// Latest interactive cell; Used for keyboard navigation.
    cc_interactive_cell: VisLinearIdx,

    /// Desired selection of next validation
    cc_desired_selection: Option<Vec<RowIdx>>,

    /*

        SECTION: Cache - Input Status

    */
    /// (Pivot, Current) selection.
    cci_selection: Option<(VisLinearIdx, VisLinearIdx)>,

    /// We have latest click.
    pub cci_has_focus: bool,

    /// Interface wants to scroll to the row.
    pub cci_want_move_scroll: bool,

    /// How many rows are rendered at once recently?
    pub cci_page_row_count: usize,
}

struct Clipboard<R> {
    slab: Box<[R]>,

    /// The first tuple element `VisRowPos` is offset from the top-left corner of the
    /// selection.
    pastes: Box<[(VisRowOffset, ColumnIdx, RowSlabIndex)]>,
}

struct UndoArg<R> {
    apply: Command<R>,
    restore: Vec<Command<R>>,
}

impl<R> Default for UiState<R> {
    fn default() -> Self {
        Self {
            num_columns: 0,
            viewer_filter_hash: 0,
            clipboard: None,
            viewer_type: std::any::TypeId::of::<()>(),
            vis_cols: Vec::default(),
            sort: Vec::default(),
            cc_cursor: CursorState::Select(default()),
            undo_queue: VecDeque::default(),
            cc_rows: Vec::default(),
            cc_row_heights: Vec::default(),
            cc_dirty: false,
            undo_cursor: 0,
            cci_selection: None,
            cci_has_focus: false,
            cc_interactive_cell: VisLinearIdx(0),
            cc_row_id_to_vis: default(),
            cc_num_frame_from_last_edit: 0,
            cc_prev_n_columns: 0,
            cc_desired_selection: None,
            cci_want_move_scroll: false,
            cci_page_row_count: 0,
        }
    }
}

enum CursorState<R> {
    Select(Vec<VisSelection>),
    Edit {
        next_focus: bool,
        last_focus: VisColumnPos,
        row: RowIdx,
        edition: R,
    },
}

impl<R> UiState<R> {
    pub fn validate_identity<V: RowViewer<R>>(&mut self, vwr: &mut V) {
        let num_columns = vwr.num_columns();
        let vwr_type_id = std::any::TypeId::of::<V>();
        let vwr_hash = AHasher::default().pipe(|mut hsh| {
            vwr.row_filter_hash().hash(&mut hsh);
            hsh.finish()
        });

        // Check for nontrivial changes.
        if self.num_columns == num_columns && self.viewer_type == vwr_type_id {
            // Check for trivial changes which does not require total reconstruction of
            // UiState.

            // If viewer's filter is changed. It always invalidates current cache.
            if self.viewer_filter_hash != vwr_hash {
                self.viewer_filter_hash = vwr_hash;
                self.cc_dirty = true;
            }

            // Defer validation of cache if it's still editing.
            {
                if !self.is_editing() {
                    self.cc_num_frame_from_last_edit += 1;
                }

                if self.cc_num_frame_from_last_edit == 2 {
                    self.cc_dirty |= !self.sort.is_empty();
                }
            }

            // Check if any sort config is invalidated.
            self.cc_dirty |= {
                let mut any_sort_invalidated = false;

                self.sort.retain(|(c, _)| {
                    vwr.is_sortable_column(c.0)
                        .tap(|x| any_sort_invalidated |= !x)
                });

                any_sort_invalidated
            };

            return;
        }

        // Clear the cache
        *self = Default::default();
        self.viewer_type = vwr_type_id;
        self.viewer_filter_hash = vwr_hash;
        self.num_columns = num_columns;

        self.vis_cols.extend((0..num_columns).map(ColumnIdx));
        self.cc_dirty = true;
    }

    pub fn validate_cc<V: RowViewer<R>>(&mut self, rows: &mut [R], vwr: &mut V) {
        if !replace(&mut self.cc_dirty, false) {
            self.handle_desired_selection();
            return;
        }

        // XXX: Boost performance with `rayon`?
        // - Returning `comparator` which is marked as `Sync`
        // - For this, `R` also need to be sent to multiple threads safely.
        // - Maybe we need specialization for `R: Send`?

        // We should validate the entire cache.
        let filter = vwr.create_row_filter();
        self.cc_rows.clear();
        self.cc_rows.extend(
            rows.iter()
                .enumerate()
                .filter_map(move |(i, x)| filter(x).then_some(i))
                .map(RowIdx),
        );

        let comparator = vwr.create_cell_comparator();
        for (sort_col, asc) in self.sort.iter().rev() {
            self.cc_rows.sort_by(|a, b| {
                comparator(&rows[a.0], &rows[b.0], sort_col.0).tap_mut(|x| {
                    if !asc.0 {
                        *x = x.reverse()
                    }
                })
            });
        }

        // Just refill with neat default height.
        self.cc_row_heights.resize(self.cc_rows.len(), 20.0);

        self.cc_row_id_to_vis.clear();
        self.cc_row_id_to_vis.extend(
            self.cc_rows
                .iter()
                .enumerate()
                .map(|(i, id)| (*id, VisRowPos(i))),
        );

        if self.handle_desired_selection() {
            // no-op.
        } else if let CursorState::Select(cursor) = &mut self.cc_cursor {
            // Validate cursor range if it's still in range.

            let old_cols = self.cc_prev_n_columns;
            let new_rows = self.cc_rows.len();
            let new_cols = self.num_columns;
            self.cc_prev_n_columns = self.num_columns;

            cursor.retain_mut(|sel| {
                let (old_min_r, old_min_c) = sel.0.row_col(old_cols);
                if old_min_r.0 >= new_rows || old_min_c.0 >= new_cols {
                    return false;
                }

                let (mut old_max_r, mut old_max_c) = sel.1.row_col(old_cols);
                old_max_r.0 = old_max_r.0.min(new_rows.saturating_sub(1));
                old_max_c.0 = old_max_c.0.min(new_cols.saturating_sub(1));

                let min = old_min_r.linear_index(new_cols, old_min_c);
                let max = old_max_r.linear_index(new_cols, old_max_c);
                *sel = VisSelection(min, max);

                true
            });
        } else {
            self.cc_cursor = CursorState::Select(Vec::default());
        }

        // Prevent overflow.
        self.validate_interactive_cell(self.vis_cols.len());
    }

    fn handle_desired_selection(&mut self) -> bool {
        let Some((next_sel, sel)) = self.cc_desired_selection.take().and_then(|x| {
            if let CursorState::Select(vec) = &mut self.cc_cursor {
                Some((x, vec))
            } else {
                None
            }
        }) else {
            return false;
        };

        // If there's any desired selections present for next validation, apply it.

        sel.clear();
        let ncol = self.vis_cols.len();

        for row_id in next_sel {
            let vis_row = self.cc_row_id_to_vis[&row_id];
            let p_left = vis_row.linear_index(ncol, VisColumnPos(0));
            let p_right = vis_row.linear_index(ncol, VisColumnPos(ncol - 1));

            sel.push(VisSelection(p_left, p_right));
        }

        true
    }

    pub fn force_mark_dirty(&mut self) {
        self.cc_dirty = true;
    }

    pub fn row_editing_cell(&mut self, row_id: RowIdx) -> Option<(bool, VisColumnPos)> {
        match &mut self.cc_cursor {
            CursorState::Edit {
                row,
                last_focus,
                next_focus,
                ..
            } if *row == row_id => Some((replace(next_focus, false), *last_focus)),
            _ => None,
        }
    }

    pub fn num_columns(&self) -> usize {
        self.num_columns
    }

    pub fn sort(&self) -> &[(ColumnIdx, IsAscending)] {
        &self.sort
    }

    pub fn unwrap_editing_row_data(&mut self) -> &mut R {
        match &mut self.cc_cursor {
            CursorState::Edit { edition, .. } => edition,
            _ => unreachable!(),
        }
    }

    pub fn is_editing(&self) -> bool {
        matches!(self.cc_cursor, CursorState::Edit { .. })
    }

    pub fn is_selected(&self, row: VisRowPos, col: VisColumnPos) -> bool {
        if let CursorState::Select(selections) = &self.cc_cursor {
            selections
                .iter()
                .any(|sel| self.vis_sel_contains(*sel, row, col))
        } else {
            false
        }
    }

    pub fn is_selected_cci(&self, row: VisRowPos, col: VisColumnPos) -> bool {
        self.cci_selection.is_some_and(|(pivot, current)| {
            self.vis_sel_contains(
                VisSelection::from_points(self.vis_cols.len(), pivot, current),
                row,
                col,
            )
        })
    }

    pub fn is_interactive_row(&self, row: VisRowPos) -> Option<VisColumnPos> {
        let (r, c) = self.cc_interactive_cell.row_col(self.vis_cols.len());
        (r == row).then_some(c)
    }

    pub fn interactive_cell(&self) -> (VisRowPos, VisColumnPos) {
        self.cc_interactive_cell.row_col(self.vis_cols.len())
    }

    pub fn cci_sel_update(&mut self, current: VisLinearIdx) {
        if let Some((_, pivot)) = &mut self.cci_selection {
            *pivot = current;
        } else {
            self.cci_selection = Some((current, current));
        }
    }

    pub fn cci_sel_update_row(&mut self, row: VisRowPos) {
        [0, self.vis_cols.len() - 1].map(|col| {
            self.cci_sel_update(row.linear_index(self.vis_cols.len(), VisColumnPos(col)))
        });
    }

    pub fn has_cci_selection(&self) -> bool {
        self.cci_selection.is_some()
    }

    pub fn vis_sel_contains(&self, sel: VisSelection, row: VisRowPos, col: VisColumnPos) -> bool {
        sel.contains(self.vis_cols.len(), row, col)
    }

    pub fn push_new_command<V: RowViewer<R>>(
        &mut self,
        table: &mut DataTable<R>,
        vwr: &mut V,
        command: Command<R>,
        capacity: usize,
    ) {
        if self.is_editing() && !matches!(command, Command::CcCancelEdit | Command::CcCommitEdit) {
            // If any non-editing command is pushed while editing, commit it first
            self.push_new_command(table, vwr, Command::CcCommitEdit, capacity);
        }

        // Generate redo argument from command
        let restore = match command {
            Command::CcHideColumn(column_idx) => {
                if self.vis_cols.len() == 1 {
                    return;
                }

                let mut vis_cols = self.vis_cols.clone();
                let idx = vis_cols.iter().position(|x| *x == column_idx).unwrap();
                vis_cols.remove(idx);

                self.push_new_command(table, vwr, Command::SetVisibleColumns(vis_cols), capacity);
                return;
            }
            Command::CcShowColumn { what, at } => {
                assert!(self.vis_cols.iter().all(|x| *x != what));

                let mut vis_cols = self.vis_cols.clone();
                vis_cols.insert(at.0, what);

                self.push_new_command(table, vwr, Command::SetVisibleColumns(vis_cols), capacity);
                return;
            }
            Command::SetVisibleColumns(ref value) => {
                if self.vis_cols.iter().eq(value.iter()) {
                    return;
                }

                vec![Command::SetVisibleColumns(self.vis_cols.clone())]
            }
            Command::CcReorderColumn { from, to } => {
                if from == to || to.0 > self.vis_cols.len() {
                    // Reorder may deliver invalid parameter if there's multiple data
                    // tables present at the same time; as the drag drop payload are
                    // compatible between different tables...
                    return;
                }

                let mut vis_cols = self.vis_cols.clone();
                if from.0 < to.0 {
                    vis_cols.insert(to.0, vis_cols[from.0]);
                    vis_cols.remove(from.0);
                } else {
                    vis_cols.remove(from.0).pipe(|x| vis_cols.insert(to.0, x));
                }

                self.push_new_command(table, vwr, Command::SetVisibleColumns(vis_cols), capacity);
                return;
            }
            Command::CcEditStart(row_id, column_pos, current) => {
                // EditStart command is directly applied.
                self.cc_cursor = CursorState::Edit {
                    edition: *current,
                    next_focus: true,
                    last_focus: column_pos,
                    row: row_id,
                };

                // Update interactive cell.
                self.cc_interactive_cell =
                    self.cc_row_id_to_vis[&row_id].linear_index(self.vis_cols.len(), column_pos);

                // No redo argument is generated.
                return;
            }
            ref cmd @ (Command::CcCancelEdit | Command::CcCommitEdit) => {
                // This edition state become selection. Restorat
                let Some((row_id, edition, _)) = self.try_take_edition() else {
                    return;
                };

                if matches!(cmd, Command::CcCancelEdit) {
                    // Cancellation does not affect to any state.
                    return;
                }

                // Change command type of self.
                self.push_new_command(
                    table,
                    vwr,
                    Command::SetRowValue(row_id, edition.into()),
                    capacity,
                );

                return;
            }

            Command::SetRowValue(row_id, _) => {
                vec![Command::SetRowValue(
                    row_id,
                    vwr.clone_row(&table.rows[row_id.0]).into(),
                )]
            }

            Command::SetCells { ref values, .. } => {
                let mut keys = Vec::from_iter(values.iter().map(|(r, ..)| *r));
                keys.dedup();

                keys.iter()
                    .map(|row_id| {
                        Command::SetRowValue(*row_id, vwr.clone_row(&table.rows[row_id.0]).into())
                    })
                    .collect()
            }

            Command::SetColumnSort(ref sort) => {
                if self.sort.iter().eq(sort.iter()) {
                    return;
                }

                vec![Command::SetColumnSort(self.sort.clone())]
            }
            Command::CcSetSelection(sel) => {
                if !sel.is_empty() {
                    self.cc_interactive_cell = sel[0].0;
                }

                self.cc_cursor = CursorState::Select(sel);
                return;
            }
            Command::InsertRows(pivot, ref values) => {
                let values = (pivot.0..pivot.0 + values.len()).map(RowIdx).collect();
                vec![Command::RemoveRow(values)]
            }
            Command::RemoveRow(ref indices) => {
                let values = indices
                    .iter()
                    .map(|x| vwr.clone_row(&table.rows[x.0]))
                    .collect();
                vec![Command::InsertRows(RowIdx(indices[0].0), values)]
            }
            Command::SetCell(_, row_id, _) => {
                vec![Command::SetRowValue(
                    row_id,
                    vwr.clone_row(&table.rows[row_id.0]).into(),
                )]
            }
        };

        // Discard all redos after this point.
        self.undo_queue.drain(0..self.undo_cursor);

        // Discard all undos that exceed the capacity.
        let new_len = capacity.saturating_sub(1).min(self.undo_queue.len());
        self.undo_queue.drain(new_len..);

        // Now it's the foremost element of undo queue.
        self.undo_cursor = 0;

        // Apply the command.
        self.cmd_apply(table, vwr, &command);

        // Push the command to the queue.
        self.undo_queue.push_front(UndoArg {
            apply: command,
            restore,
        });
    }

    fn cmd_apply<V: RowViewer<R>>(
        &mut self,
        table: &mut DataTable<R>,
        vwr: &mut V,
        cmd: &Command<R>,
    ) {
        match cmd {
            Command::SetVisibleColumns(cols) => {
                self.validate_interactive_cell(cols.len());
                self.vis_cols.clear();
                self.vis_cols.extend(cols.iter().cloned());
                self.cc_dirty = true;
            }
            Command::SetColumnSort(new_sort) => {
                self.sort.clear();
                self.sort.extend(new_sort.iter().cloned());
                self.cc_dirty = true;
            }
            Command::SetRowValue(row_id, value) => {
                self.cc_num_frame_from_last_edit = 0;
                table.dirty_flag = true;
                table.rows[row_id.0] = vwr.clone_row(value);
            }
            Command::SetCell(value, row, col) => {
                self.cc_num_frame_from_last_edit = 0;
                table.dirty_flag = true;
                vwr.set_cell_value(value, &mut table.rows[row.0], col.0);
            }
            Command::SetCells { slab, values } => {
                self.cc_num_frame_from_last_edit = 0;
                table.dirty_flag = true;

                for (row, col, value_id) in values.iter() {
                    vwr.set_cell_value(&slab[value_id.0], &mut table.rows[row.0], col.0);
                }
            }
            Command::InsertRows(pos, values) => {
                self.cc_dirty = true; // It invalidates all current `RowId` occurences.
                table.dirty_flag = true;

                table
                    .rows
                    .splice(pos.0..pos.0, values.iter().map(|x| vwr.clone_row(x)));

                self.queue_select_rows((pos.0..pos.0 + values.len()).map(RowIdx));
            }
            Command::RemoveRow(values) => {
                debug_assert!(values.windows(2).all(|x| x[0] < x[1]));
                self.cc_dirty = true; // It invalidates all current `RowId` occurences.
                table.dirty_flag = true;

                let mut index = 0;
                table.rows.retain(|_| {
                    let idx_now = index.tap(|_| index += 1);
                    values.binary_search(&RowIdx(idx_now)).is_err()
                });

                self.queue_select_rows([]);
            }
            Command::CcHideColumn(_)
            | Command::CcShowColumn { .. }
            | Command::CcReorderColumn { .. }
            | Command::CcEditStart(..)
            | Command::CcCommitEdit
            | Command::CcCancelEdit
            | Command::CcSetSelection(_) => unreachable!(),
        }
    }

    fn queue_select_rows(&mut self, rows: impl IntoIterator<Item = RowIdx>) {
        self.cc_desired_selection = Some(rows.into_iter().collect());
    }

    fn validate_interactive_cell(&mut self, new_num_column: usize) {
        let (r, c) = self.cc_interactive_cell.row_col(self.vis_cols.len());
        let rmax = self.cc_rows.len().saturating_sub(1);
        let clen = self.vis_cols.len();

        self.cc_interactive_cell =
            VisLinearIdx(r.0.min(rmax) * clen + c.0.min(new_num_column.saturating_sub(1)));
    }

    pub fn has_clipboard_contents(&self) -> bool {
        self.clipboard.is_some()
    }

    pub fn has_undo(&self) -> bool {
        self.undo_cursor < self.undo_queue.len()
    }

    pub fn has_redo(&self) -> bool {
        self.undo_cursor > 0
    }

    pub fn cursor_as_selection(&self) -> Option<&[VisSelection]> {
        match &self.cc_cursor {
            CursorState::Select(x) => Some(x),
            CursorState::Edit { .. } => None,
        }
    }

    fn try_take_edition(&mut self) -> Option<(RowIdx, R, VisColumnPos)> {
        if matches!(self.cc_cursor, CursorState::Edit { .. }) {
            match replace(&mut self.cc_cursor, CursorState::Select(Vec::default())) {
                CursorState::Edit {
                    row,
                    edition,
                    last_focus,
                    ..
                } => Some((row, edition, last_focus)),
                _ => unreachable!(),
            }
        } else {
            None
        }
    }

    pub fn ui_action_context(&self) -> UiActionContext {
        UiActionContext {
            cursor: match &self.cc_cursor {
                CursorState::Select(x) => {
                    if x.is_empty() {
                        UiCursorState::Idle
                    } else if x.len() == 1 && x[0].0 == x[0].1 {
                        UiCursorState::SelectOne
                    } else {
                        UiCursorState::SelectMany
                    }
                }
                CursorState::Edit { .. } => UiCursorState::Editing,
            },
        }
    }

    pub fn undo<V: RowViewer<R>>(&mut self, table: &mut DataTable<R>, vwr: &mut V) -> bool {
        if self.undo_cursor == self.undo_queue.len() {
            return false;
        }

        let queue = take(&mut self.undo_queue);
        {
            let item = &queue[self.undo_cursor];
            for cmd in item.restore.iter() {
                self.cmd_apply(table, vwr, cmd);
            }
            self.undo_cursor += 1;
        }
        self.undo_queue = queue;

        true
    }

    pub fn redo<V: RowViewer<R>>(&mut self, table: &mut DataTable<R>, vwr: &mut V) -> bool {
        if self.undo_cursor == 0 {
            return false;
        }

        let queue = take(&mut self.undo_queue);
        {
            self.undo_cursor -= 1;
            self.cmd_apply(table, vwr, &queue[self.undo_cursor].apply);
        }
        self.undo_queue = queue;

        true
    }

    pub fn set_interactive_cell(&mut self, row: VisRowPos, col: VisColumnPos) {
        self.cc_interactive_cell = row.linear_index(self.vis_cols.len(), col);
    }

    pub fn try_apply_ui_action(
        &mut self,
        table: &mut DataTable<R>,
        vwr: &mut impl RowViewer<R>,
        action: UiAction,
    ) -> Vec<Command<R>> {
        fn empty<T, R>(_: T) -> Vec<Command<R>> {
            default()
        }

        self.cci_want_move_scroll = true;

        let (ic_r, ic_c) = self.cc_interactive_cell.row_col(self.vis_cols.len());
        match action {
            UiAction::SelectionStartEditing => {
                let row_id = self.cc_rows[ic_r.0];
                let row = vwr.clone_row(&table.rows[row_id.0]);
                vec![Command::CcEditStart(row_id, ic_c, Box::new(row))]
            }
            UiAction::CancelEdition => vec![Command::CcCancelEdit],
            UiAction::CommitEdition => vec![Command::CcCommitEdit],
            UiAction::CommitEditionAndMove(dir) => {
                let pos = self.moved_position(self.cc_interactive_cell, dir);
                let (r, c) = pos.row_col(self.vis_cols.len());
                let row_id = self.cc_rows[r.0];
                let row_value = if self.is_editing() && ic_r == r {
                    vwr.clone_row(self.unwrap_editing_row_data())
                } else {
                    vwr.clone_row(&table.rows[row_id.0])
                };

                vec![
                    Command::CcCommitEdit,
                    Command::CcEditStart(row_id, c, row_value.into()),
                ]
            }
            UiAction::MoveSelection(dir) => {
                let pos = self.moved_position(self.cc_interactive_cell, dir);
                vec![Command::CcSetSelection(vec![VisSelection(pos, pos)])]
            }
            UiAction::Undo => self.undo(table, vwr).pipe(empty),
            UiAction::Redo => self.redo(table, vwr).pipe(empty),
            UiAction::CopySelection | UiAction::CutSelection => {
                let sels = self.collect_selection();
                self.clipboard = None;

                if sels.is_empty() {
                    return vec![]; // we do nothing.
                }

                // Copy contents to clipboard
                let offset = sels.first().unwrap().0;
                let mut slab = Vec::with_capacity(10);
                let mut vis_map = HashMap::with_capacity(10);

                for vis_row in self.collect_selected_rows() {
                    vis_map.insert(vis_row, slab.len());
                    slab.push(vwr.clone_row(&table.rows[self.cc_rows[vis_row.0].0]));
                }

                self.clipboard = Some(Clipboard {
                    slab: slab.into_boxed_slice(),
                    pastes: sels
                        .iter()
                        .map(|(v_r, v_c)| {
                            (
                                VisRowOffset(v_r.0 - offset.0),
                                self.vis_cols[v_c.0],
                                RowSlabIndex(vis_map[&v_r]),
                            )
                        })
                        .collect(),
                });

                // TODO: Interact with system clipboard?
                // - Then we need a way to serialize contents to string.

                if action == UiAction::CutSelection {
                    self.try_apply_ui_action(table, vwr, UiAction::DeleteSelection)
                } else {
                    vec![]
                }
            }
            UiAction::SelectionDuplicateValues => {
                let pivot_row = vwr.clone_row(&table.rows[self.cc_rows[ic_r.0].0]);
                let sels = self.collect_selection();

                vec![Command::SetCells {
                    slab: [pivot_row].into(),
                    values: sels
                        .into_iter()
                        .map(|(r, c)| (self.cc_rows[r.0], self.vis_cols[c.0], RowSlabIndex(0)))
                        .collect(),
                }]
            }
            UiAction::PasteInPlace => {
                let Some(clip) = &self.clipboard else {
                    return vec![];
                };

                let values =
                    Vec::from_iter(clip.pastes.iter().filter_map(|(offset, col, slab_id)| {
                        let vis_r = VisRowPos(ic_r.0 + offset.0);
                        (vis_r.0 < self.cc_rows.len())
                            .then(|| (self.cc_rows[vis_r.0], *col, *slab_id))
                    }));

                let rows = Vec::from_iter(values.iter().map(|(r, ..)| *r)).tap_mut(|x| x.dedup());
                self.cc_desired_selection = Some(rows);

                vec![Command::SetCells {
                    slab: clip.slab.iter().map(|x| vwr.clone_row(x)).collect(),
                    values: values.into_boxed_slice(),
                }]
            }
            UiAction::PasteInsert => {
                let Some(clip) = &self.clipboard else {
                    return vec![];
                };

                let mut last = usize::MAX;
                let mut rows = clip
                    .pastes
                    .iter()
                    .filter(|&(offset, ..)| replace(&mut last, offset.0) != offset.0)
                    .map(|(offset, ..)| (*offset, vwr.new_empty_row()))
                    .collect::<BTreeMap<_, _>>();

                for (offset, column, slab_id) in &*clip.pastes {
                    vwr.set_cell_value(
                        &clip.slab[slab_id.0],
                        rows.get_mut(offset).unwrap(),
                        column.0,
                    );
                }

                let pos = if self.sort.is_empty() {
                    self.cc_rows[ic_r.0]
                } else {
                    RowIdx(table.rows.len())
                };

                let row_values = rows.into_values().collect();
                vec![Command::InsertRows(pos, row_values)]
            }
            UiAction::DuplicateRow => {
                let rows = self
                    .collect_selected_rows()
                    .into_iter()
                    .map(|x| self.cc_rows[x.0])
                    .map(|r| vwr.clone_row(&table.rows[r.0]))
                    .collect();

                let pos = if self.sort.is_empty() {
                    self.cc_rows[ic_r.0]
                } else {
                    RowIdx(table.rows.len())
                };

                vec![Command::InsertRows(pos, rows)]
            }
            UiAction::DeleteSelection => {
                let default = vwr.new_empty_row();
                let sels = self.collect_selection();
                let slab = vec![default].into_boxed_slice();

                vec![Command::SetCells {
                    slab,
                    values: sels
                        .into_iter()
                        .map(|(r, c)| (self.cc_rows[r.0], self.vis_cols[c.0], RowSlabIndex(0)))
                        .collect(),
                }]
            }
            UiAction::DeleteRow => {
                let rows = self
                    .collect_selected_rows()
                    .into_iter()
                    .map(|x| self.cc_rows[x.0])
                    .collect();

                vec![Command::RemoveRow(rows)]
            }
            UiAction::SelectAll => {
                if self.cc_rows.is_empty() {
                    return vec![];
                }

                vec![Command::CcSetSelection(vec![VisSelection(
                    VisLinearIdx(0),
                    VisRowPos(self.cc_rows.len().saturating_sub(1))
                        .linear_index(self.vis_cols.len(), VisColumnPos(self.vis_cols.len() - 1)),
                )])]
            }

            action @ (UiAction::NavPageDown
            | UiAction::NavPageUp
            | UiAction::NavTop
            | UiAction::NavBottom) => {
                let ofst = match action {
                    UiAction::NavPageDown => self.cci_page_row_count as isize,
                    UiAction::NavPageUp => -(self.cci_page_row_count as isize),
                    UiAction::NavTop => isize::MIN,
                    UiAction::NavBottom => isize::MAX,
                    _ => unreachable!(),
                };

                let new_ic_r = (ic_r.0 as isize)
                    .saturating_add(ofst)
                    .clamp(0, self.cc_rows.len().saturating_sub(1) as _);
                self.cc_interactive_cell =
                    VisLinearIdx(new_ic_r as usize * self.vis_cols.len() + ic_c.0);

                self.validate_interactive_cell(self.vis_cols.len());
                vec![Command::CcSetSelection(vec![VisSelection(
                    self.cc_interactive_cell,
                    self.cc_interactive_cell,
                )])]
            }
        }
    }

    fn collect_selection(&self) -> BTreeSet<(VisRowPos, VisColumnPos)> {
        let mut set = BTreeSet::new();

        if let CursorState::Select(selections) = &self.cc_cursor {
            for sel in selections.iter() {
                let (top, left) = sel.0.row_col(self.vis_cols.len());
                let (bottom, right) = sel.1.row_col(self.vis_cols.len());

                for r in top.0..=bottom.0 {
                    for c in left.0..=right.0 {
                        set.insert((VisRowPos(r), VisColumnPos(c)));
                    }
                }
            }
        }

        set
    }

    fn collect_selected_rows(&self) -> BTreeSet<VisRowPos> {
        let mut rows = BTreeSet::new();

        if let CursorState::Select(selections) = &self.cc_cursor {
            for sel in selections.iter() {
                let (top, _) = sel.0.row_col(self.vis_cols.len());
                let (bottom, _) = sel.1.row_col(self.vis_cols.len());

                for r in top.0..=bottom.0 {
                    rows.insert(VisRowPos(r));
                }
            }
        }

        rows
    }

    fn moved_position(&self, pos: VisLinearIdx, dir: MoveDirection) -> VisLinearIdx {
        let (VisRowPos(r), VisColumnPos(c)) = pos.row_col(self.vis_cols.len());

        let (rmax, cmax) = (
            self.cc_rows.len().saturating_sub(1),
            self.vis_cols.len().saturating_sub(1),
        );

        let (nr, nc) = match dir {
            MoveDirection::Up => match (r, c) {
                (0, c) => (0, c),
                (r, c) => (r - 1, c),
            },
            MoveDirection::Down => match (r, c) {
                (r, c) if r == rmax => (r, c),
                (r, c) => (r + 1, c),
            },
            MoveDirection::Left => match (r, c) {
                (0, 0) => (0, 0),
                (r, 0) => (r - 1, cmax),
                (r, c) => (r, c - 1),
            },
            MoveDirection::Right => match (r, c) {
                (r, c) if r == rmax && c == cmax => (r, c),
                (r, c) if c == cmax => (r + 1, 0),
                (r, c) => (r, c + 1),
            },
        };

        VisLinearIdx(nr * self.vis_cols.len() + nc)
    }

    pub fn cci_take_selection(&mut self, mods: egui::Modifiers) -> Option<Vec<VisSelection>> {
        let ncol = self.vis_cols.len();
        let cci_sel = self
            .cci_selection
            .take()
            .map(|(_0, _1)| VisSelection::from_points(ncol, _0, _1))?;

        if mods.is_none() {
            return Some(vec![cci_sel]);
        }

        let mut sel = self.cursor_as_selection().unwrap_or_default().to_owned();
        let idx_contains = sel.iter().position(|x| x.contains_rect(ncol, cci_sel));
        if sel.is_empty() {
            sel.push(cci_sel);
            return Some(sel);
        }

        if mods.command_only() {
            if let Some(idx) = idx_contains {
                sel.remove(idx);
            } else {
                sel.push(cci_sel);
            }
        }

        if mods.cmd_ctrl_matches(Modifiers::SHIFT) {
            let last = sel.last_mut().unwrap();
            if cci_sel.is_point() && last.is_point() {
                *last = last.union(ncol, cci_sel);
            } else if idx_contains.is_none() {
                sel.push(cci_sel);
            };
        }

        Some(sel)
    }
}

/* ------------------------------------------ Commands ------------------------------------------ */

pub(crate) enum Command<R> {
    CcHideColumn(ColumnIdx),
    CcShowColumn {
        what: ColumnIdx,
        at: VisColumnPos,
    },
    CcReorderColumn {
        from: VisColumnPos,
        to: VisColumnPos,
    },

    SetColumnSort(Vec<(ColumnIdx, IsAscending)>),
    SetVisibleColumns(Vec<ColumnIdx>),

    CcSetSelection(Vec<VisSelection>), // Cache - Set Selection

    SetRowValue(RowIdx, Box<R>),
    SetCells {
        slab: Box<[R]>,
        values: Box<[(RowIdx, ColumnIdx, RowSlabIndex)]>,
    },
    SetCell(Box<R>, RowIdx, ColumnIdx),

    InsertRows(RowIdx, Box<[R]>),
    RemoveRow(Vec<RowIdx>),

    CcEditStart(RowIdx, VisColumnPos, Box<R>),
    CcCancelEdit,
    CcCommitEdit,
}
