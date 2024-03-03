pub use egui_extras::Column as TableColumnConfig;

/// Represents a user interaction, calculated from the UI input state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiAction {
    ActivateSelectedCell,
    CancelEdit,
    CommitEdit,
    Undo,
    Redo,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellUiState {
    #[default]
    View,
    EditStarted,
    Editing,
}

impl CellUiState {
    pub fn is_editing(self) -> bool {
        matches!(self, Self::EditStarted | Self::Editing)
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TrivialConfig {
    pub max_undo_history: usize,
}

impl Default for TrivialConfig {
    fn default() -> Self {
        Self {
            max_undo_history: 100,
        }
    }
}

pub trait RowViewer<R: Send>: std::hash::Hash {
    fn num_columns(&mut self) -> usize;

    fn column_name(&mut self, column: usize) -> &str;

    fn column_config(&mut self, column: usize) -> TableColumnConfig {
        let _ = column;
        TableColumnConfig::auto().resizable(true)
    }

    fn is_sortable_column(&mut self, column: usize) -> bool;

    fn compare_column(&mut self, row_l: &R, row_r: &R, column: usize) -> std::cmp::Ordering;

    /// Should return true if the column is modified. Otherwise, it won't be updated.
    ///
    /// When it's activated, the `active` flag is set to true. You can utilize this to
    /// expand editor, show popup, etc.
    fn draw_cell(&mut self, ui: &mut egui::Ui, row: &mut R, column: usize, state: CellUiState);

    fn empty_row(&mut self) -> R;

    fn clone_column(&mut self, src: &R, dst: &mut R, column: usize);

    /// Tries to clone between different columns.
    fn clone_column_arbitrary(
        &mut self,
        src: &R,
        src_column: usize,
        dst: &mut R,
        dst_column: usize,
    ) {
        debug_assert!(src_column != dst_column);
        let _ = (src, dst, src_column, dst_column);
    }

    fn clone_column_smart(&mut self, src: &R, dst: &mut R, column: usize, offset: usize) {
        let _ = offset;
        self.clone_column(src, dst, column);
    }

    fn clone_row(&mut self, src: &R) -> R;

    fn clear_column(&mut self, row: &mut R, column: usize);

    fn filter_row(&mut self, row: &R) -> bool {
        true
    }

    fn detect_hotkey(&mut self, ui: &egui::InputState) -> Option<UiAction> {
        self::detect_hotkey_excel(ui)
    }

    fn trivial_configs(&mut self) -> TrivialConfig {
        Default::default()
    }
}

pub fn detect_hotkey_excel(input: &egui::InputState) -> Option<UiAction> {
    // TODO
    None
}
