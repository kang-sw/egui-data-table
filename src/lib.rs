#![doc = include_str!("../README.md")]

pub mod draw;
pub mod viewer;

pub use draw::{Renderer, Style};
pub use viewer::{RowViewer, UiAction};

/// You may want to sync egui version with this crate.
pub extern crate egui;

/* ---------------------------------------------------------------------------------------------- */
/*                                           CORE CLASS                                           */
/* ---------------------------------------------------------------------------------------------- */

/// Prevents direct modification of `Vec`
pub struct DataTable<R> {
    /// Efficient row data storage
    ///
    /// XXX: If we use `VecDeque` here, it'd be more efficient when inserting new element
    /// at the beginning of the list. However, it does not support `splice` method like
    /// `Vec`, which results in extremely inefficient when there's multiple insertions.
    ///
    /// The efficiency order of general operations are only twice as slow when using
    /// `Vec`, we're just ignoring it for now. Maybe we can utilize `IndexMap` for this
    /// purpose, however, there are many trade-offs to consider, for now, we're just
    /// using `Vec` for simplicity.
    rows: Vec<R>,

    /// Is Dirty?
    dirty_flag: bool,

    /// Ui
    ui: Option<Box<draw::state::UiState<R>>>,
}

impl<R: std::fmt::Debug> std::fmt::Debug for DataTable<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spreadsheet")
            .field("rows", &self.rows)
            .finish()
    }
}

impl<R> Default for DataTable<R> {
    fn default() -> Self {
        Self {
            rows: Default::default(),
            ui: Default::default(),
            dirty_flag: false,
        }
    }
}

impl<R> FromIterator<R> for DataTable<R> {
    fn from_iter<T: IntoIterator<Item = R>>(iter: T) -> Self {
        Self {
            rows: iter.into_iter().collect(),
            ..Default::default()
        }
    }
}

impl<R> DataTable<R> {
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

    pub fn take(&mut self) -> Vec<R> {
        self.ui = None;
        std::mem::take(&mut self.rows)
    }

    pub fn replace(&mut self, new: Vec<R>) -> Vec<R> {
        self.ui = None;
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
            self.ui = None;
        }
    }

    pub fn clear_dirty_flag(&mut self) {
        self.dirty_flag = false;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty_flag
    }

    pub fn has_user_modification(&self) -> bool {
        self.dirty_flag
    }

    pub fn clear_user_modification_flag(&mut self) {
        self.dirty_flag = false;
    }
}

impl<R> Extend<R> for DataTable<R> {
    /// Programmatic extend operation will invalidate the index table cache.
    fn extend<T: IntoIterator<Item = R>>(&mut self, iter: T) {
        // Invalidate the cache
        self.ui = None;
        self.rows.extend(iter);
    }
}

fn default<T: Default>() -> T {
    T::default()
}
