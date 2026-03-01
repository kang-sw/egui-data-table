use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    mem::{replace, take},
};
use std::collections::{HashMap, HashSet};
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

mod types;
mod command;
mod clipboard;
mod selection;
mod action;
mod validation;
mod model_ops;

pub(crate) use types::*;
pub(crate) use command::Command;
pub(crate) use model_ops::DataModelOps;

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

    pub fn is_interactive_row(&self, row: VisRowPos) -> Option<VisColumnPos> {
        let (r, c) = self.cc_interactive_cell.row_col(self.p.vis_cols.len());
        (r == row).then_some(c)
    }

    pub fn interactive_cell(&self) -> (VisRowPos, VisColumnPos) {
        self.cc_interactive_cell.row_col(self.p.vis_cols.len())
    }

    pub fn has_clipboard_contents(&self) -> bool {
        self.clipboard.is_some()
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

    pub fn set_interactive_cell(&mut self, row: VisRowPos, col: VisColumnPos) {
        self.cc_interactive_cell = row.linear_index(self.p.vis_cols.len(), col);
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
}
