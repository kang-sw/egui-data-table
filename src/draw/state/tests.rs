use super::*;
use crate::viewer::{CellWriteContext, EmptyRowCreateContext};

// ========================= Mock Infrastructure =========================

type TestRow = Vec<i32>;

struct MockViewer {
    columns: usize,
    sortable_columns: Vec<bool>,
    editable: bool,
    #[allow(dead_code)]
    allow_insert: bool,
    #[allow(dead_code)]
    allow_delete: bool,
    filter_fn: Option<Box<dyn Fn(&TestRow) -> bool>>,
    filter_version: u64,
}

impl MockViewer {
    fn new(columns: usize) -> Self {
        Self {
            columns,
            sortable_columns: vec![true; columns],
            editable: true,
            allow_insert: true,
            allow_delete: true,
            filter_fn: None,
            filter_version: 0,
        }
    }
}

impl DataModelOps<TestRow> for MockViewer {
    fn num_columns(&mut self) -> usize {
        self.columns
    }

    fn is_sortable_column(&mut self, column: usize) -> bool {
        self.sortable_columns.get(column).copied().unwrap_or(false)
    }

    fn is_editable_cell(&mut self, _column: usize, _row: usize, _row_value: &TestRow) -> bool {
        self.editable
    }

    fn allow_row_insertions(&mut self) -> bool {
        self.allow_insert
    }

    fn allow_row_deletions(&mut self) -> bool {
        self.allow_delete
    }

    fn compare_cell(&self, row_a: &TestRow, row_b: &TestRow, column: usize) -> std::cmp::Ordering {
        let a = row_a.get(column).copied().unwrap_or(0);
        let b = row_b.get(column).copied().unwrap_or(0);
        a.cmp(&b)
    }

    fn row_filter_hash(&mut self) -> u64 {
        self.filter_version
    }

    fn filter_row(&mut self, row: &TestRow) -> bool {
        self.filter_fn.as_ref().map_or(true, |f| f(row))
    }

    fn set_cell_value(&mut self, src: &TestRow, dst: &mut TestRow, column: usize) {
        if column < src.len() && column < dst.len() {
            dst[column] = src[column];
        }
    }

    fn confirm_cell_write_by_ui(
        &mut self,
        _current: &TestRow,
        _next: &TestRow,
        _column: usize,
        _context: CellWriteContext,
    ) -> bool {
        true
    }

    fn confirm_row_deletion_by_ui(&mut self, _row: &TestRow) -> bool {
        true
    }

    fn new_empty_row(&mut self) -> TestRow {
        vec![0; self.columns]
    }

    fn new_empty_row_for(&mut self, _context: EmptyRowCreateContext) -> TestRow {
        self.new_empty_row()
    }

    fn clone_row(&mut self, row: &TestRow) -> TestRow {
        row.clone()
    }

    fn clone_row_for_insertion(&mut self, row: &TestRow) -> TestRow {
        row.clone()
    }

    fn clone_row_as_copied_base(&mut self, row: &TestRow) -> TestRow {
        row.clone()
    }

    fn on_highlight_cell(&mut self, _row: &TestRow, _column: usize) {}
    fn on_highlight_change(&mut self, _highlighted: &[&TestRow], _unhighlighted: &[&TestRow]) {}
    fn on_row_updated(&mut self, _row_index: usize, _new_row: &TestRow, _old_row: &TestRow) {}
    fn on_row_inserted(&mut self, _row_index: usize, _row: &TestRow) {}
    fn on_row_removed(&mut self, _row_index: usize, _row: &TestRow) {}
}

/// Helper: create a UiState with identity validated and cc validated.
fn setup_state(vwr: &mut MockViewer, table: &mut DataTable<TestRow>) -> UiState<TestRow> {
    let mut state = UiState::default();
    state.validate_identity(vwr);
    state.validate_cc(&mut table.rows, vwr);
    state
}

