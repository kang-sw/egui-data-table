use std::borrow::Cow;

use egui::{Key, KeyboardShortcut, Modifiers};
pub use egui_extras::Column as TableColumnConfig;
use tap::prelude::Pipe;

/// The primary trait for the spreadsheet viewer.
// TODO: When lifetime for `'static` is stabilized; remove the `static` bound.
pub trait RowViewer<R>: 'static {
    /// Number of columns. Changing this will invalidate the table rendering status
    /// totally(including undo histories), therefore frequently changing this value is
    /// discouraged.
    fn num_columns(&mut self) -> usize;

    /// Name of the column. This can be dynamically changed.
    fn column_name(&mut self, column: usize) -> Cow<'static, str> {
        Cow::Borrowed(
            &" 0 1 2 3 4 5 6 7 8 91011121314151617181920212223242526272829303132"
                [(column % 10 * 2).pipe(|x| x..x + 2)],
        )
    }

    /// Returns the rendering configuration for the column.
    fn column_render_config(&mut self, column: usize) -> TableColumnConfig {
        let _ = column;
        TableColumnConfig::auto().resizable(true)
    }

    /// Returns if given column is 'sortable'
    fn is_sortable_column(&mut self, column: usize) -> bool {
        let _ = column;
        false
    }

    /// Compare two column contents for sort.
    fn create_cell_comparator(&mut self) -> impl Fn(&R, &R, usize) -> std::cmp::Ordering {
        |_, _, _| std::cmp::Ordering::Equal
    }

    /// Get hash value of a filter. This is used to determine if the filter has changed.
    fn row_filter_hash(&mut self) -> &impl std::hash::Hash {
        &()
    }

    /// Create a filter for the row. Filter is applied on every table invalidation.
    fn create_row_filter(&mut self) -> impl Fn(&R) -> bool {
        |_| true
    }

    /// Display values of the cell. Any input will be consumed before table renderer;
    /// therefore any widget rendered inside here is read-only.
    ///
    /// To deal with input, use `cell_edit` method. If you need to deal with drag/drop,
    /// see [`RowViewer::on_cell_view_response`] which delivers resulting response of
    /// containing cell.
    fn show_cell_view(&mut self, ui: &mut egui::Ui, row: &R, column: usize);

    /// Use this to check if given cell is going to take any dropped payload / use as drag
    /// source.
    fn on_cell_view_response(
        &mut self,
        row: &R,
        column: usize,
        resp: &egui::Response,
    ) -> Option<Box<R>> {
        let _ = (row, column, resp);
        None
    }

    /// Edit values of the cell.
    fn show_cell_editor(
        &mut self,
        ui: &mut egui::Ui,
        row: &mut R,
        column: usize,
    ) -> Option<egui::Response>;

    /// Set the value of a column in a row.
    fn set_cell_value(&mut self, src: &R, dst: &mut R, column: usize);

    /// In the write context that happens outside of `show_cell_editor`, this method is
    /// called on every cell value editions.
    fn confirm_cell_write(
        &mut self,
        current: &R,
        next: &R,
        column: usize,
        context: CellWriteContext,
    ) -> bool {
        let _ = (current, next, column, context);
        true
    }

    /// Before removing each row, this method is called to confirm the deletion from the
    /// viewer. This won't be called during the undo/redo operation!
    fn confirm_row_deletion(&mut self, _row: &R) -> bool {
        true
    }

    /// Create a new empty row.
    fn new_empty_row(&mut self) -> R;

    /// Create a new empty row under the given context.
    fn new_empty_row_for(&mut self, context: EmptyRowCreateContext) -> R {
        let _ = context;
        self.new_empty_row()
    }

    /// Create duplication of existing row.
    ///
    /// You may want to override this method for more efficient duplication.
    fn clone_row(&mut self, row: &R) -> R {
        let mut dst = self.new_empty_row();
        for i in 0..self.num_columns() {
            self.set_cell_value(row, &mut dst, i);
        }
        dst
    }

    /// Create duplication of existing row for insertion.
    fn clone_row_for_insertion(&mut self, row: &R) -> R {
        self.clone_row(row)
    }

    /// Called when a cell is selected/highlighted.
    fn on_highlight_cell(&mut self, row: &R, column: usize) {
        let _ = (row, column);
    }

    /// Return hotkeys for the current context.
    fn hotkeys(&mut self, context: &UiActionContext) -> Vec<(egui::KeyboardShortcut, UiAction)> {
        self::default_hotkeys(context)
    }

    /// Get trivial configurations for renderer.
    fn trivial_config(&mut self) -> TrivialConfig {
        Default::default()
    }
}

