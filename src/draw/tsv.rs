#![allow(unused)]
//! A short implementation for reading and writing TSV data.

use std::ops::Range;

pub fn write_tab(buf: &mut String) {
    buf.push('\t');
}

pub fn write_newline(buf: &mut String) {
    buf.push('\n');
}

pub fn write_content(buf: &mut String, mut item: &str) {
    if item.is_empty() {
        item = " ";
    }

    buf.reserve(item.len());

    for char in item.chars() {
        match char {
            '\t' => buf.push_str(r"\t"),
            '\n' => buf.push_str(r"\n"),
            '\r' => buf.push_str(r"\r"),
            '\\' => buf.push_str(r"\\"),
            _ => buf.push(char),
        }
    }
}

/* ============================================================================================== */
/*                                             READER                                             */
/* ============================================================================================== */

pub struct ParsedTsv {
    /// We need owned buffer to store escaped TSV data.
    data: String,

    /// Byte span info for each cell in the TSV data. As long as the cell is explicitly allocated
    /// using tab character, the cell will be stored in this vector even if it is empty.
    cell_spans: Vec<Range<u32>>,

    /// Index offsets for start of each row in the `cell_spans` vector.
    row_offsets: Vec<u32>,
}

impl ParsedTsv {
    pub fn parse(data: &str) -> Self {
        #[derive(Clone, Copy)]
        enum ParseState {
            Empty,
            Escaping,
        }

        let mut s = Self {
            data: Default::default(),
            cell_spans: Default::default(),
            row_offsets: Default::default(),
        };

        let mut state = ParseState::Empty;
        let mut cell_start_char = 0;

        // Add initial row offset.
        s.row_offsets.push(0);

        for char in data.chars() {
            match state {
                ParseState::Empty => match char {
                    '\n' | '\t' => {
                        if char == '\t' || cell_start_char != s.data.len() as u32 {
                            // For tab character, we don't care if it's empty cell. Otherwise,
                            // we add the last cell only when it's not empty.
                            s.cell_spans.push(cell_start_char..s.data.len() as u32);
                            cell_start_char = s.data.len() as _;
                        }

                        if char == '\n' {
                            // Add row offset and move to new row.
                            s.row_offsets.push(s.cell_spans.len() as _);
                        }
                    }
                    '\r' => {
                        // Ignoring.
                    }
                    '\\' => state = ParseState::Escaping,
                    ch => s.data.push(ch),
                },
                ParseState::Escaping => {
                    match char {
                        't' => s.data.push('\t'),
                        'n' => s.data.push('\n'),
                        'r' => s.data.push('\r'),
                        '\\' => s.data.push('\\'),
                        ch => {
                            // Just add the character as it is.
                            s.data.push('\\');
                            s.data.push(ch);
                        }
                    }

                    state = ParseState::Empty;
                }
            }
        }

        // Need to check if we have any remaining cell to add.
        {
            if cell_start_char != s.data.len() as u32 {
                s.cell_spans.push(cell_start_char..s.data.len() as u32);
            }

            if *s.row_offsets.last().unwrap() != s.cell_spans.len() as u32 {
                s.row_offsets.push(s.cell_spans.len() as _);
            }
        }

        // Optimize buffer usage.
        s.data.shrink_to_fit();
        s.cell_spans.shrink_to_fit();
        s.row_offsets.shrink_to_fit();

        s
    }

    /// Calculate the width of the table. This is the longest row in the table.
    pub fn calc_table_width(&self) -> usize {
        self.row_offsets
            .windows(2)
            .map(|range| range[1] - range[0])
            .max()
            .unwrap_or(0) as usize
    }

    pub fn num_columns_at(&self, row: usize) -> usize {
        if row >= self.row_offsets.len() - 1 {
            return 0;
        }

        let start = self.row_offsets[row] as usize;
        let end = self.row_offsets[row + 1] as usize;

        end - start
    }

    pub fn num_rows(&self) -> usize {
        self.row_offsets.len() - 1
    }

    pub fn get_cell(&self, row: usize, column: usize) -> Option<&str> {
        let row_offset = *self.row_offsets.get(row)? as usize;
        let cell_span = self.cell_spans.get(row_offset + column)?;

        Some(&self.data[cell_span.start as usize..cell_span.end as usize])
    }

    // TODO: Iterator function which returns (row, column, cell data) tuple.
    pub fn iter_rows(&self) -> impl Iterator<Item = (usize, impl Iterator<Item = (usize, &str)>)> {
        self.row_offsets
            .windows(2)
            .enumerate()
            .map(move |(row, range)| {
                let (start, end) = (range[0] as usize, range[1] as usize);
                let row_iter = (start..end).map(move |cell_offset| {
                    let cell_span = self.cell_spans.get(cell_offset).unwrap();
                    (
                        cell_offset - start,
                        &self.data[cell_span.start as usize..cell_span.end as usize],
                    )
                });

                (row, row_iter)
            })
    }

    #[cfg(test)]
    fn iter_index_data(&self) -> impl Iterator<Item = (usize, usize, &str)> {
        self.iter_rows()
            .flat_map(|(row, row_iter)| row_iter.map(move |(col, data)| (row, col, data)))
    }
}

#[test]
fn tsv_parsing() {
    const TSV_DATA: &str = "Hello\tWorld\nThis\tIs\tA\tTest";

    let parsed = ParsedTsv::parse(TSV_DATA);
    assert_eq!(parsed.num_columns_at(0), 2);
    assert_eq!(parsed.num_columns_at(1), 4);
    assert_eq!(parsed.num_columns_at(2), 0);

    assert_eq!(parsed.num_rows(), 2);

    assert_eq!(parsed.get_cell(0, 0), Some("Hello"));
    assert_eq!(parsed.get_cell(0, 1), Some("World"));
    assert_eq!(parsed.get_cell(1, 0), Some("This"));
    assert_eq!(parsed.get_cell(1, 1), Some("Is"));
    assert_eq!(parsed.get_cell(1, 2), Some("A"));
    assert_eq!(parsed.get_cell(1, 3), Some("Test"));
    assert!(parsed.get_cell(1, 4).is_none());

    assert_eq!(
        parsed.iter_index_data().collect::<Vec<_>>(),
        vec![
            (0, 0, "Hello"),
            (0, 1, "World"),
            (1, 0, "This"),
            (1, 1, "Is"),
            (1, 2, "A"),
            (1, 3, "Test"),
        ]
    );
}
