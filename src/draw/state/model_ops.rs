use crate::viewer::{CellWriteContext, EmptyRowCreateContext};
use crate::RowViewer;

/// A subset of `RowViewer<R>` containing only pure data-model methods (no egui dependency).
/// This enables unit testing state logic without requiring an egui context.
pub(crate) trait DataModelOps<R> {
    fn num_columns(&mut self) -> usize;
    fn is_sortable_column(&mut self, column: usize) -> bool;
    fn is_editable_cell(&mut self, column: usize, row: usize, row_value: &R) -> bool;
    fn allow_row_insertions(&mut self) -> bool;
    fn allow_row_deletions(&mut self) -> bool;
    fn compare_cell(&self, row_a: &R, row_b: &R, column: usize) -> std::cmp::Ordering;
    fn row_filter_hash(&mut self) -> u64;
    fn filter_row(&mut self, row: &R) -> bool;
    fn set_cell_value(&mut self, src: &R, dst: &mut R, column: usize);
    fn confirm_cell_write_by_ui(
        &mut self,
        current: &R,
        next: &R,
        column: usize,
        context: CellWriteContext,
    ) -> bool;
    fn confirm_row_deletion_by_ui(&mut self, row: &R) -> bool;
    fn new_empty_row(&mut self) -> R;
    fn new_empty_row_for(&mut self, context: EmptyRowCreateContext) -> R;
    fn clone_row(&mut self, row: &R) -> R;
    fn clone_row_for_insertion(&mut self, row: &R) -> R;
    fn clone_row_as_copied_base(&mut self, row: &R) -> R;
    fn on_highlight_cell(&mut self, row: &R, column: usize);
    fn on_highlight_change(&mut self, highlighted: &[&R], unhighlighted: &[&R]);
    fn on_row_updated(&mut self, row_index: usize, new_row: &R, old_row: &R);
    fn on_row_inserted(&mut self, row_index: usize, row: &R);
    fn on_row_removed(&mut self, row_index: usize, row: &R);
}

impl<R, V: RowViewer<R>> DataModelOps<R> for V {
    fn num_columns(&mut self) -> usize {
        RowViewer::num_columns(self)
    }

    fn is_sortable_column(&mut self, column: usize) -> bool {
        RowViewer::is_sortable_column(self, column)
    }

    fn is_editable_cell(&mut self, column: usize, row: usize, row_value: &R) -> bool {
        RowViewer::is_editable_cell(self, column, row, row_value)
    }

    fn allow_row_insertions(&mut self) -> bool {
        RowViewer::allow_row_insertions(self)
    }

    fn allow_row_deletions(&mut self) -> bool {
        RowViewer::allow_row_deletions(self)
    }

    fn compare_cell(&self, row_a: &R, row_b: &R, column: usize) -> std::cmp::Ordering {
        RowViewer::compare_cell(self, row_a, row_b, column)
    }

    fn row_filter_hash(&mut self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::hash::DefaultHasher::new();
        RowViewer::row_filter_hash(self).hash(&mut h);
        h.finish()
    }

    fn filter_row(&mut self, row: &R) -> bool {
        RowViewer::filter_row(self, row)
    }

    fn set_cell_value(&mut self, src: &R, dst: &mut R, column: usize) {
        RowViewer::set_cell_value(self, src, dst, column)
    }

    fn confirm_cell_write_by_ui(
        &mut self,
        current: &R,
        next: &R,
        column: usize,
        context: CellWriteContext,
    ) -> bool {
        RowViewer::confirm_cell_write_by_ui(self, current, next, column, context)
    }

    fn confirm_row_deletion_by_ui(&mut self, row: &R) -> bool {
        RowViewer::confirm_row_deletion_by_ui(self, row)
    }

    fn new_empty_row(&mut self) -> R {
        RowViewer::new_empty_row(self)
    }

    fn new_empty_row_for(&mut self, context: EmptyRowCreateContext) -> R {
        RowViewer::new_empty_row_for(self, context)
    }

    fn clone_row(&mut self, row: &R) -> R {
        RowViewer::clone_row(self, row)
    }

    fn clone_row_for_insertion(&mut self, row: &R) -> R {
        RowViewer::clone_row_for_insertion(self, row)
    }

    fn clone_row_as_copied_base(&mut self, row: &R) -> R {
        RowViewer::clone_row_as_copied_base(self, row)
    }

    fn on_highlight_cell(&mut self, row: &R, column: usize) {
        RowViewer::on_highlight_cell(self, row, column)
    }

    fn on_highlight_change(&mut self, highlighted: &[&R], unhighlighted: &[&R]) {
        RowViewer::on_highlight_change(self, highlighted, unhighlighted)
    }

    fn on_row_updated(&mut self, row_index: usize, new_row: &R, old_row: &R) {
        RowViewer::on_row_updated(self, row_index, new_row, old_row)
    }

    fn on_row_inserted(&mut self, row_index: usize, row: &R) {
        RowViewer::on_row_inserted(self, row_index, row)
    }

    fn on_row_removed(&mut self, row_index: usize, row: &R) {
        RowViewer::on_row_removed(self, row_index, row)
    }
}