/* ------------------------------------------- Context ------------------------------------------ */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CellWriteContext {
    /// Value is being pasted/duplicated from different row.
    Paste,

    /// Value is being cleared by cut/delete operation.
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EmptyRowCreateContext {
    /// Row is created to be used as simple default template.
    Default,

    /// Row is created to be used explicit `empty` value when deletion
    DeletionDefault,

    /// Row is created to be inserted as a new row.
    InsertNewLine,
}

/* ------------------------------------------- Hotkeys ------------------------------------------ */

/// Base context for determining current input state.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct UiActionContext {
    pub cursor: UiCursorState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiCursorState {
    Idle,
    Editing,
    SelectOne,
    SelectMany,
}

impl UiCursorState {
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    pub fn is_editing(&self) -> bool {
        matches!(self, Self::Editing)
    }

    pub fn is_selecting(&self) -> bool {
        matches!(self, Self::SelectOne | Self::SelectMany)
    }
}

/* ----------------------------------------- Ui Actions ----------------------------------------- */

/// Represents a user interaction, calculated from the UI input state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiAction {
    SelectionStartEditing,

    CancelEdition,
    CommitEdition,

    CommitEditionAndMove(MoveDirection),

    Undo,
    Redo,

    MoveSelection(MoveDirection),
    CopySelection,
    CutSelection,
    PasteInPlace,
    PasteInsert,

    DuplicateRow,
    DeleteSelection,
    DeleteRow,

    NavPageDown,
    NavPageUp,
    NavTop,
    NavBottom,

    SelectionDuplicateValues,
    SelectAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoveDirection {
    Up,
    Down,
    Left,
    Right,
}

pub fn default_hotkeys(context: &UiActionContext) -> Vec<(KeyboardShortcut, UiAction)> {
    let c = context.cursor;

    fn shortcut(actions: &[(Modifiers, Key, UiAction)]) -> Vec<(egui::KeyboardShortcut, UiAction)> {
        actions
            .iter()
            .map(|(m, k, a)| (egui::KeyboardShortcut::new(*m, *k), *a))
            .collect()
    }

    let none = Modifiers::NONE;
    let ctrl = Modifiers::CTRL;
    let alt = Modifiers::ALT;
    let shift = Modifiers::SHIFT;

    use UiAction::CommitEditionAndMove;
    type MD = MoveDirection;

    if c.is_editing() {
        shortcut(&[
            (none, Key::Escape, UiAction::CommitEdition),
            (ctrl, Key::Escape, UiAction::CancelEdition),
            (shift, Key::Enter, CommitEditionAndMove(MD::Up)),
            (ctrl, Key::Enter, CommitEditionAndMove(MD::Down)),
            (shift, Key::Tab, CommitEditionAndMove(MD::Left)),
            (none, Key::Tab, CommitEditionAndMove(MD::Right)),
        ])
    } else {
        shortcut(&[
            (ctrl, Key::X, UiAction::CutSelection),
            (ctrl, Key::C, UiAction::CopySelection),
            (ctrl | shift, Key::V, UiAction::PasteInsert),
            (ctrl, Key::V, UiAction::PasteInPlace),
            (ctrl, Key::Y, UiAction::Redo),
            (ctrl, Key::Z, UiAction::Undo),
            (none, Key::Enter, UiAction::SelectionStartEditing),
            (none, Key::ArrowUp, UiAction::MoveSelection(MD::Up)),
            (none, Key::ArrowDown, UiAction::MoveSelection(MD::Down)),
            (none, Key::ArrowLeft, UiAction::MoveSelection(MD::Left)),
            (none, Key::ArrowRight, UiAction::MoveSelection(MD::Right)),
            (shift, Key::V, UiAction::PasteInsert),
            (alt, Key::V, UiAction::PasteInsert),
            (ctrl | shift, Key::D, UiAction::DuplicateRow),
            (ctrl, Key::D, UiAction::SelectionDuplicateValues),
            (ctrl, Key::A, UiAction::SelectAll),
            (ctrl, Key::Delete, UiAction::DeleteRow),
            (none, Key::Delete, UiAction::DeleteSelection),
            (none, Key::Backspace, UiAction::DeleteSelection),
            (none, Key::PageUp, UiAction::NavPageUp),
            (none, Key::PageDown, UiAction::NavPageDown),
            (none, Key::Home, UiAction::NavTop),
            (none, Key::End, UiAction::NavBottom),
        ])
    }
}

/* ---------------------------------------- Configuration --------------------------------------- */

#[derive(Clone, Debug)]
pub struct TrivialConfig {
    /// If specify this as [`None`], the heterogeneous row height will be used.
    pub table_row_height: Option<f32>,

    /// Maximum number of undo history. This is applied when actual action is performed.
    pub max_undo_history: usize,
}

impl Default for TrivialConfig {
    fn default() -> Self {
        Self {
            table_row_height: None,
            max_undo_history: 100,
        }
    }
}
