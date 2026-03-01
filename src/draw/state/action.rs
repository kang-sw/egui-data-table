use super::*;

impl<R> UiState<R> {
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

                let mut commands = vec![Command::CcCommitEdit];

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
}
