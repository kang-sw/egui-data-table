use std::sync::atomic::AtomicU64;

use slab::Slab;

pub mod ui;

/* ---------------------------------------------------------------------------------------------- */
/*                                           CORE CLASS                                           */
/* ---------------------------------------------------------------------------------------------- */

type RowSlotId = usize;

/// Prevents direct modification of `Vec`
#[derive(Debug, Clone)]
pub struct Spreadsheet<R> {
    /// Unique ID for this spreadsheet. Used for identifying cache entries during single
    /// process run..
    unique_id: u64,

    /// Next row ID to be allocated
    row_id_gen: u64,

    /// Efficient row data storage
    rows: Slab<RowSlot<R>>,
}

#[derive(Debug, Clone)]
struct RowSlot<R> {
    id: u64,
    data: R,
}

fn alloc_id() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl<R> Default for Spreadsheet<R> {
    fn default() -> Self {
        Self {
            unique_id: alloc_id(),
            row_id_gen: 0,
            rows: Default::default(),
        }
    }
}

impl<R> FromIterator<R> for Spreadsheet<R> {
    fn from_iter<T: IntoIterator<Item = R>>(iter: T) -> Self {
        let mut row_id_gen = 0;
        let rows = Slab::from_iter(iter.into_iter().enumerate().map(|(id, data)| {
            row_id_gen = id as u64;
            (
                id,
                RowSlot {
                    id: row_id_gen,
                    data,
                },
            )
        }));

        Self {
            unique_id: alloc_id(),
            row_id_gen,
            rows,
        }
    }
}

impl<R> Spreadsheet<R> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn num_rows(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn dump(&self) -> Vec<&R> {
        let mut slots = Vec::from_iter(self.rows.iter().map(|(_, row)| row));
        slots.sort_by_key(|row| row.id);
        slots.into_iter().map(|row| &row.data).collect()
    }

    pub fn compact(&mut self) {
        // TODO: Compact slab, indices, reassign unique_id.
    }

    pub fn retain(&mut self, mut f: impl FnMut(&R) -> bool) {
        let mut removed_any = false;
        self.rows.retain(|_, row| {
            let retain = f(&row.data);
            removed_any |= !retain;
            retain
        });

        if removed_any {
            self.unique_id = alloc_id();
        }
    }

    fn push_inner(&mut self, row: R) -> RowSlotId {
        let id = self.row_id_gen;
        self.row_id_gen += 1;
        self.rows.insert(RowSlot { id, data: row })
    }
}

impl<R> Extend<R> for Spreadsheet<R> {
    /// Programmatic extend operation will invalidate the index table cache.
    fn extend<T: IntoIterator<Item = R>>(&mut self, iter: T) {
        // Invalidate the cache
        self.unique_id = alloc_id();

        for row in iter {
            self.push_inner(row);
        }
    }
}
