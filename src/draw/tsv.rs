//! A short implementation for reading and writing TSV data.

pub struct TsvWriter<'a, W> {
    writer: &'a mut W,
}

/* ============================================================================================== */
/*                                             READER                                             */
/* ============================================================================================== */

pub struct TsvReader<'a, R> {
    reader: &'a mut R,
}
