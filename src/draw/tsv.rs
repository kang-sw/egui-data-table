//! A short implementation for reading and writing TSV data.

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

pub struct TsvReader<'a, R> {
    reader: &'a mut R,
}
