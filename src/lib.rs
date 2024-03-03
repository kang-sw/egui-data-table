use std::{collections::VecDeque, sync::atomic::AtomicU64};

pub mod draw;
pub mod viewer;

pub use viewer::{CellUiState, RowViewer, UiAction};

/* ---------------------------------------------------------------------------------------------- */
/*                                           CORE CLASS                                           */
/* ---------------------------------------------------------------------------------------------- */

/// Prevents direct modification of `Vec`
#[derive(Debug, Clone)]
pub struct Spreadsheet<R> {
    /// Unique ID for this spreadsheet. Used for identifying cache entries during single
    /// process run..
    unique_id: u64,

    /// Efficient row data storage
    rows: VecDeque<R>,
}

fn alloc_id() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl<R> Default for Spreadsheet<R> {
    fn default() -> Self {
        Self {
            unique_id: alloc_id(),
            rows: Default::default(),
        }
    }
}

impl<R> FromIterator<R> for Spreadsheet<R> {
    fn from_iter<T: IntoIterator<Item = R>>(iter: T) -> Self {
        Self {
            unique_id: alloc_id(),
            rows: iter.into_iter().collect(),
        }
    }
}

impl<R> Spreadsheet<R> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &R> {
        self.rows.iter()
    }

    pub fn take(&mut self) -> VecDeque<R> {
        std::mem::take(&mut self.rows)
    }

    pub fn replace(&mut self, new: VecDeque<R>) -> VecDeque<R> {
        std::mem::replace(&mut self.rows, new)
    }

    pub fn retain(&mut self, mut f: impl FnMut(&R) -> bool) {
        let mut removed_any = false;
        self.rows.retain(|row| {
            let retain = f(row);
            removed_any |= !retain;
            retain
        });

        if removed_any {
            self.unique_id = alloc_id();
        }
    }
}

impl<R> Extend<R> for Spreadsheet<R> {
    /// Programmatic extend operation will invalidate the index table cache.
    fn extend<T: IntoIterator<Item = R>>(&mut self, iter: T) {
        // Invalidate the cache
        self.unique_id = alloc_id();
        self.rows.extend(iter);
    }
}
