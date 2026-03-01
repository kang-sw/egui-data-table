use super::*;

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

impl<R> UiState<R> {
    pub fn push_new_command(
        &mut self,
        table: &mut DataTable<R>,
        vwr: &mut impl DataModelOps<R>,
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

    pub(super) fn cmd_apply(
        &mut self,
        table: &mut DataTable<R>,
        vwr: &mut impl DataModelOps<R>,
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
                    let _ = modified_rows
                        .entry(row.clone())
                        .or_insert_with(|| vwr.clone_row(&table.rows[row.0]));

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

    pub fn has_undo(&self) -> bool {
        self.undo_cursor < self.undo_queue.len()
    }

    pub fn has_redo(&self) -> bool {
        self.undo_cursor > 0
    }

    pub fn undo(&mut self, table: &mut DataTable<R>, vwr: &mut impl DataModelOps<R>) -> bool {
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

    pub fn redo(&mut self, table: &mut DataTable<R>, vwr: &mut impl DataModelOps<R>) -> bool {
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
}
