use egui::{Key, KeyboardShortcut, Modifiers};
pub use egui_extras::Column as TableColumnConfig;

/// The primary trait for the spreadsheet viewer.
pub trait RowViewer<R: Send + Clone>: std::hash::Hash + 'static {
    fn num_columns(&self) -> usize;

    fn column_name(&self, column: usize) -> &str;

    fn column_config(&self, column: usize) -> TableColumnConfig {
        let _ = column;
        TableColumnConfig::auto().resizable(true)
    }

    /// Returns if given column is 'sortable'
    fn is_sortable_column(&self, column: usize) -> bool {
        let _ = column;
        false
    }

    /// Compare two column contents for sort.
    fn compare_column_for_sort(&self, row_l: &R, row_r: &R, column: usize) -> std::cmp::Ordering {
        let _ = (row_l, row_r, column);
        std::cmp::Ordering::Equal
    }

    /// Display values of the cell.
    fn cell_view(&mut self, ui: &mut egui::Ui, row: &R, column: usize);

    /// Edit values of the cell.
    fn cell_edit(
        &mut self,
        ui: &mut egui::Ui,
        row: &mut R,
        column: usize,
        focus_column: Option<usize>,
    ) -> impl Into<EditorAction>;

    /// Create a new empty row.
    fn empty_row(&mut self) -> R;

    /// Set the value of a column in a row.
    fn set_column_value(&mut self, src: &R, dst: &mut R, column: usize);

    /// Generative clone of sequential rows. e.g. sequentially increment integer values
    fn clone_column_generative(&mut self, pivot: &R, rows: &mut [&mut R], column: usize) {
        for row in rows {
            self.set_column_value(pivot, row, column);
        }
    }

    /// Clear the value of a column in a row.
    fn clear_column(&mut self, row: &mut R, column: usize) {
        let empty_row = self.empty_row();
        self.set_column_value(&empty_row, row, column);
    }

    /// Filter the row. e.g. String search
    fn filter_row(&self, row: &R) -> bool {
        let _ = row;
        true
    }

    /// Returns if the row has a filter.
    fn has_row_filter(&self) -> bool {
        false
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

    InsertRowBelow,
    InsertRowAbove,
    DuplicateRow,

    SelectionDuplicateValues,
    SelectionGenerateValues,
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
                // TODO: Move with arrow key
            ],
        )
    }
}

/* ---------------------------------------- Configuration --------------------------------------- */

#[derive(Clone, Debug)]
pub struct TrivialConfig {
    pub table_row_height: Option<f32>,
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
