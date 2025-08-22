use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    hash::{Hash, Hasher},
    mem::{replace, take},
};
use std::collections::HashSet;
use egui::{
    ahash::{AHasher, HashMap, HashMapExt},
    Modifiers,
};
use itertools::Itertools;
use tap::prelude::{Pipe, Tap};

use crate::{
    default,
    draw::tsv,
    viewer::{
        CellWriteContext, DecodeErrorBehavior, EmptyRowCreateContext, MoveDirection, RowCodec,
        UiActionContext, UiCursorState,
    },
    DataTable, RowViewer, UiAction,
};

macro_rules! int_ty {
(
    $(#[$meta:meta])*
    struct $name:ident ($($ty:ty),+); $($rest:tt)*) => {
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, PartialOrd, Ord)]
    $(#[$meta])*
    pub(crate) struct $name($(pub(in crate::draw) $ty),+);

    int_ty!($($rest)*);
};
() => {}
}

int_ty!(
    struct VisLinearIdx(usize);
    struct VisSelection(VisLinearIdx, VisLinearIdx);
    struct RowSlabIndex(usize);

    struct RowIdx(usize);
    struct VisRowPos(usize);
    struct VisRowOffset(usize);
    struct VisColumnPos(usize);

    #[cfg_attr(feature = "persistency", derive(serde::Serialize, serde::Deserialize))]
    struct IsAscending(bool);
    #[cfg_attr(feature = "persistency", derive(serde::Serialize, serde::Deserialize))]
    struct ColumnIdx(usize);
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
    /// Type id of the viewer.
    viewer_type: std::any::TypeId,

    /// Unique hash of the viewer. This is to prevent cache invalidation when the viewer
    /// state is changed.
    viewer_filter_hash: u64,

    /// Undo queue.
    ///
    /// - Push tasks front of the queue.
    /// - Drain all elements from `0..undo_cursor`
    /// - Pop overflow elements from the back.
    undo_queue: VecDeque<UndoArg<R>>,

    /// Undo cursor => increment by 1 on every undo, decrement by 1 on redo.
    undo_cursor: usize,

    /// Clipboard contents.
    ///
    /// XXX: Should we move this into global storage?
    clipboard: Option<Clipboard<R>>,

    /// Persistent data
    p: PersistData,

    #[cfg(feature = "persistency")]
    is_p_loaded: bool,

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
    cc_desired_selection: Option<Vec<(RowIdx, Vec<ColumnIdx>)>>,

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

#[cfg_attr(feature = "persistency", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default)]
struct PersistData {
    /// Cached number of columns.
    num_columns: usize,

    /// Visible columns selected by user.
    vis_cols: Vec<ColumnIdx>,

    /// Column sorting state.
    sort: Vec<(ColumnIdx, IsAscending)>,
}

struct Clipboard<R> {
    slab: Box<[R]>,

    /// The first tuple element `VisRowPos` is offset from the top-left corner of the
    /// selection.
    pastes: Box<[(VisRowOffset, ColumnIdx, RowSlabIndex)]>,
}

impl<R> Clipboard<R> {
    pub fn sort(&mut self) {
        self.pastes
            .sort_by(|(a_row, a_col, ..), (b_row, b_col, ..)| {
                a_row.0.cmp(&b_row.0).then(a_col.0.cmp(&b_col.0))
            })
    }
}

struct UndoArg<R> {
    apply: Command<R>,
    restore: Vec<Command<R>>,
}

impl<R> Default for UiState<R> {
    fn default() -> Self {
        Self {
            viewer_filter_hash: 0,
            clipboard: None,
            viewer_type: std::any::TypeId::of::<()>(),
            cc_cursor: CursorState::Select(default()),
            undo_queue: VecDeque::new(),
            cc_rows: Vec::new(),
            cc_row_heights: Vec::new(),
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
            p: default(),
            #[cfg(feature = "persistency")]
            is_p_loaded: false,
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
    pub fn cc_is_dirty(&self) -> bool {
        self.cc_dirty
    }

    pub fn validate_identity<V: RowViewer<R>>(&mut self, vwr: &mut V) {
        let num_columns = vwr.num_columns();
        let vwr_type_id = std::any::TypeId::of::<V>();
        let vwr_hash = AHasher::default().pipe(|mut hsh| {
            vwr.row_filter_hash().hash(&mut hsh);
            hsh.finish()
        });

        // Check for nontrivial changes.
        if self.p.num_columns == num_columns && self.viewer_type == vwr_type_id {
            // Check for trivial changes which does not require total reconstruction of
            // UiState.

            // If viewer's filter is changed. It always invalidates current cache.
            if self.viewer_filter_hash != vwr_hash {
                self.viewer_filter_hash = vwr_hash;
                self.cc_dirty = true;
            }

            // Defer validation of cache if it's still editing. This is prevent annoying re-sort
            // during editing multiple cells in-a-row without escape from insertion mode.
            {
                if !self.is_editing() {
                    self.cc_num_frame_from_last_edit += 1;
                }

                if self.cc_num_frame_from_last_edit == 2 {
                    self.cc_dirty |= !self.p.sort.is_empty();
                }
            }

            // Check if any sort config is invalidated.
            self.cc_dirty |= {
                let mut any_sort_invalidated = false;

                self.p.sort.retain(|(c, _)| {
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
        self.p.num_columns = num_columns;

        self.p.vis_cols.extend((0..num_columns).map(ColumnIdx));
        self.cc_dirty = true;
    }

    #[cfg(feature = "persistency")]
    pub fn validate_persistency<V: RowViewer<R>>(
        &mut self,
        ctx: &egui::Context,
        ui_id: egui::Id,
        vwr: &mut V,
    ) {
        if !self.is_p_loaded {
            // Load initial storage status
            self.is_p_loaded = true;
            self.cc_dirty = true;
            let p: PersistData =
                ctx.memory_mut(|m| m.data.get_persisted(ui_id).unwrap_or_default());

            if p.num_columns == self.p.num_columns {
                // Data should only be copied when column count matches. Otherwise, we regard
                // stored column differs from the current.
                self.p = p;

                // Only retain valid sorting configuration.
                self.p.sort.retain(|(col, _)| vwr.is_sortable_column(col.0));
            }
        } else if self.cc_dirty {
            // Copy current ui status into persistency storage.
            ctx.memory_mut(|m| m.data.insert_persisted(ui_id, self.p.clone()));
        }
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
        self.cc_rows.clear();
        self.cc_rows.extend(
            rows.iter()
                .enumerate()
                .filter_map(|(i, x)| vwr.filter_row(x).then_some(i))
                .map(RowIdx),
        );

        for (sort_col, asc) in self.p.sort.iter().rev() {
            self.cc_rows.sort_by(|a, b| {
                vwr.compare_cell(&rows[a.0], &rows[b.0], sort_col.0)
                    .tap_mut(|x| {
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
            let new_cols = self.p.num_columns;
            self.cc_prev_n_columns = self.p.num_columns;

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
        self.validate_interactive_cell(self.p.vis_cols.len());
    }

    pub fn try_update_clipboard_from_string<V: RowViewer<R>>(
        &mut self,
        vwr: &mut V,
        contents: &str,
    ) -> bool {
        /*
            NOTE: System clipboard implementation

            We can't just determine if the internal clipboard should be preferred over the system
            clipboard, as the source of clipboard contents can be vary. Therefore, on every copy
            operation, we should clear out the system clipboard unconditionally, then we tries to
            encode the copied content into clipboard.

            TODO: Need to find way to handle if both of system clipboard and internal clipboard
            content exist. We NEED to determine if which content should be applied for this.

            # Dumping

            - For rectangular(including single cell) selection of data, we'll just create
              appropriate sized small TSV data which suits within given range.
                - Note that this'll differentiate the clipboard behavior from internal-only
                  version.
            - For non-rectangular selections, full-scale rectangular table is dumped which
              can cover all selection range including empty selections; where any data that
              is not being dumped is just emptied out.
                - From this, any data cell that is just 'empty' but selected, should be dumped
                  as explicit empty data; in this case, empty data wrapped with double
                  quotes("").

            # Decoding

            - Every format is regarded as TSV. (only \t, \n matters)
            - For TSV data with same column count with this table
                - Parse as full-scale table, then put into clipboard as-is.
            - Column count is less than current table
                - In this case, current selection matters.
                - Offset the copied content table as the selection column offset.
                - Then create clipboard data from it.
            - If column count is larger than this, it is invalid data; we just skip parsing
        */

        let Some(mut codec) = vwr.try_create_codec(false) else {
            // Even when there is system clipboard content, we're going to ignore it and use
            // internal clipboard if there's no way to parse it.
            return false;
        };

        if let CursorState::Select(selections) = &self.cc_cursor {
            let Some(first) = selections.first().map(|x| x.0) else {
                // No selectgion present. Do nothing
                return false;
            };

            let (.., col) = first.row_col(self.p.vis_cols.len());
            col
        } else {
            // If there's no selection, we'll just ignore the system clipboard input
            return false;
        };

        let selection_offset = if let CursorState::Select(sel) = &self.cc_cursor {
            sel.first().map_or(0, |idx| {
                let (_, c) = idx.0.row_col(self.p.vis_cols.len());
                c.0
            })
        } else {
            0
        };

        let view = tsv::ParsedTsv::parse(contents);
        let table_width = view.calc_table_width();

        if table_width > self.p.vis_cols.len() {
            // If the copied data has more columns than current table, we'll just ignore it.
            return false;
        }

        // If any cell is failed to be parsed, we'll just give up all parsing then use internal
        // clipboard instead.

        let mut slab = Vec::new();
        let mut pastes = Vec::new();

        for (row_offset, row_data) in view.iter_rows() {
            let slab_id = slab.len();
            slab.push(codec.create_empty_decoded_row());

            // The restoration point of pastes stack.
            let pastes_restore = pastes.len();

            for (column, data) in row_data {
                let col_idx = column + selection_offset;

                if col_idx > self.p.vis_cols.len() {
                    // If the column is out of range, we'll just ignore it.
                    return false;
                }

                match codec.decode_column(data, col_idx, &mut slab[slab_id]) {
                    Ok(_) => {
                        pastes.push((
                            VisRowOffset(row_offset),
                            ColumnIdx(col_idx),
                            RowSlabIndex(slab_id),
                        ));
                    }
                    Err(DecodeErrorBehavior::SkipCell) => {
                        // Skip this cell.
                    }
                    Err(DecodeErrorBehavior::SkipRow) => {
                        pastes.drain(pastes_restore..);
                        slab.pop();
                        break;
                    }
                    Err(DecodeErrorBehavior::Abort) => {
                        return false;
                    }
                }
            }
        }

        // Replace the clipboard content from the parsed data.
        self.clipboard = Some(Clipboard {
            slab: slab.into_boxed_slice(),
            pastes: pastes.into_boxed_slice(),
        });

        true
    }

    fn try_dump_clipboard_content<V: RowViewer<R>>(
        clipboard: &Clipboard<R>,
        vwr: &mut V,
    ) -> Option<String> {
        // clipboard MUST be sorted before dumping; XXX: add assertion?
        #[allow(unused_mut)]
        let mut codec = vwr.try_create_codec(true)?;

        let mut width = 0;
        let mut height = 0;

        // We're going to offset the column to the minimum column index to make the selection copy
        // more intuitive. If not, the copied data will be shifted to the right if the selection is
        // not the very first column.
        let mut min_column = usize::MAX;

        for (row, column, ..) in clipboard.pastes.iter() {
            width = width.max(column.0 + 1);
            height = height.max(row.0 + 1);
            min_column = min_column.min(column.0);
        }

        let column_offset = min_column;
        let mut buf_out = String::new();
        let mut buf_tmp = String::new();
        let mut row_cursor = 0;

        for (row, columns, ..) in &clipboard.pastes.iter().chunk_by(|(row, ..)| *row) {
            while row_cursor < row.0 {
                tsv::write_newline(&mut buf_out);
                row_cursor += 1;
            }

            let mut column_cursor = 0;

            for (_, column, data_idx) in columns.into_iter() {
                while column_cursor < column.0 - column_offset {
                    tsv::write_tab(&mut buf_out);
                    column_cursor += 1;
                }

                let data = &clipboard.slab[data_idx.0];
                codec.encode_column(data, column.0, &mut buf_tmp);

                tsv::write_content(&mut buf_out, &buf_tmp);
                buf_tmp.clear();
            }
        }

        Some(buf_out)
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
        let ncol = self.p.vis_cols.len();

        for (row_id, columns) in next_sel {
            let vis_row = self.cc_row_id_to_vis[&row_id];

            if columns.is_empty() {
                let p_left = vis_row.linear_index(ncol, VisColumnPos(0));
                let p_right = vis_row.linear_index(ncol, VisColumnPos(ncol - 1));

                sel.push(VisSelection(p_left, p_right));
            } else {
                for col in columns {
                    let Some(vis_c) = self.p.vis_cols.iter().position(|x| *x == col) else {
                        continue;
                    };

                    let p = vis_row.linear_index(ncol, VisColumnPos(vis_c));
                    sel.push(VisSelection(p, p));
                }
            }
        }

        true
    }

    pub fn vis_cols(&self) -> &Vec<ColumnIdx> {
        &self.p.vis_cols
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
        self.p.num_columns
    }

    pub fn sort(&self) -> &[(ColumnIdx, IsAscending)] {
        &self.p.sort
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
                VisSelection::from_points(self.p.vis_cols.len(), pivot, current),
                row,
                col,
            )
        })
    }

    pub fn is_interactive_row(&self, row: VisRowPos) -> Option<VisColumnPos> {
        let (r, c) = self.cc_interactive_cell.row_col(self.p.vis_cols.len());
        (r == row).then_some(c)
    }

    pub fn interactive_cell(&self) -> (VisRowPos, VisColumnPos) {
        self.cc_interactive_cell.row_col(self.p.vis_cols.len())
    }

    pub fn cci_sel_update(&mut self, current: VisLinearIdx) {
        if let Some((_, pivot)) = &mut self.cci_selection {
            *pivot = current;
        } else {
            self.cci_selection = Some((current, current));
        }
    }

    pub fn cci_sel_update_row(&mut self, row: VisRowPos) {
        [0, self.p.vis_cols.len() - 1].map(|col| {
            self.cci_sel_update(row.linear_index(self.p.vis_cols.len(), VisColumnPos(col)))
        });
    }

    pub fn has_cci_selection(&self) -> bool {
        self.cci_selection.is_some()
    }

    pub fn vis_sel_contains(&self, sel: VisSelection, row: VisRowPos, col: VisColumnPos) -> bool {
        sel.contains(self.p.vis_cols.len(), row, col)
    }

    fn get_highlight_changes<'a>(
        &'a self,
        table: &'a DataTable<R>,
        sel: &[VisSelection],
    ) -> (Vec<&'a R>, Vec<&'a R>) {
        let mut ohs: BTreeSet<&VisSelection> = BTreeSet::default();
        let nhs: BTreeSet<&VisSelection> = sel.iter().collect();

        if let CursorState::Select(s) = &self.cc_cursor {
            ohs = s.iter().collect();
        }

        // IMPORTANT the new highlight may include a selection that includes the old highlight selection
        //           this happens when making a second multi-select using shift
        //           e.g.   old: 1..=5, 10..=10, new: 1..=5, 10..=15

        /// Flatten a set of ranges into a set of linear indices, e.g. [(1,3), (6,8)] -> [1,2,3,6,7,8]
        fn flatten_ranges(ranges: &[(usize, usize)]) -> HashSet<usize> {
            ranges.iter()
                .flat_map(|&(start, end)| start..=end)
                .collect()
        }

        /// Only keep elements in old_rows that are NOT in new_rows
        fn deselected_rows(old_rows: &HashSet<usize>, new_rows: &HashSet<usize>) -> Vec<usize> {
            let missing: Vec<usize> = old_rows
                .difference(&new_rows)
                .copied()
                .collect();

            missing
        }
        
        let nhs_range = self.make_row_range(&nhs);
        let ohs_range = self.make_row_range(&ohs);
        
        let nhs_rows = flatten_ranges(&nhs_range);
        let ohs_rows = flatten_ranges(&ohs_range);

        let deselected_rows = deselected_rows(&ohs_rows, &nhs_rows);
        
        let highlighted: Vec<&R> = nhs_rows
            .into_iter()
            .sorted()
            .map(|r| {
                let row_id = self.cc_rows[r];
                &table.rows[row_id.0]
            })
            .collect();
        let unhighlighted: Vec<&R> = deselected_rows
            .into_iter()
            .sorted()
            .map(|r| {
                let row_id = self.cc_rows[r];
                &table.rows[row_id.0]
            })
            .collect();

        (highlighted, unhighlighted)
    }

    fn make_row_range<'a>(&'a self, nhs: &BTreeSet<&VisSelection>) -> Vec<(usize, usize)> {
        nhs.iter().map(|sel| {
            let (start_ic_r, _ic_c) = sel.0.row_col(self.p.vis_cols.len());
            let (end_ic_r, _ic_c) = sel.1.row_col(self.p.vis_cols.len());
            (start_ic_r.0, end_ic_r.0)
        }).collect::<Vec<(usize, usize)>>()
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
                if self.p.vis_cols.len() == 1 {
                    return;
                }

                let mut vis_cols = self.p.vis_cols.clone();
                let idx = vis_cols.iter().position(|x| *x == column_idx).unwrap();
                vis_cols.remove(idx);

                self.push_new_command(table, vwr, Command::SetVisibleColumns(vis_cols), capacity);
                return;
            }
            Command::CcShowColumn { what, at } => {
                assert!(self.p.vis_cols.iter().all(|x| *x != what));

                let mut vis_cols = self.p.vis_cols.clone();
                vis_cols.insert(at.0, what);

                self.push_new_command(table, vwr, Command::SetVisibleColumns(vis_cols), capacity);
                return;
            }
            Command::SetVisibleColumns(ref value) => {
                if self.p.vis_cols.iter().eq(value.iter()) {
                    return;
                }

                vec![Command::SetVisibleColumns(self.p.vis_cols.clone())]
            }
            Command::CcReorderColumn { from, to } => {
                if from == to || to.0 > self.p.vis_cols.len() {
                    // Reorder may deliver invalid parameter if there's multiple data
                    // tables present at the same time; as the drag drop payload are
                    // compatible between different tables...
                    return;
                }

                let mut vis_cols = self.p.vis_cols.clone();
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
                    self.cc_row_id_to_vis[&row_id].linear_index(self.p.vis_cols.len(), column_pos);

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

            Command::CcSetCells {
                context,
                slab,
                values,
            } => {
                let mut values = values.to_vec();

                values.retain(|(row, col, slab_id)| {
                    
                    if vwr.is_editable_cell(col.0, row.0, &table.rows[row.0]) {
                        vwr.confirm_cell_write_by_ui(
                            &table.rows[row.0],
                            &slab[slab_id.0],
                            col.0,
                            context,
                        )
                    } else {
                        false
                    }
                    
                });

                return self.push_new_command(
                    table,
                    vwr,
                    Command::SetCells {
                        slab,
                        values: values.into_boxed_slice(),
                    },
                    capacity,
                );
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
                if self.p.sort.iter().eq(sort.iter()) {
                    return;
                }

                vec![Command::SetColumnSort(self.p.sort.clone())]
            }
            Command::CcSetSelection(sel) => {
                if !sel.is_empty() {
                    self.cc_interactive_cell = sel[0].0;
                    
                    let (ic_r, ic_c) = self.cc_interactive_cell.row_col(self.p.vis_cols.len());
                    let row_id = self.cc_rows[ic_r.0];

                    let idx = self.vis_cols()[ic_c.0];

                    let row = &table.rows[row_id.0];
                        
                    vwr.on_highlight_cell(row, idx.0);
                }

                let (highlighted, unhighlighted) = self.get_highlight_changes(table, &sel);
                vwr.on_highlight_change(&highlighted, &unhighlighted);
                self.cc_cursor = CursorState::Select(sel);
                return;
            }
            Command::InsertRows(pivot, ref values) => {
                let values = (pivot.0..pivot.0 + values.len()).map(RowIdx).collect();
                vec![Command::RemoveRow(values)]
            }
            Command::RemoveRow(ref indices) => {
                if indices.is_empty() {
                    // From various sources, it can be just 'empty' removal command
                    return;
                }

                // Ensure indices are sorted.
                debug_assert!(indices.windows(2).all(|x| x[0] < x[1]));

                // Collect contiguous chunks.
                let mut chunks = vec![vec![indices[0]]];

                for index in indices.windows(2) {
                    if index[0].0 + 1 == index[1].0 {
                        chunks.last_mut().unwrap().push(index[1]);
                    } else {
                        chunks.push(vec![index[1]]);
                    }
                }

                chunks
                    .into_iter()
                    .map(|x| {
                        Command::InsertRows(
                            x[0],
                            x.into_iter()
                                .map(|x| vwr.clone_row(&table.rows[x.0]))
                                .collect(),
                        )
                    })
                    .collect()
            }
            Command::CcUpdateSystemClipboard(..) => {
                // This command MUST've be consumed before calling this.
                unreachable!()
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
                self.p.vis_cols.clear();
                self.p.vis_cols.extend(cols.iter().cloned());
                self.cc_dirty = true;
            }
            Command::SetColumnSort(new_sort) => {
                self.p.sort.clear();
                self.p.sort.extend(new_sort.iter().cloned());
                self.cc_dirty = true;
            }
            Command::SetRowValue(row_id, value) => {
                self.cc_num_frame_from_last_edit = 0;
                table.dirty_flag = true;
                let old_row = vwr.clone_row(&table.rows[row_id.0]);
                table.rows[row_id.0] = vwr.clone_row(value); 

                vwr.on_row_updated(row_id.0, &table.rows[row_id.0], &old_row);
            }
            Command::SetCells { slab, values } => {
                self.cc_num_frame_from_last_edit = 0;
                table.dirty_flag = true;

                let mut modified_rows: HashMap<RowIdx, R> = HashMap::new();
                
                for (row, col, value_id) in values.iter() {
                    let _ = modified_rows.entry(row.clone()).or_insert_with(|| vwr.clone_row(&table.rows[row.0]));
                    
                    vwr.set_cell_value(&slab[value_id.0], &mut table.rows[row.0], col.0);
                }

                for (row, old_row) in modified_rows.iter() {
                    vwr.on_row_updated(row.0, &mut table.rows[row.0], old_row);
                }
            }
            Command::InsertRows(pos, values) => {
                self.cc_dirty = true; // It invalidates all current `RowId` occurrences.
                table.dirty_flag = true;

                table
                    .rows
                    .splice(pos.0..pos.0, values.iter().map(|x| vwr.clone_row(x)));
                let range = pos.0..pos.0 + values.len();
                
                for row_index in range.clone() {
                    vwr.on_row_inserted(row_index, &mut table.rows[row_index]);
                }
                self.queue_select_rows(range.map(RowIdx));
            }
            Command::RemoveRow(values) => {
                debug_assert!(values.windows(2).all(|x| x[0] < x[1]));
                self.cc_dirty = true; // It invalidates all current `RowId` occurrences.
                table.dirty_flag = true;

                for row_index in values.iter() {
                    vwr.on_row_removed(row_index.0, &mut table.rows[row_index.0]);
                }
                
                let mut index = 0;
                table.rows.retain(|_| {
                    let idx_now = index.tap(|_| index += 1);
                    values.binary_search(&RowIdx(idx_now)).is_err()
                });

                self.queue_select_rows([]);
            }
            Command::CcHideColumn(..)
            | Command::CcShowColumn { .. }
            | Command::CcReorderColumn { .. }
            | Command::CcEditStart(..)
            | Command::CcCommitEdit
            | Command::CcCancelEdit
            | Command::CcSetSelection(..)
            | Command::CcSetCells { .. }
            | Command::CcUpdateSystemClipboard(..) => unreachable!(),
        }
    }

    fn queue_select_rows(&mut self, rows: impl IntoIterator<Item = RowIdx>) {
        self.cc_desired_selection = Some(rows.into_iter().map(|r| (r, default())).collect());
    }

    fn validate_interactive_cell(&mut self, new_num_column: usize) {
        let (r, c) = self.cc_interactive_cell.row_col(self.p.vis_cols.len());
        let rmax = self.cc_rows.len().saturating_sub(1);
        let clen = self.p.vis_cols.len();

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
        self.cc_interactive_cell = row.linear_index(self.p.vis_cols.len(), col);
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

        let (ic_r, ic_c) = self.cc_interactive_cell.row_col(self.p.vis_cols.len());
        match action {
            UiAction::SelectionStartEditing => {
                let row_id = self.cc_rows[ic_r.0];
                let src_row = &table.rows[row_id.0];
                if vwr.is_editable_cell(ic_c.0, ic_r.0, &src_row) {
                    let row = vwr.clone_row(src_row);
                    vec![Command::CcEditStart(row_id, ic_c, Box::new(row))]
                } else {
                    vec![]
                }
            }
            UiAction::CancelEdition => vec![Command::CcCancelEdit],
            UiAction::CommitEdition => vec![Command::CcCommitEdit],
            UiAction::CommitEditionAndMove(dir) => {
                let pos = self.moved_position(self.cc_interactive_cell, dir);
                let (r, c) = pos.row_col(self.p.vis_cols.len());

                let mut commands = vec![
                    Command::CcCommitEdit,
                ];
                
                let row_id = self.cc_rows[r.0];
                if vwr.is_editable_cell(c.0, r.0, &table.rows[row_id.0]) {
                    let row_value = if self.is_editing() && ic_r == r {
                        vwr.clone_row(self.unwrap_editing_row_data())
                    } else {
                        vwr.clone_row(&table.rows[row_id.0])
                    };
                    
                    commands.push(Command::CcEditStart(row_id, c, row_value.into()));
                }
                
                commands
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
                    slab.push(vwr.clone_row_as_copied_base(&table.rows[self.cc_rows[vis_row.0].0]));
                }

                let clipboard = Clipboard {
                    slab: slab.into_boxed_slice(),
                    pastes: sels
                        .iter()
                        .map(|(v_r, v_c)| {
                            (
                                VisRowOffset(v_r.0 - offset.0),
                                self.p.vis_cols[v_c.0],
                                RowSlabIndex(vis_map[v_r]),
                            )
                        })
                        .collect(),
                }
                .tap_mut(Clipboard::sort);

                let sys_clip = Self::try_dump_clipboard_content(&clipboard, vwr);
                self.clipboard = Some(clipboard);

                if action == UiAction::CutSelection {
                    self.try_apply_ui_action(table, vwr, UiAction::DeleteSelection)
                } else {
                    vec![]
                }
                .tap_mut(|v| {
                    // We only overwrite system clipboard when codec support is active.
                    if let Some(clip) = sys_clip {
                        v.push(Command::CcUpdateSystemClipboard(clip));
                    }
                })
            }
            UiAction::SelectionDuplicateValues => {
                let pivot_row = vwr.clone_row_as_copied_base(&table.rows[self.cc_rows[ic_r.0].0]);
                let sels = self.collect_selection();

                vec![Command::CcSetCells {
                    slab: [pivot_row].into(),
                    values: sels
                        .into_iter()
                        .map(|(r, c)| (self.cc_rows[r.0], self.p.vis_cols[c.0], RowSlabIndex(0)))
                        .collect(),
                    context: CellWriteContext::Paste,
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

                let desired = self.cc_desired_selection.get_or_insert(default());
                desired.clear();

                for (row, group) in &values.iter().chunk_by(|(row, ..)| *row) {
                    desired.push((row, group.map(|(_, c, ..)| *c).collect()))
                }

                vec![Command::CcSetCells {
                    slab: clip.slab.iter().map(|x| vwr.clone_row(x)).collect(),
                    values: values.into_boxed_slice(),
                    context: CellWriteContext::Paste,
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
                    .map(|(offset, ..)| {
                        (
                            *offset,
                            vwr.new_empty_row_for(EmptyRowCreateContext::InsertNewLine),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();

                for (offset, column, slab_id) in &*clip.pastes {
                    vwr.set_cell_value(
                        &clip.slab[slab_id.0],
                        rows.get_mut(offset).unwrap(),
                        column.0,
                    );
                }

                let pos = if self.p.sort.is_empty() {
                    self.cc_rows[ic_r.0]
                } else {
                    RowIdx(table.rows.len())
                };

                let row_values = rows.into_values().collect();
                vec![Command::InsertRows(pos, row_values)]
            }
            UiAction::DuplicateRow => {
                if vwr.allow_row_insertions() {
                    let rows = self
                        .collect_selected_rows()
                        .into_iter()
                        .map(|x| self.cc_rows[x.0])
                        .map(|r| vwr.clone_row_for_insertion(&table.rows[r.0]))
                        .collect();

                    let pos = if self.p.sort.is_empty() {
                        self.cc_rows[ic_r.0]
                    } else {
                        RowIdx(table.rows.len())
                    };

                    vec![Command::InsertRows(pos, rows)]
                } else {
                    vec![]
                }
            }
            UiAction::DeleteSelection => {
                let default = vwr.new_empty_row_for(EmptyRowCreateContext::DeletionDefault);
                let sels = self.collect_selection();
                let slab = vec![default].into_boxed_slice();

                vec![Command::CcSetCells {
                    slab,
                    values: sels
                        .into_iter()
                        .map(|(r, c)| (self.cc_rows[r.0], self.p.vis_cols[c.0], RowSlabIndex(0)))
                        .collect(),
                    context: CellWriteContext::Clear,
                }]
            }
            UiAction::DeleteRow => {
                if vwr.allow_row_deletions() {
                    let rows = self
                        .collect_selected_rows()
                        .into_iter()
                        .map(|x| self.cc_rows[x.0])
                        .filter(|row| vwr.confirm_row_deletion_by_ui(&table.rows[row.0]))
                        .collect();

                    vec![Command::RemoveRow(rows)]
                } else {
                    vec![]
                }
            }
            UiAction::SelectAll => {
                if self.cc_rows.is_empty() {
                    return vec![];
                }

                vec![Command::CcSetSelection(vec![VisSelection(
                    VisLinearIdx(0),
                    VisRowPos(self.cc_rows.len().saturating_sub(1)).linear_index(
                        self.p.vis_cols.len(),
                        VisColumnPos(self.p.vis_cols.len() - 1),
                    ),
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
                    VisLinearIdx(new_ic_r as usize * self.p.vis_cols.len() + ic_c.0);

                self.validate_interactive_cell(self.p.vis_cols.len());
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
                let (top, left) = sel.0.row_col(self.p.vis_cols.len());
                let (bottom, right) = sel.1.row_col(self.p.vis_cols.len());

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
                let (top, _) = sel.0.row_col(self.p.vis_cols.len());
                let (bottom, _) = sel.1.row_col(self.p.vis_cols.len());

                for r in top.0..=bottom.0 {
                    rows.insert(VisRowPos(r));
                }
            }
        }

        rows
    }

    fn moved_position(&self, pos: VisLinearIdx, dir: MoveDirection) -> VisLinearIdx {
        let (VisRowPos(r), VisColumnPos(c)) = pos.row_col(self.p.vis_cols.len());

        let (rmax, cmax) = (
            self.cc_rows.len().saturating_sub(1),
            self.p.vis_cols.len().saturating_sub(1),
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

        VisLinearIdx(nr * self.p.vis_cols.len() + nc)
    }

    pub fn cci_take_selection(&mut self, mods: egui::Modifiers) -> Option<Vec<VisSelection>> {
        let ncol = self.p.vis_cols.len();
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

/// NOTE: `Cc` prefix stands for cache command which won't be stored in undo/redo queue, since they
/// are not called from `cmd_apply` method.
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
    CcSetCells {
        slab: Box<[R]>,
        values: Box<[(RowIdx, ColumnIdx, RowSlabIndex)]>,
        context: CellWriteContext,
    },
    SetCells {
        slab: Box<[R]>,
        values: Box<[(RowIdx, ColumnIdx, RowSlabIndex)]>,
    },

    InsertRows(RowIdx, Box<[R]>),
    RemoveRow(Vec<RowIdx>),

    CcEditStart(RowIdx, VisColumnPos, Box<R>),
    CcCancelEdit,
    CcCommitEdit,

    CcUpdateSystemClipboard(String),
}
