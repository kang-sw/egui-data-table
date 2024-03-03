use std::{
    collections::VecDeque,
    hash::Hasher,
    mem::{replace, take},
    sync::Arc,
};

use egui::ahash::{AHasher, HashMap};
use tap::prelude::{Pipe, Tap};

use crate::{
    default,
    viewer::{MoveDirection, UiActionContext, UiCursorState},
    RowViewer, Spreadsheet, UiAction,
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
    struct ColumnIdx(usize);
    struct RowId(usize);
    struct VisRowPos(usize);
    struct VisColumnPos(usize);
    struct IsAscending(bool);
);

impl VisSelection {
    pub fn contains(&self, ncol: usize, row: VisRowPos, col: VisColumnPos) -> bool {
        let (top, left) = self.0.row_col(ncol);
        let (bottom, right) = self.1.row_col(ncol);

        row.0 >= top.0 && row.0 <= bottom.0 && col.0 >= left.0 && col.0 <= right.0
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

    // pub fn from_row_col(ncol: usize, r: VisRowPos, c: VisColumnPos) -> Self {
    //     r.linear_index(ncol, c).pipe(|idx| Self(idx, idx))
    // }
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

pub(crate) struct UiState<R> {
    /// Cached number of columns.
    num_columns: usize,

    /// Type id of the viewer.
    viewer_type: std::any::TypeId,

    /// Unique hash of the viewer. This is to prevent cache invalidation when the viewer
    /// state is changed.
    viewer_hash: u64,

    /// Visible columns selected by user.
    pub vis_cols: Vec<ColumnIdx>,

    /// Column sorting state.
    sort: Vec<(ColumnIdx, IsAscending)>,

    /// Latest interactive cell; Used for keyboard navigation.
    cc_interactive_cell: VisLinearIdx,

    /// Undo queue.
    ///
    /// - Push tasks front of the queue.
    /// - Drain all elements from `0..undo_cursor`
    /// - Pop overflow elements from the back.
    undo_queue: VecDeque<UndoArg<R>>,

    /// Undo cursor => increment by 1 on every undo, decrement by 1 on redo.
    undo_cursor: usize,

    /// TODO: Clipboard

    /*

        SECTION: Cache - Rendering

    */
    /// Cached rows.
    pub cc_rows: Vec<(RowId, f32)>,
    cc_row_id_to_vis: HashMap<RowId, VisRowPos>,

    /// Spreadsheet is modified during the last validation.
    cc_dirty: bool,

    /// Row selections. First element's top-left corner is always 'highlight' row if
    /// editing row isn't present.
    cc_cursor: CursorState<R>,

    /// Number of frames from the last edit. Used to validate sorting.
    cc_num_frame_from_last_edit: usize,

    /*

        SECTION: Cache - Input Status

    */
    /// (Pivot, Current) selection.
    cci_selection: Option<(VisLinearIdx, VisLinearIdx)>,

    /// We have latest click.
    pub cci_has_focus: bool,
}

struct UndoArg<R> {
    apply: Command<R>,
    restore: Vec<Command<R>>,
}

impl<R> Default for UiState<R> {
    fn default() -> Self {
        Self {
            num_columns: 0,
            viewer_hash: 0,
            viewer_type: std::any::TypeId::of::<()>(),
            vis_cols: Vec::default(),
            sort: Vec::default(),
            cc_cursor: CursorState::Select(default()),
            undo_queue: VecDeque::default(),
            cc_rows: Vec::default(),
            cc_dirty: false,
            undo_cursor: 0,
            cci_selection: None,
            cci_has_focus: false,
            cc_interactive_cell: VisLinearIdx(0),
            cc_row_id_to_vis: default(),
            cc_num_frame_from_last_edit: 0,
        }
    }
}

enum CursorState<R> {
    Select(Vec<VisSelection>),
    Edit {
        next_focus: bool,
        last_focus: VisColumnPos,
        row: RowId,
        edition: R,
    },
}

impl<R: Send + Clone> UiState<R> {
    pub fn validate_identity<V: RowViewer<R>>(&mut self, vwr: &mut V) {
        let num_columns = vwr.num_columns();
        let vwr_type_id = std::any::TypeId::of::<V>();
        let vwr_hash = AHasher::default().pipe(|mut hsh| {
            // TODO: When non-static type id  <br/>
            // std::any::TypeId::of::<V>().hash(&mut hsh);

            vwr.hash(&mut hsh);
            hsh.finish()
        });

        if self.num_columns == num_columns && self.viewer_type == vwr_type_id {
            if self.viewer_hash != vwr_hash {
                self.viewer_hash = vwr_hash;
                self.cc_dirty = true;
            }

            if !self.is_editing() {
                self.cc_num_frame_from_last_edit += 1;
            }

            if self.cc_num_frame_from_last_edit == 2 {
                // When finished editing, if there's any sorting, we should validate the cache.
                self.cc_dirty |= !self.sort.is_empty() || vwr.has_row_filter();
            }

            return;
        }

        // Clear the cache
        *self = Default::default();
        self.viewer_type = vwr_type_id;
        self.viewer_hash = vwr_hash;
        self.num_columns = num_columns;

        self.vis_cols.extend((0..num_columns).map(ColumnIdx));
        self.cc_dirty = true;
    }

    pub fn validate_cc<V: RowViewer<R>>(&mut self, rows: &mut VecDeque<R>, vwr: &mut V) {
        if !replace(&mut self.cc_dirty, false) {
            return;
        }

        // TODO: Boost performance with `rayon`

        // We should validate the entire cache.
        let mut it_all_rows = rows
            .iter()
            .enumerate()
            .filter_map(|(i, x)| vwr.filter_row(x).then_some(i))
            .map(RowId);

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
            self.cc_rows.push((row, 20.)); // Just neat default value.
        }

        // TODO: Sort rows by column
        for (sort_col, asc) in self.sort.iter().rev() {
            self.cc_rows.sort_by(|(a, _), (b, _)| {
                vwr.compare_column_for_sort(&rows[a.0], &rows[b.0], sort_col.0)
                    .tap_mut(|x| {
                        if !asc.0 {
                            *x = x.reverse()
                        }
                    })
            });
        }

        self.cc_row_id_to_vis.clear();
        self.cc_row_id_to_vis.extend(
            self.cc_rows
                .iter()
                .enumerate()
                .map(|(i, (id, ..))| (*id, VisRowPos(i))),
        );

        // Clear selection.
        self.cc_cursor = CursorState::Select(Vec::default());

        // Prevent overflow.
        self.validate_interactive_cell(self.vis_cols.len());
    }

    /// - `None`: Not editing this row
    /// - `Some(true)`: Freshly started editing
    /// - `Some(false)`: Already editing
    pub fn row_is_fresh_edit(&mut self, row_id: RowId) -> Option<Option<VisColumnPos>> {
        let CursorState::Edit {
            next_focus,
            row,
            last_focus,
            ..
        } = &mut self.cc_cursor
        else {
            return None;
        };

        if *row != row_id {
            return None;
        }

        Some(replace(next_focus, false).then_some(*last_focus))
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

    pub fn cci_sel_take(&mut self) -> Option<VisSelection> {
        self.cci_selection
            .take()
            .map(|(pivot, current)| VisSelection::from_points(self.vis_cols.len(), pivot, current))
    }

    pub fn has_cci_selection(&self) -> bool {
        self.cci_selection.is_some()
    }

    pub fn vis_sel_contains(&self, sel: VisSelection, row: VisRowPos, col: VisColumnPos) -> bool {
        sel.contains(self.vis_cols.len(), row, col)
    }

    pub fn push_new_command(
        &mut self,
        sheet: &mut Spreadsheet<R>,
        mut command: Command<R>,
        capacity: usize,
    ) {
        if self.is_editing() && !matches!(command, Command::CancelEdit | Command::CommitEdit) {
            // If any non-editing command is pushed while editing, commit it first
            self.push_new_command(sheet, Command::CommitEdit, capacity);
        }

        // Generate redo argument from command
        let restore = match command {
            Command::Noop => unimplemented!("Do not make noop command manually!"),
            Command::HideColumn(column_idx) => {
                if self.vis_cols.len() == 1 {
                    return;
                }

                let mut vis_cols = self.vis_cols.clone();
                let idx = vis_cols.iter().position(|x| *x == column_idx).unwrap();
                vis_cols.remove(idx);

                command = Command::SetVisibleColumns(vis_cols);
                vec![Command::SetVisibleColumns(self.vis_cols.clone())]
            }
            Command::ShowColumn { what, at } => {
                assert!(self.vis_cols.iter().all(|x| *x != what));

                let mut vis_cols = self.vis_cols.clone();
                vis_cols.insert(at.0, what);

                command = Command::SetVisibleColumns(vis_cols);
                vec![Command::SetVisibleColumns(self.vis_cols.clone())]
            }
            Command::SetVisibleColumns(ref value) => {
                if self.vis_cols.iter().eq(value.iter()) {
                    return;
                }

                vec![Command::SetVisibleColumns(self.vis_cols.clone())]
            }
            Command::ReorderColumn { from, to } => {
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

                command = Command::SetVisibleColumns(vis_cols);
                vec![Command::SetVisibleColumns(self.vis_cols.clone())]
            }
            Command::EditStart(row_id, column_pos, current) => {
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
            ref cmd @ (Command::CancelEdit | Command::CommitEdit) => {
                // This edition state become selection. Restorat
                let Some((row_id, edition, _)) = self.try_take_edition() else {
                    // TODO: Errmsg - No row is being edited
                    return;
                };

                if matches!(cmd, Command::CancelEdit) {
                    // Cancellation does not affect to any state.
                    return;
                }

                command = Command::SetRowValue(row_id, edition.into());

                // Restoration become simple selection.
                vec![Command::SetRowValue(
                    row_id,
                    sheet.rows[row_id.0].clone().into(),
                )]
            }
            Command::SetRowValue(_, _) => todo!(),
            Command::SetRowValues(_) => todo!(),
            Command::SetCells(_) => todo!(),
            Command::SetColumnSort(ref sort) => {
                if self.sort.iter().eq(sort.iter()) {
                    return;
                }

                vec![Command::SetColumnSort(self.sort.clone())]
            }
            Command::CacheSetSelection(sel) => {
                self.cc_cursor = CursorState::Select(sel);
                return;
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
        self.cmd_apply(sheet, &command);

        // Push the command to the queue.
        self.undo_queue.push_front(UndoArg {
            apply: command,
            restore,
        });
    }

    fn cmd_apply(&mut self, sheet: &mut Spreadsheet<R>, cmd: &Command<R>) {
        match cmd {
            Command::Noop => {}
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
            Command::HideColumn(_) => unreachable!(),
            Command::ShowColumn { .. } => unreachable!(),
            Command::ReorderColumn { .. } => unreachable!(),
            Command::EditStart(..) => unreachable!(),
            Command::CommitEdit | Command::CancelEdit => unreachable!(),
            Command::SetRowValue(row_id, value) => {
                self.cc_num_frame_from_last_edit = 0;
                sheet.dirty_flag = true;
                sheet.rows[row_id.0] = (**value).clone();
            }
            Command::SetRowValues(_) => {
                self.cc_num_frame_from_last_edit = 0;
                sheet.dirty_flag = true;
                todo!()
            }
            Command::SetCells(_) => {
                self.cc_num_frame_from_last_edit = 0;
                sheet.dirty_flag = true;
                todo!()
            }
            Command::CacheSetSelection(_) => unreachable!(),
        }
    }

    fn validate_interactive_cell(&mut self, new_num_column: usize) {
        let (r, c) = self.cc_interactive_cell.row_col(self.vis_cols.len());
        let rmax = self.cc_rows.len().saturating_sub(1);
        let cmax = self.vis_cols.len().saturating_sub(1);

        self.cc_interactive_cell =
            VisLinearIdx(r.0.min(rmax) * cmax + c.0.min(new_num_column.saturating_sub(1)));
    }

    fn take_unwrap_selection(&mut self) -> Vec<VisSelection> {
        match replace(&mut self.cc_cursor, CursorState::Select(Vec::default())) {
            CursorState::Select(selections) => selections,
            _ => unreachable!(),
        }
    }

    fn cursor_as_selection(&self) -> Option<&[VisSelection]> {
        match &self.cc_cursor {
            CursorState::Select(x) => Some(x),
            CursorState::Edit { .. } => None,
        }
    }

    fn try_take_edition(&mut self) -> Option<(RowId, R, VisColumnPos)> {
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

    pub fn undo(&mut self, sheet: &mut Spreadsheet<R>) -> bool {
        if self.undo_cursor == self.undo_queue.len() {
            return false;
        }

        let queue = take(&mut self.undo_queue);
        {
            let item = &queue[self.undo_cursor];
            for cmd in item.restore.iter() {
                self.cmd_apply(sheet, cmd);
            }
            self.undo_cursor += 1;
        }
        self.undo_queue = queue;

        true
    }

    pub fn redo(&mut self, sheet: &mut Spreadsheet<R>) -> bool {
        if self.undo_cursor == 0 {
            return false;
        }

        let queue = take(&mut self.undo_queue);
        {
            self.undo_cursor -= 1;
            self.cmd_apply(sheet, &queue[self.undo_cursor].apply);
        }
        self.undo_queue = queue;

        true
    }

    pub fn try_apply_ui_action(
        &mut self,
        sheet: &mut Spreadsheet<R>,
        viewer: &mut impl RowViewer<R>,
        action: UiAction,
    ) -> Vec<Command<R>> {
        fn empty<T, R>(_: T) -> Vec<Command<R>> {
            default()
        }

        match action {
            UiAction::SelectionStartEditing => {
                let (r, c) = self.cc_interactive_cell.row_col(self.vis_cols.len());
                let row_id = self.cc_rows[r.0].0;
                let row = sheet.rows[row_id.0].clone();
                vec![Command::EditStart(row_id, c, Box::new(row))]
            }
            UiAction::CancelEdition => vec![Command::CancelEdit],
            UiAction::CommitEdition => vec![Command::CommitEdit],
            UiAction::CommitEditionAndMove(dir) => {
                let (src_r, ..) = self.cc_interactive_cell.row_col(self.vis_cols.len());
                let pos = self.moved_position(self.cc_interactive_cell, dir);
                let (r, c) = pos.row_col(self.vis_cols.len());
                let row_id = self.cc_rows[r.0].0;
                let row_value = if self.is_editing() && src_r == r {
                    self.unwrap_editing_row_data().clone()
                } else {
                    sheet.rows[row_id.0].clone()
                };

                vec![
                    Command::CommitEdit,
                    Command::EditStart(row_id, c, row_value.into()),
                ]
            }
            UiAction::MoveSelection(_) => todo!(),
            UiAction::Undo => self.undo(sheet).pipe(empty),
            UiAction::Redo => self.redo(sheet).pipe(empty),
            UiAction::CopySelection => todo!(),
            UiAction::CutSelection => todo!(),
            UiAction::InsertRowBelow => todo!(),
            UiAction::InsertRowAbove => todo!(),
            UiAction::DuplicateRow => todo!(),
            UiAction::SelectionDuplicateValues => todo!(),
            UiAction::SelectionGenerateValues => todo!(),
        }
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

    pub fn add_noti_toast(&self, msg: impl Fn(&mut egui::Ui) + Send + 'static) {
        todo!()
    }

    pub fn log_warn(&self, msg: impl Into<String>) {
        let msg = msg.into();
        self.add_noti_toast(move |ui| {
            ui.label(msg.as_str());
        });
    }
}

/* ------------------------------------------ Commands ------------------------------------------ */

pub(crate) enum Command<R> {
    Noop,

    HideColumn(ColumnIdx),
    ShowColumn {
        what: ColumnIdx,
        at: VisColumnPos,
    },
    ReorderColumn {
        from: VisColumnPos,
        to: VisColumnPos,
    },

    SetColumnSort(Vec<(ColumnIdx, IsAscending)>),
    SetVisibleColumns(Vec<ColumnIdx>),

    CacheSetSelection(Vec<VisSelection>), // Cache - Set Selection

    SetRowValue(RowId, Box<R>),
    SetRowValues(Vec<(RowId, Arc<R>)>),
    SetCells(Vec<(RowId, Vec<ColumnIdx>, Arc<R>)>),

    EditStart(RowId, VisColumnPos, Box<R>),
    CancelEdit,
    CommitEdit,
}