// ========================= types.rs tests =========================

mod types_tests {
    use super::super::types::*;

    #[test]
    fn vis_linear_idx_row_col_roundtrip() {
        let ncol = 5;
        for r in 0..10 {
            for c in 0..ncol {
                let idx = VisRowPos(r).linear_index(ncol, VisColumnPos(c));
                let (got_r, got_c) = idx.row_col(ncol);
                assert_eq!(got_r, VisRowPos(r));
                assert_eq!(got_c, VisColumnPos(c));
            }
        }
    }

    #[test]
    fn vis_selection_contains() {
        let ncol = 4;
        // Selection from (1,1) to (3,2)
        let sel = VisSelection::from_points(
            ncol,
            VisRowPos(1).linear_index(ncol, VisColumnPos(1)),
            VisRowPos(3).linear_index(ncol, VisColumnPos(2)),
        );

        // Inside
        assert!(sel.contains(ncol, VisRowPos(1), VisColumnPos(1)));
        assert!(sel.contains(ncol, VisRowPos(2), VisColumnPos(2)));
        assert!(sel.contains(ncol, VisRowPos(3), VisColumnPos(1)));

        // Outside
        assert!(!sel.contains(ncol, VisRowPos(0), VisColumnPos(1)));
        assert!(!sel.contains(ncol, VisRowPos(2), VisColumnPos(3)));
        assert!(!sel.contains(ncol, VisRowPos(4), VisColumnPos(1)));
        assert!(!sel.contains(ncol, VisRowPos(2), VisColumnPos(0)));
    }

    #[test]
    fn vis_selection_contains_rect() {
        let ncol = 5;
        let outer = VisSelection::from_points(
            ncol,
            VisRowPos(0).linear_index(ncol, VisColumnPos(0)),
            VisRowPos(5).linear_index(ncol, VisColumnPos(4)),
        );
        let inner = VisSelection::from_points(
            ncol,
            VisRowPos(1).linear_index(ncol, VisColumnPos(1)),
            VisRowPos(3).linear_index(ncol, VisColumnPos(3)),
        );

        assert!(outer.contains_rect(ncol, inner));
        assert!(!inner.contains_rect(ncol, outer));
    }

    #[test]
    fn vis_selection_from_points_normalizes() {
        let ncol = 4;
        // Provide points in reverse order
        let sel = VisSelection::from_points(
            ncol,
            VisRowPos(3).linear_index(ncol, VisColumnPos(2)),
            VisRowPos(1).linear_index(ncol, VisColumnPos(0)),
        );

        let (top, left) = sel.0.row_col(ncol);
        let (bottom, right) = sel.1.row_col(ncol);

        assert_eq!(top, VisRowPos(1));
        assert_eq!(left, VisColumnPos(0));
        assert_eq!(bottom, VisRowPos(3));
        assert_eq!(right, VisColumnPos(2));
    }

    #[test]
    fn vis_selection_union() {
        let ncol = 5;
        let a = VisSelection::from_points(
            ncol,
            VisRowPos(1).linear_index(ncol, VisColumnPos(1)),
            VisRowPos(2).linear_index(ncol, VisColumnPos(2)),
        );
        let b = VisSelection::from_points(
            ncol,
            VisRowPos(3).linear_index(ncol, VisColumnPos(0)),
            VisRowPos(4).linear_index(ncol, VisColumnPos(3)),
        );
        let u = a.union(ncol, b);
        let (top, left) = u.0.row_col(ncol);
        let (bottom, right) = u.1.row_col(ncol);

        assert_eq!(top, VisRowPos(1));
        assert_eq!(left, VisColumnPos(0));
        assert_eq!(bottom, VisRowPos(4));
        assert_eq!(right, VisColumnPos(3));
    }

