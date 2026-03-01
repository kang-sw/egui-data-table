use super::*;

impl<R> UiState<R> {
    pub fn is_selected(&self, row: VisRowPos, col: VisColumnPos) -> bool {
        if let CursorState::Select(selections) = &self.cc_cursor {
            selections
                .iter()
                .any(|sel| self.vis_sel_contains(*sel, row, col))
        } else {
            false
        }
    }

    pub fn is_selected_cci(&self, row: VisRowPos, col: VisColumnPos) -> bool {
        self.cci_selection.is_some_and(|(pivot, current)| {
            self.vis_sel_contains(
                VisSelection::from_points(self.p.vis_cols.len(), pivot, current),
                row,
                col,
            )
        })
    }

    pub fn vis_sel_contains(&self, sel: VisSelection, row: VisRowPos, col: VisColumnPos) -> bool {
        sel.contains(self.p.vis_cols.len(), row, col)
    }

    pub fn cci_sel_update(&mut self, current: VisLinearIdx) {
        if let Some((_, pivot)) = &mut self.cci_selection {
            *pivot = current;
        } else {
            self.cci_selection = Some((current, current));
        }
    }

    pub fn cci_sel_update_row(&mut self, row: VisRowPos) {
        [0, self.p.vis_cols.len() - 1].map(|col| {
            self.cci_sel_update(row.linear_index(self.p.vis_cols.len(), VisColumnPos(col)))
        });
    }

    pub fn has_cci_selection(&self) -> bool {
        self.cci_selection.is_some()
    }

    pub fn cci_take_selection(&mut self, mods: egui::Modifiers) -> Option<Vec<VisSelection>> {
        let ncol = self.p.vis_cols.len();
        let cci_sel = self
            .cci_selection
            .take()
            .map(|(_0, _1)| VisSelection::from_points(ncol, _0, _1))?;

        if mods.is_none() {
            return Some(vec![cci_sel]);
        }

        let mut sel = self.cursor_as_selection().unwrap_or_default().to_owned();
        let idx_contains = sel.iter().position(|x| x.contains_rect(ncol, cci_sel));
        if sel.is_empty() {
            sel.push(cci_sel);
            return Some(sel);
        }

        if mods.command_only() {
            if let Some(idx) = idx_contains {
                sel.remove(idx);
            } else {
                sel.push(cci_sel);
            }
        }

        if mods.cmd_ctrl_matches(Modifiers::SHIFT) {
            let last = sel.last_mut().unwrap();
            if cci_sel.is_point() && last.is_point() {
                *last = last.union(ncol, cci_sel);
            } else if idx_contains.is_none() {
                sel.push(cci_sel);
            };
        }

        Some(sel)
    }

    pub(super) fn collect_selection(&self) -> BTreeSet<(VisRowPos, VisColumnPos)> {
        let mut set = BTreeSet::new();

        if let CursorState::Select(selections) = &self.cc_cursor {
            for sel in selections.iter() {
                let (top, left) = sel.0.row_col(self.p.vis_cols.len());
                let (bottom, right) = sel.1.row_col(self.p.vis_cols.len());

                for r in top.0..=bottom.0 {
                    for c in left.0..=right.0 {
                        set.insert((VisRowPos(r), VisColumnPos(c)));
                    }
                }
            }
        }

        set
    }

    pub(super) fn collect_selected_rows(&self) -> BTreeSet<VisRowPos> {
        let mut rows = BTreeSet::new();

        if let CursorState::Select(selections) = &self.cc_cursor {
            for sel in selections.iter() {
                let (top, _) = sel.0.row_col(self.p.vis_cols.len());
                let (bottom, _) = sel.1.row_col(self.p.vis_cols.len());

                for r in top.0..=bottom.0 {
                    rows.insert(VisRowPos(r));
                }
            }
        }

        rows
    }

    pub(super) fn moved_position(&self, pos: VisLinearIdx, dir: MoveDirection) -> VisLinearIdx {
        let (VisRowPos(r), VisColumnPos(c)) = pos.row_col(self.p.vis_cols.len());

        let (rmax, cmax) = (
            self.cc_rows.len().saturating_sub(1),
            self.p.vis_cols.len().saturating_sub(1),
        );

        let (nr, nc) = match dir {
            MoveDirection::Up => match (r, c) {
                (0, c) => (0, c),
                (r, c) => (r - 1, c),
            },
            MoveDirection::Down => match (r, c) {
                (r, c) if r == rmax => (r, c),
                (r, c) => (r + 1, c),
            },
            MoveDirection::Left => match (r, c) {
                (0, 0) => (0, 0),
                (r, 0) => (r - 1, cmax),
                (r, c) => (r, c - 1),
            },
            MoveDirection::Right => match (r, c) {
                (r, c) if r == rmax && c == cmax => (r, c),
                (r, c) if c == cmax => (r + 1, 0),
                (r, c) => (r, c + 1),
            },
        };

        VisLinearIdx(nr * self.p.vis_cols.len() + nc)
    }

    pub(super) fn get_highlight_changes<'a>(
        &'a self,
        table: &'a DataTable<R>,
        sel: &[VisSelection],
    ) -> (Vec<&'a R>, Vec<&'a R>) {
        let mut ohs: BTreeSet<&VisSelection> = BTreeSet::default();
        let nhs: BTreeSet<&VisSelection> = sel.iter().collect();

        if let CursorState::Select(s) = &self.cc_cursor {
            ohs = s.iter().collect();
        }

        // IMPORTANT the new highlight may include a selection that includes the old highlight selection
        //           this happens when making a second multi-select using shift
        //           e.g.   old: 1..=5, 10..=10, new: 1..=5, 10..=15

        /// Flatten a set of ranges into a set of linear indices, e.g. [(1,3), (6,8)] -> [1,2,3,6,7,8]
        fn flatten_ranges(ranges: &[(usize, usize)]) -> HashSet<usize> {
            ranges.iter()
                .flat_map(|&(start, end)| start..=end)
                .collect()
        }

        /// Only keep elements in old_rows that are NOT in new_rows
        fn deselected_rows(old_rows: &HashSet<usize>, new_rows: &HashSet<usize>) -> Vec<usize> {
            let missing: Vec<usize> = old_rows
                .difference(&new_rows)
                .copied()
                .collect();

            missing
        }

        let nhs_range = self.make_row_range(&nhs);
        let ohs_range = self.make_row_range(&ohs);

        let nhs_rows = flatten_ranges(&nhs_range);
        let ohs_rows = flatten_ranges(&ohs_range);

        let deselected_rows = deselected_rows(&ohs_rows, &nhs_rows);

        let highlighted: Vec<&R> = nhs_rows
            .into_iter()
            .sorted()
            .map(|r| {
                let row_id = self.cc_rows[r];
                &table.rows[row_id.0]
            })
            .collect();
        let unhighlighted: Vec<&R> = deselected_rows
            .into_iter()
            .sorted()
            .map(|r| {
                let row_id = self.cc_rows[r];
                &table.rows[row_id.0]
            })
            .collect();

        (highlighted, unhighlighted)
    }

    fn make_row_range<'a>(&'a self, nhs: &BTreeSet<&VisSelection>) -> Vec<(usize, usize)> {
        nhs.iter().map(|sel| {
            let (start_ic_r, _ic_c) = sel.0.row_col(self.p.vis_cols.len());
            let (end_ic_r, _ic_c) = sel.1.row_col(self.p.vis_cols.len());
            (start_ic_r.0, end_ic_r.0)
        }).collect::<Vec<(usize, usize)>>()
    }
}
