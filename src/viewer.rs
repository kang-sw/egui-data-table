use egui::{Key, KeyboardShortcut, Modifiers};
pub use egui_extras::Column as TableColumnConfig;

/// The primary trait for the spreadsheet viewer.
// TODO: When lifetime for `'static` is stabilized; remove the `static` bound.
pub trait RowViewer<R>: 'static {
    /// Number of columns. Changing this will invalidate the table rendering status
    /// totally(including undo histories), therefore frequently changing this value is
    /// discouraged.
    fn num_columns(&mut self) -> usize;

    /// Name of the column. This can be dynamically changed.
    fn column_name(&mut self, column: usize) -> &str;

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
    fn cell_comparator(&mut self) -> impl Fn(&R, &R, usize) -> std::cmp::Ordering {
        |_, _, _| std::cmp::Ordering::Equal
    }

    /// Display values of the cell. Any input will be consumed before table renderer;
    /// therefore any widget rendered inside here is read-only.
    ///
    /// To deal with input, use `cell_edit` method. If you need to deal with drag/drop,
    /// see [`RowViewer::cell_view_dnd_response`] which delivers resulting response of
    /// containing cell.
    fn cell_view(&mut self, ui: &mut egui::Ui, row: &R, column: usize);

    /// Use this to check if given cell is going to take any dropped payload / use as drag
    /// source.
    fn cell_view_dnd_response(
        &mut self,
        row: &R,
        column: usize,
        resp: &egui::Response,
    ) -> Option<Box<R>> {
        let _ = (row, column, resp);
        None
    }

    /// Edit values of the cell.
    fn cell_editor(
        &mut self,
        ui: &mut egui::Ui,
        row: &mut R,
        column: usize,
        focus_column: Option<usize>,
    ) -> impl Into<EditorAction>;

    /// Set the value of a column in a row.
    fn cell_set_value(&mut self, src: &R, dst: &mut R, column: usize);

    /// Create a new empty row.
    fn row_empty(&mut self) -> R;

    /// Create duplication of existing row.
    fn row_clone(&mut self, row: &R) -> R;

    /// Get hash value of a filter. This is used to determine if the filter has changed.
    fn hash_row_filter(&mut self) -> &impl std::hash::Hash {
        &()
    }

    /// Create a filter for the row. Filter is applied on every table invalidation.
    fn row_filter(&mut self) -> impl Fn(&R) -> bool {
        |_| true
    }

    /// Clear the value of a column in a row.
    fn clear_column(&mut self, row: &mut R, column: usize) {
        let empty_row = self.row_empty();
        self.cell_set_value(&empty_row, row, column);
    }

    /// Method should consume all inputs, until there's no more inputs to consume.
    /// Returning [`Some`] forever may cause the application to hang.
    fn detect_hotkey(
        &mut self,
        input: &mut egui::InputState,
        context: &UiActionContext,
    ) -> Option<UiAction> {
        self::detect_hotkey_excel(input, context)
    }

    /// Get trivial configurations for renderer.
    fn trivial_config(&mut self) -> TrivialConfig {
        Default::default()
    }
}

/* ------------------------------------------- Hotkeys ------------------------------------------ */

/// Base context for determining current input state.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct UiActionContext {
    ///
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

pub fn detect_hotkey_excel(
    i: &mut egui::InputState,
    context: &UiActionContext,
) -> Option<UiAction> {
    let c = context.cursor;

    fn shortcut(
        i: &mut egui::InputState,
        actions: &[(Modifiers, Key, UiAction)],
    ) -> Option<UiAction> {
        for (m, k, a) in actions {
            if i.consume_shortcut(&KeyboardShortcut::new(*m, *k)) {
                return Some(*a);
            }
        }

        None
    }

    let none = Modifiers::NONE;
    let ctrl = Modifiers::CTRL;
    let alt = Modifiers::ALT;
    let shift = Modifiers::SHIFT;

    use UiAction::CommitEditionAndMove;
    type MD = MoveDirection;

    if c.is_editing() {
        shortcut(
            i,
            &[
                (none, Key::Escape, UiAction::CommitEdition),
                (ctrl, Key::Escape, UiAction::CancelEdition),
                (shift, Key::Enter, CommitEditionAndMove(MD::Up)),
                (ctrl, Key::Enter, CommitEditionAndMove(MD::Down)),
                (shift, Key::Tab, CommitEditionAndMove(MD::Left)),
                (none, Key::Tab, CommitEditionAndMove(MD::Right)),
            ],
        )
    } else {
        shortcut(
            i,
            &[
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
            ],
        )
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

/* ------------------------------------------- Action ------------------------------------------- */

#[derive(Default, Clone, Copy)]
pub enum EditorAction {
    #[default]
    Idle,
    Commit,
    Cancel,
}

impl From<Option<bool>> for EditorAction {
    fn from(value: Option<bool>) -> Self {
        match value {
            Some(true) => Self::Commit,
            Some(false) => Self::Cancel,
            None => Self::Idle,
        }
    }
}