    #[test]
    fn vis_selection_is_point() {
        let ncol = 3;
        let idx = VisRowPos(2).linear_index(ncol, VisColumnPos(1));
        let sel = VisSelection(idx, idx);
        assert!(sel.is_point());

        let sel2 = VisSelection::from_points(
            ncol,
            VisRowPos(0).linear_index(ncol, VisColumnPos(0)),
            VisRowPos(1).linear_index(ncol, VisColumnPos(1)),
        );
        assert!(!sel2.is_point());
    }
}

// ========================= command system tests =========================

mod command_tests {
    use super::*;

    #[test]
    fn set_row_value_and_undo_redo() {
        let mut vwr = MockViewer::new(3);
        let mut table = DataTable::from_iter(vec![vec![1, 2, 3], vec![4, 5, 6]]);
        let mut state = setup_state(&mut vwr, &mut table);

        // Set row 0 to new value
        state.push_new_command(
            &mut table,
            &mut vwr,
            Command::SetRowValue(RowIdx(0), Box::new(vec![10, 20, 30])),
            100,
        );
        assert_eq!(table.rows[0], vec![10, 20, 30]);
        assert!(state.has_undo());

        // Undo
        assert!(state.undo(&mut table, &mut vwr));
        assert_eq!(table.rows[0], vec![1, 2, 3]);
        assert!(state.has_redo());

        // Redo
        assert!(state.redo(&mut table, &mut vwr));
        assert_eq!(table.rows[0], vec![10, 20, 30]);
    }

