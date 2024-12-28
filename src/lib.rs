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

    pub fn take(&mut self) -> Vec<R> {
        self.mark_dirty();
        std::mem::take(&mut self.rows)
    }

    /// Replace the current data with the new one.
    pub fn replace(&mut self, new: Vec<R>) -> Vec<R> {
        self.mark_dirty();
        std::mem::replace(&mut self.rows, new)
    }

    /// Insert a row at the specified index. This is thin wrapper of `Vec::retain` which provides
    /// additional dirty flag optimization.
    pub fn retain(&mut self, mut f: impl FnMut(&R) -> bool) {
        let mut removed_any = false;
        self.rows.retain(|row| {
            let retain = f(row);
            removed_any |= !retain;
            retain
        });

        if removed_any {
            self.mark_dirty();
        }
    }

    /// Check if the UI is obsolete and needs to be re-rendered due to data changes.
    pub fn is_dirty(&self) -> bool {
        self.ui.as_ref().is_some_and(|ui| ui.cc_is_dirty())
    }

    #[deprecated(
        since = "0.5.1",
        note = "user-driven dirty flag clearance is redundant"
    )]
    pub fn clear_dirty_flag(&mut self) {
        // This is intentionally became a no-op
    }

    fn mark_dirty(&mut self) {
        let Some(state) = self.ui.as_mut() else {
            return;
        };

        state.force_mark_dirty();
    }

    /// Returns true if there were any user-driven(triggered by UI) modifications.
    pub fn has_user_modification(&self) -> bool {
        self.dirty_flag
    }

    /// Clears the user-driven(triggered by UI) modification flag.
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

impl<R> std::ops::Deref for DataTable<R> {
    type Target = Vec<R>;

    fn deref(&self) -> &Self::Target {
        &self.rows
    }
}

impl<R> std::ops::DerefMut for DataTable<R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.mark_dirty();
        &mut self.rows
    }
}

impl<R: Clone> Clone for DataTable<R> {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows.clone(),
            // UI field is treated as cache.
            ui: None,
            dirty_flag: self.dirty_flag,
        }
    }
}
