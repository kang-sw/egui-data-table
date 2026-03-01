use super::*;

impl<R> UiState<R> {
    pub fn try_update_clipboard_from_string<V: RowViewer<R>>(
        &mut self,
        vwr: &mut V,
        contents: &str,
    ) -> bool {
        /*
            NOTE: System clipboard implementation

            We can't just determine if the internal clipboard should be preferred over the system
            clipboard, as the source of clipboard contents can be vary. Therefore, on every copy
            operation, we should clear out the system clipboard unconditionally, then we tries to
            encode the copied content into clipboard.

            TODO: Need to find way to handle if both of system clipboard and internal clipboard
            content exist. We NEED to determine if which content should be applied for this.

            # Dumping

            - For rectangular(including single cell) selection of data, we'll just create
              appropriate sized small TSV data which suits within given range.
                - Note that this'll differentiate the clipboard behavior from internal-only
                  version.
            - For non-rectangular selections, full-scale rectangular table is dumped which
              can cover all selection range including empty selections; where any data that
              is not being dumped is just emptied out.
                - From this, any data cell that is just 'empty' but selected, should be dumped
                  as explicit empty data; in this case, empty data wrapped with double
                  quotes("").

            # Decoding

            - Every format is regarded as TSV. (only \t, \n matters)
            - For TSV data with same column count with this table
                - Parse as full-scale table, then put into clipboard as-is.
            - Column count is less than current table
                - In this case, current selection matters.
                - Offset the copied content table as the selection column offset.
                - Then create clipboard data from it.
            - If column count is larger than this, it is invalid data; we just skip parsing
        */

        let Some(mut codec) = vwr.try_create_codec(false) else {
            // Even when there is system clipboard content, we're going to ignore it and use
            // internal clipboard if there's no way to parse it.
            return false;
        };

        if let CursorState::Select(selections) = &self.cc_cursor {
            let Some(first) = selections.first().map(|x| x.0) else {
                // No selectgion present. Do nothing
                return false;
            };

            let (.., col) = first.row_col(self.p.vis_cols.len());
            col
        } else {
            // If there's no selection, we'll just ignore the system clipboard input
            return false;
        };

        let selection_offset = if let CursorState::Select(sel) = &self.cc_cursor {
            sel.first().map_or(0, |idx| {
                let (_, c) = idx.0.row_col(self.p.vis_cols.len());
                c.0
            })
        } else {
            0
        };

        let view = tsv::ParsedTsv::parse(contents);
        let table_width = view.calc_table_width();

        if table_width > self.p.vis_cols.len() {
            // If the copied data has more columns than current table, we'll just ignore it.
            return false;
        }

        // If any cell is failed to be parsed, we'll just give up all parsing then use internal
        // clipboard instead.

        let mut slab = Vec::new();
        let mut pastes = Vec::new();

        for (row_offset, row_data) in view.iter_rows() {
            let slab_id = slab.len();
            slab.push(codec.create_empty_decoded_row());

            // The restoration point of pastes stack.
            let pastes_restore = pastes.len();

            for (column, data) in row_data {
                let col_idx = column + selection_offset;

                if col_idx > self.p.vis_cols.len() {
                    // If the column is out of range, we'll just ignore it.
                    return false;
                }

                match codec.decode_column(data, col_idx, &mut slab[slab_id]) {
                    Ok(_) => {
                        pastes.push((
                            VisRowOffset(row_offset),
                            ColumnIdx(col_idx),
                            RowSlabIndex(slab_id),
                        ));
                    }
                    Err(DecodeErrorBehavior::SkipCell) => {
                        // Skip this cell.
                    }
                    Err(DecodeErrorBehavior::SkipRow) => {
                        pastes.drain(pastes_restore..);
                        slab.pop();
                        break;
                    }
                    Err(DecodeErrorBehavior::Abort) => {
                        return false;
                    }
                }
            }
        }

        // Replace the clipboard content from the parsed data.
        self.clipboard = Some(Clipboard {
            slab: slab.into_boxed_slice(),
            pastes: pastes.into_boxed_slice(),
        });

        true
    }

    pub(super) fn try_dump_clipboard_content<V: RowViewer<R>>(
        clipboard: &Clipboard<R>,
        vwr: &mut V,
    ) -> Option<String> {
        // clipboard MUST be sorted before dumping; XXX: add assertion?
        #[allow(unused_mut)]
        let mut codec = vwr.try_create_codec(true)?;

        let mut width = 0;
        let mut height = 0;

        // We're going to offset the column to the minimum column index to make the selection copy
        // more intuitive. If not, the copied data will be shifted to the right if the selection is
        // not the very first column.
        let mut min_column = usize::MAX;

        for (row, column, ..) in clipboard.pastes.iter() {
            width = width.max(column.0 + 1);
            height = height.max(row.0 + 1);
            min_column = min_column.min(column.0);
        }

        let column_offset = min_column;
        let mut buf_out = String::new();
        let mut buf_tmp = String::new();
        let mut row_cursor = 0;

        for (row, columns, ..) in &clipboard.pastes.iter().chunk_by(|(row, ..)| *row) {
            while row_cursor < row.0 {
                tsv::write_newline(&mut buf_out);
                row_cursor += 1;
            }

            let mut column_cursor = 0;

            for (_, column, data_idx) in columns.into_iter() {
                while column_cursor < column.0 - column_offset {
                    tsv::write_tab(&mut buf_out);
                    column_cursor += 1;
                }

                let data = &clipboard.slab[data_idx.0];
                codec.encode_column(data, column.0, &mut buf_tmp);

                tsv::write_content(&mut buf_out, &buf_tmp);
                buf_tmp.clear();
            }
        }

        Some(buf_out)
    }
}