    #[test]
    fn insert_rows_and_undo() {
        let mut vwr = MockViewer::new(2);
        let mut table = DataTable::from_iter(vec![vec![1, 2], vec![3, 4]]);
        let mut state = setup_state(&mut vwr, &mut table);

        state.push_new_command(
            &mut table,
            &mut vwr,
            Command::InsertRows(RowIdx(1), vec![vec![10, 20], vec![30, 40]].into()),
            100,
        );
        assert_eq!(table.rows.len(), 4);
        assert_eq!(table.rows[1], vec![10, 20]);
        assert_eq!(table.rows[2], vec![30, 40]);
        assert_eq!(table.rows[3], vec![3, 4]);

        // Undo
        assert!(state.undo(&mut table, &mut vwr));
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0], vec![1, 2]);
        assert_eq!(table.rows[1], vec![3, 4]);
    }

    #[test]
    fn remove_row_and_undo() {
        let mut vwr = MockViewer::new(2);
        let mut table = DataTable::from_iter(vec![vec![1, 2], vec![3, 4], vec![5, 6]]);
        let mut state = setup_state(&mut vwr, &mut table);

        state.push_new_command(
            &mut table,
            &mut vwr,
            Command::RemoveRow(vec![RowIdx(1)]),
            100,
        );
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0], vec![1, 2]);
        assert_eq!(table.rows[1], vec![5, 6]);

        // Undo
        assert!(state.undo(&mut table, &mut vwr));
        assert_eq!(table.rows.len(), 3);
        assert_eq!(table.rows[1], vec![3, 4]);
    }

    #[test]
    fn undo_capacity_overflow_removes_oldest() {
        let mut vwr = MockViewer::new(2);
        let mut table = DataTable::from_iter(vec![vec![0, 0]]);
        let mut state = setup_state(&mut vwr, &mut table);
        let capacity = 3;

        for i in 1..=5 {
            state.push_new_command(
                &mut table,
                &mut vwr,
                Command::SetRowValue(RowIdx(0), Box::new(vec![i, i])),
                capacity,
            );
        }

        // Should be able to undo at most `capacity - 1` times (capacity includes current)
        let mut undo_count = 0;
        while state.undo(&mut table, &mut vwr) {
            undo_count += 1;
        }
        assert!(undo_count <= capacity);
    }

    #[test]
    fn new_command_clears_redo_stack() {
        let mut vwr = MockViewer::new(2);
        let mut table = DataTable::from_iter(vec![vec![0, 0]]);
        let mut state = setup_state(&mut vwr, &mut table);

        // Push two commands
        state.push_new_command(
            &mut table,
            &mut vwr,
            Command::SetRowValue(RowIdx(0), Box::new(vec![1, 1])),
            100,
        );
        state.push_new_command(
            &mut table,
            &mut vwr,
            Command::SetRowValue(RowIdx(0), Box::new(vec![2, 2])),
            100,
        );

        // Undo once
        state.undo(&mut table, &mut vwr);
        assert!(state.has_redo());

        // Push new command should clear redo
        state.push_new_command(
            &mut table,
            &mut vwr,
            Command::SetRowValue(RowIdx(0), Box::new(vec![3, 3])),
            100,
        );
        assert!(!state.has_redo());
    }

    #[test]
    fn undo_redo_on_empty_returns_false() {
        let mut vwr = MockViewer::new(2);
        let mut table = DataTable::from_iter(vec![vec![0, 0]]);
        let mut state = setup_state(&mut vwr, &mut table);

        assert!(!state.undo(&mut table, &mut vwr));
        assert!(!state.redo(&mut table, &mut vwr));
    }

    #[test]
    fn set_column_sort() {
        let mut vwr = MockViewer::new(3);
        let mut table = DataTable::from_iter(vec![vec![3, 1, 2], vec![1, 3, 1], vec![2, 2, 3]]);
        let mut state = setup_state(&mut vwr, &mut table);

        // Sort by column 0 ascending
        state.push_new_command(
            &mut table,
            &mut vwr,
            Command::SetColumnSort(vec![(ColumnIdx(0), IsAscending(true))]),
            100,
        );
        assert!(state.cc_is_dirty());

        // Validate to apply sorting
        state.validate_cc(&mut table.rows, &mut vwr);

        // cc_rows should be sorted by column 0
        assert_eq!(state.cc_rows.len(), 3);
        assert_eq!(state.cc_rows[0], RowIdx(1)); // value 1
        assert_eq!(state.cc_rows[1], RowIdx(2)); // value 2
        assert_eq!(state.cc_rows[2], RowIdx(0)); // value 3
    }

    #[test]
    fn set_visible_columns() {
        let mut vwr = MockViewer::new(4);
        let mut table = DataTable::from_iter(vec![vec![1, 2, 3, 4]]);
        let mut state = setup_state(&mut vwr, &mut table);

        assert_eq!(state.vis_cols().len(), 4);

        // Hide column 2
        state.push_new_command(
            &mut table,
            &mut vwr,
            Command::SetVisibleColumns(vec![ColumnIdx(0), ColumnIdx(1), ColumnIdx(3)]),
            100,
        );

        assert_eq!(state.vis_cols().len(), 3);
        assert!(!state.vis_cols().contains(&ColumnIdx(2)));

        // Undo
        state.undo(&mut table, &mut vwr);
        assert_eq!(state.vis_cols().len(), 4);
    }
}

// ========================= selection tests =========================

mod selection_tests {
    use super::*;

    fn make_state_with_rows(
        ncol: usize,
        nrows: usize,
    ) -> (MockViewer, DataTable<TestRow>, UiState<TestRow>) {
        let mut vwr = MockViewer::new(ncol);
        let rows: Vec<TestRow> = (0..nrows)
            .map(|i| (0..ncol).map(|c| (i * ncol + c) as i32).collect())
            .collect();
        let mut table = DataTable::from_iter(rows);
        let state = setup_state(&mut vwr, &mut table);
        (vwr, table, state)
    }

    #[test]
    fn cci_take_selection_none_replaces() {
        let (_, _, mut state) = make_state_with_rows(3, 5);

        // Set initial selection
        state.cc_cursor = CursorState::Select(vec![VisSelection(VisLinearIdx(0), VisLinearIdx(2))]);

        // Simulate mouse drag
        let idx = VisRowPos(2).linear_index(3, VisColumnPos(1));
        state.cci_sel_update(idx);

        let result = state.cci_take_selection(SelectionModifier::None);
        assert!(result.is_some());
        let sel = result.unwrap();
        assert_eq!(sel.len(), 1);
        // Point selection at (2,1)
        assert!(sel[0].contains(3, VisRowPos(2), VisColumnPos(1)));
    }

