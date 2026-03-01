use super::*;

impl<R> UiState<R> {
    pub fn validate_identity<V: DataModelOps<R> + 'static>(&mut self, vwr: &mut V) {
        let num_columns = vwr.num_columns();
        let vwr_type_id = std::any::TypeId::of::<V>();
        let vwr_hash = vwr.row_filter_hash();

        // Check for nontrivial changes.
        if self.p.num_columns == num_columns && self.viewer_type == vwr_type_id {
            // Check for trivial changes which does not require total reconstruction of
            // UiState.

            // If viewer's filter is changed. It always invalidates current cache.
            if self.viewer_filter_hash != vwr_hash {
                self.viewer_filter_hash = vwr_hash;
                self.cc_dirty = true;
            }

            // Defer validation of cache if it's still editing. This is prevent annoying re-sort
            // during editing multiple cells in-a-row without escape from insertion mode.
            {
                if !self.is_editing() {
                    self.cc_num_frame_from_last_edit += 1;
                }

                if self.cc_num_frame_from_last_edit == 2 {
                    self.cc_dirty |= !self.p.sort.is_empty();
                }
            }

            // Check if any sort config is invalidated.
            self.cc_dirty |= {
                let mut any_sort_invalidated = false;

                self.p.sort.retain(|(c, _)| {
                    vwr.is_sortable_column(c.0)
                        .tap(|x| any_sort_invalidated |= !x)
                });

                any_sort_invalidated
            };

            return;
        }

        // Clear the cache
        *self = Default::default();
        self.viewer_type = vwr_type_id;
        self.viewer_filter_hash = vwr_hash;
        self.p.num_columns = num_columns;

        self.p.vis_cols.extend((0..num_columns).map(ColumnIdx));
        self.cc_dirty = true;
    }

    #[cfg(feature = "persistency")]
    pub fn validate_persistency(
        &mut self,
        ctx: &egui::Context,
        ui_id: egui::Id,
        vwr: &mut impl DataModelOps<R>,
    ) {
        if !self.is_p_loaded {
            // Load initial storage status
            self.is_p_loaded = true;
            self.cc_dirty = true;
            let p: PersistData =
                ctx.memory_mut(|m| m.data.get_persisted(ui_id).unwrap_or_default());

            if p.num_columns == self.p.num_columns {
                // Data should only be copied when column count matches. Otherwise, we regard
                // stored column differs from the current.
                self.p = p;

                // Only retain valid sorting configuration.
                self.p.sort.retain(|(col, _)| vwr.is_sortable_column(col.0));
            }
        } else if self.cc_dirty {
            // Copy current ui status into persistency storage.
            ctx.memory_mut(|m| m.data.insert_persisted(ui_id, self.p.clone()));
        }
    }

    pub fn validate_cc(&mut self, rows: &mut [R], vwr: &mut impl DataModelOps<R>) {
        if !replace(&mut self.cc_dirty, false) {
            self.handle_desired_selection();
            return;
        }

        // XXX: Boost performance with `rayon`?
        // - Returning `comparator` which is marked as `Sync`
        // - For this, `R` also need to be sent to multiple threads safely.
        // - Maybe we need specialization for `R: Send`?

        // We should validate the entire cache.
        self.cc_rows.clear();
        self.cc_rows.extend(
            rows.iter()
                .enumerate()
                .filter_map(|(i, x)| vwr.filter_row(x).then_some(i))
                .map(RowIdx),
        );

        for (sort_col, asc) in self.p.sort.iter().rev() {
            self.cc_rows.sort_by(|a, b| {
                vwr.compare_cell(&rows[a.0], &rows[b.0], sort_col.0)
                    .tap_mut(|x| {
                        if !asc.0 {
                            *x = x.reverse()
                        }
                    })
            });
        }

        // Just refill with neat default height.
        self.cc_row_heights.resize(self.cc_rows.len(), 20.0);

        self.cc_row_id_to_vis.clear();
        self.cc_row_id_to_vis.extend(
            self.cc_rows
                .iter()
                .enumerate()
                .map(|(i, id)| (*id, VisRowPos(i))),
        );

        if self.handle_desired_selection() {
            // no-op.
        } else if let CursorState::Select(cursor) = &mut self.cc_cursor {
            // Validate cursor range if it's still in range.

            let old_cols = self.cc_prev_n_columns;
            let new_rows = self.cc_rows.len();
            let new_cols = self.p.num_columns;
            self.cc_prev_n_columns = self.p.num_columns;

            cursor.retain_mut(|sel| {
                let (old_min_r, old_min_c) = sel.0.row_col(old_cols);
                if old_min_r.0 >= new_rows || old_min_c.0 >= new_cols {
                    return false;
                }

                let (mut old_max_r, mut old_max_c) = sel.1.row_col(old_cols);
                old_max_r.0 = old_max_r.0.min(new_rows.saturating_sub(1));
                old_max_c.0 = old_max_c.0.min(new_cols.saturating_sub(1));

                let min = old_min_r.linear_index(new_cols, old_min_c);
                let max = old_max_r.linear_index(new_cols, old_max_c);
                *sel = VisSelection(min, max);

                true
            });
        } else {
            self.cc_cursor = CursorState::Select(Vec::default());
        }

        // Prevent overflow.
        self.validate_interactive_cell(self.p.vis_cols.len());
    }

    pub(super) fn handle_desired_selection(&mut self) -> bool {
        let Some((next_sel, sel)) = self.cc_desired_selection.take().and_then(|x| {
            if let CursorState::Select(vec) = &mut self.cc_cursor {
                Some((x, vec))
            } else {
                None
            }
        }) else {
            return false;
        };

        // If there's any desired selections present for next validation, apply it.

        sel.clear();
        let ncol = self.p.vis_cols.len();

        for (row_id, columns) in next_sel {
            let vis_row = self.cc_row_id_to_vis[&row_id];

            if columns.is_empty() {
                let p_left = vis_row.linear_index(ncol, VisColumnPos(0));
                let p_right = vis_row.linear_index(ncol, VisColumnPos(ncol - 1));

                sel.push(VisSelection(p_left, p_right));
            } else {
                for col in columns {
                    let Some(vis_c) = self.p.vis_cols.iter().position(|x| *x == col) else {
                        continue;
                    };

                    let p = vis_row.linear_index(ncol, VisColumnPos(vis_c));
                    sel.push(VisSelection(p, p));
                }
            }
        }

        true
    }
}
