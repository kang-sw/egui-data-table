pub use egui_extras::Column as TableColumnConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiAction {
    ActivateSelectedCell,
    CancelEdit,
    CommitEdit,
    Undo,
    Redo,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellEditState {
    #[default]
    Disabled,
    JustEnabled,
    Enabled,
}

pub trait RowViewer<R: Send> {
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

    fn filter_row(&mut self, row: &R) -> bool {
        true
    }

    fn detect_hotkey(&mut self, ui: &egui::InputState) -> Option<UiAction> {
        self::detect_hotkey_excel(ui)
    }
}

pub fn detect_hotkey_excel(input: &egui::InputState) -> Option<UiAction> {
    // TODO
    None
}