    #[test]
    fn cci_take_selection_toggle() {
        let (_, _, mut state) = make_state_with_rows(3, 5);

        // Existing selection at row 0
        state.cc_cursor = CursorState::Select(vec![VisSelection(VisLinearIdx(0), VisLinearIdx(2))]);

        // Add new selection at row 2
        let idx = VisRowPos(2).linear_index(3, VisColumnPos(1));
        state.cci_sel_update(idx);

        let result = state.cci_take_selection(SelectionModifier::Toggle);
        assert!(result.is_some());
        let sel = result.unwrap();
        // Should have 2 selections now (original + new)
        assert_eq!(sel.len(), 2);
    }

    #[test]
    fn cci_take_selection_toggle_removes_contained() {
        let (_, _, mut state) = make_state_with_rows(3, 5);

        // Existing selection at point (0,0)
        let point = VisLinearIdx(0);
        state.cc_cursor = CursorState::Select(vec![VisSelection(point, point)]);

        // Click same point again with toggle
        state.cci_sel_update(point);

        let result = state.cci_take_selection(SelectionModifier::Toggle);
        assert!(result.is_some());
        let sel = result.unwrap();
        // Should have removed the selection
        assert_eq!(sel.len(), 0);
    }

    #[test]
    fn cci_take_selection_extend() {
        let (_, _, mut state) = make_state_with_rows(3, 5);

        // Existing point selection at (0,0)
        let p0 = VisLinearIdx(0);
        state.cc_cursor = CursorState::Select(vec![VisSelection(p0, p0)]);

        // Extend to (2,1)
        let p1 = VisRowPos(2).linear_index(3, VisColumnPos(1));
        state.cci_sel_update(p1);

        let result = state.cci_take_selection(SelectionModifier::Extend);
        assert!(result.is_some());
        let sel = result.unwrap();
        assert_eq!(sel.len(), 1);
        // Should be the union of both points
        assert!(sel[0].contains(3, VisRowPos(0), VisColumnPos(0)));
        assert!(sel[0].contains(3, VisRowPos(2), VisColumnPos(1)));
    }

    #[test]
    fn cci_take_selection_returns_none_without_cci() {
        let (_, _, mut state) = make_state_with_rows(3, 5);
        // No cci_selection set
        let result = state.cci_take_selection(SelectionModifier::None);
        assert!(result.is_none());
    }

    #[test]
    fn collect_selection_basic() {
        let (_, _, mut state) = make_state_with_rows(3, 5);

        // Select from (1,0) to (2,1)
        state.cc_cursor = CursorState::Select(vec![VisSelection::from_points(
            3,
            VisRowPos(1).linear_index(3, VisColumnPos(0)),
            VisRowPos(2).linear_index(3, VisColumnPos(1)),
        )]);

        let cells = state.collect_selection();
        assert_eq!(cells.len(), 4); // 2 rows x 2 cols
        assert!(cells.contains(&(VisRowPos(1), VisColumnPos(0))));
        assert!(cells.contains(&(VisRowPos(1), VisColumnPos(1))));
        assert!(cells.contains(&(VisRowPos(2), VisColumnPos(0))));
        assert!(cells.contains(&(VisRowPos(2), VisColumnPos(1))));
    }

    #[test]
    fn collect_selected_rows() {
        let (_, _, mut state) = make_state_with_rows(4, 10);

        state.cc_cursor = CursorState::Select(vec![VisSelection::from_points(
            4,
            VisRowPos(2).linear_index(4, VisColumnPos(0)),
            VisRowPos(4).linear_index(4, VisColumnPos(3)),
        )]);

        let rows = state.collect_selected_rows();
        assert_eq!(rows.len(), 3);
        assert!(rows.contains(&VisRowPos(2)));
        assert!(rows.contains(&VisRowPos(3)));
        assert!(rows.contains(&VisRowPos(4)));
    }

    #[test]
    fn is_selected() {
        let (_, _, mut state) = make_state_with_rows(3, 5);

        state.cc_cursor = CursorState::Select(vec![VisSelection::from_points(
            3,
            VisRowPos(1).linear_index(3, VisColumnPos(1)),
            VisRowPos(1).linear_index(3, VisColumnPos(1)),
        )]);

        assert!(state.is_selected(VisRowPos(1), VisColumnPos(1)));
        assert!(!state.is_selected(VisRowPos(0), VisColumnPos(0)));
        assert!(!state.is_selected(VisRowPos(1), VisColumnPos(0)));
    }
}

// ========================= validation tests =========================

mod validation_tests {
    use super::*;

    #[test]
    fn validate_identity_resets_on_column_change() {
        let mut vwr = MockViewer::new(3);
        let mut table = DataTable::from_iter(vec![vec![1, 2, 3]]);
        let mut state = setup_state(&mut vwr, &mut table);

        assert_eq!(state.num_columns(), 3);

        // Change column count
        vwr.columns = 5;
        state.validate_identity(&mut vwr);

        assert_eq!(state.num_columns(), 5);
        assert_eq!(state.vis_cols().len(), 5);
        assert!(state.cc_is_dirty());
    }

    #[test]
    fn validate_identity_detects_filter_change() {
        let mut vwr = MockViewer::new(3);
        let mut table = DataTable::from_iter(vec![vec![1, 2, 3]]);
        let mut state = setup_state(&mut vwr, &mut table);

        // Clear dirty from setup
        state.validate_cc(&mut table.rows, &mut vwr);
        assert!(!state.cc_is_dirty());

        // Change filter hash
        vwr.filter_version = 42;
        state.validate_identity(&mut vwr);
        assert!(state.cc_is_dirty());
    }

    #[test]
    fn validate_cc_applies_sort() {
        let mut vwr = MockViewer::new(2);
        let mut table = DataTable::from_iter(vec![vec![30, 1], vec![10, 3], vec![20, 2]]);
        let mut state = setup_state(&mut vwr, &mut table);

        // Set sort by column 0 ascending
        state.push_new_command(
            &mut table,
            &mut vwr,
            Command::SetColumnSort(vec![(ColumnIdx(0), IsAscending(true))]),
            100,
        );
        state.validate_cc(&mut table.rows, &mut vwr);

        // Verify visual order is sorted
        assert_eq!(state.cc_rows[0], RowIdx(1)); // 10
        assert_eq!(state.cc_rows[1], RowIdx(2)); // 20
        assert_eq!(state.cc_rows[2], RowIdx(0)); // 30
    }

    #[test]
    fn validate_cc_applies_filter() {
        let mut vwr = MockViewer::new(2);
        vwr.filter_fn = Some(Box::new(|row: &TestRow| row[0] > 10));
        vwr.filter_version = 1;

        let mut table =
            DataTable::from_iter(vec![vec![5, 1], vec![15, 2], vec![25, 3], vec![3, 4]]);
        let state = setup_state(&mut vwr, &mut table);

        assert_eq!(state.cc_rows.len(), 2);
        assert_eq!(state.cc_rows[0], RowIdx(1)); // 15
        assert_eq!(state.cc_rows[1], RowIdx(2)); // 25
    }

    #[test]
    fn handle_desired_selection() {
        let mut vwr = MockViewer::new(3);
        let mut table = DataTable::from_iter(vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]]);
        let mut state = setup_state(&mut vwr, &mut table);

        // Queue desired selection for row 1 (full row)
        state.queue_select_rows([RowIdx(1)]);
        state.cc_dirty = true;
        state.validate_cc(&mut table.rows, &mut vwr);

        if let CursorState::Select(sel) = &state.cc_cursor {
            assert!(!sel.is_empty());
            // Should select the full row
            let (top, left) = sel[0].0.row_col(3);
            let (bottom, right) = sel[0].1.row_col(3);
            assert_eq!(top, VisRowPos(1));
            assert_eq!(left, VisColumnPos(0));
            assert_eq!(bottom, VisRowPos(1));
            assert_eq!(right, VisColumnPos(2));
        } else {
            panic!("Expected Select cursor state");
        }
    }
}

// ========================= action tests =========================

mod action_tests {
    use super::*;
    use crate::viewer::MoveDirection;

    #[test]
    fn move_selection_boundaries() {
        let mut vwr = MockViewer::new(3);
        let mut table = DataTable::from_iter(vec![vec![1, 2, 3], vec![4, 5, 6]]);
        let state = setup_state(&mut vwr, &mut table);

        // Test moved_position at boundaries
        let start = VisRowPos(0).linear_index(3, VisColumnPos(0));

        // Moving up from (0,0) stays at (0,0)
        let result = state.moved_position(start, MoveDirection::Up);
        assert_eq!(result.row_col(3), (VisRowPos(0), VisColumnPos(0)));

        // Moving left from (0,0) stays at (0,0)
        let result = state.moved_position(start, MoveDirection::Left);
        assert_eq!(result.row_col(3), (VisRowPos(0), VisColumnPos(0)));

        // Moving down from (0,0) goes to (1,0)
        let result = state.moved_position(start, MoveDirection::Down);
        assert_eq!(result.row_col(3), (VisRowPos(1), VisColumnPos(0)));

        // Moving right from (0,0) goes to (0,1)
        let result = state.moved_position(start, MoveDirection::Right);
        assert_eq!(result.row_col(3), (VisRowPos(0), VisColumnPos(1)));
    }

    #[test]
    fn move_selection_wraps_right() {
        let mut vwr = MockViewer::new(3);
        let mut table = DataTable::from_iter(vec![vec![1, 2, 3], vec![4, 5, 6]]);
        let state = setup_state(&mut vwr, &mut table);

        // Moving right from (0, 2) wraps to (1, 0)
        let start = VisRowPos(0).linear_index(3, VisColumnPos(2));
        let result = state.moved_position(start, MoveDirection::Right);
        assert_eq!(result.row_col(3), (VisRowPos(1), VisColumnPos(0)));
    }

    #[test]
    fn move_selection_wraps_left() {
        let mut vwr = MockViewer::new(3);
        let mut table = DataTable::from_iter(vec![vec![1, 2, 3], vec![4, 5, 6]]);
        let state = setup_state(&mut vwr, &mut table);

        // Moving left from (1, 0) wraps to (0, 2)
        let start = VisRowPos(1).linear_index(3, VisColumnPos(0));
        let result = state.moved_position(start, MoveDirection::Left);
        assert_eq!(result.row_col(3), (VisRowPos(0), VisColumnPos(2)));
    }

    #[test]
    fn move_selection_bottom_right_corner() {
        let mut vwr = MockViewer::new(3);
        let mut table = DataTable::from_iter(vec![vec![1, 2, 3], vec![4, 5, 6]]);
        let state = setup_state(&mut vwr, &mut table);

        // Moving down from bottom row stays
        let start = VisRowPos(1).linear_index(3, VisColumnPos(2));
        let result = state.moved_position(start, MoveDirection::Down);
        assert_eq!(result.row_col(3), (VisRowPos(1), VisColumnPos(2)));

        // Moving right from bottom-right corner stays
        let result = state.moved_position(start, MoveDirection::Right);
        assert_eq!(result.row_col(3), (VisRowPos(1), VisColumnPos(2)));
    }
}
