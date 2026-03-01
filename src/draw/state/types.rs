use tap::prelude::Pipe;

macro_rules! int_ty {
(
    $(#[$meta:meta])*
    struct $name:ident ($($ty:ty),+); $($rest:tt)*) => {
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, PartialOrd, Ord)]
    $(#[$meta])*
    pub(crate) struct $name($(pub(in crate::draw) $ty),+);

    int_ty!($($rest)*);
};
() => {}
}

int_ty!(
    struct VisLinearIdx(usize);
    struct VisSelection(VisLinearIdx, VisLinearIdx);
    struct RowSlabIndex(usize);

    struct RowIdx(usize);
    struct VisRowPos(usize);
    struct VisRowOffset(usize);
    struct VisColumnPos(usize);

    #[cfg_attr(feature = "persistency", derive(serde::Serialize, serde::Deserialize))]
    struct IsAscending(bool);
    #[cfg_attr(feature = "persistency", derive(serde::Serialize, serde::Deserialize))]
    struct ColumnIdx(usize);
);

impl VisSelection {
    pub fn contains(&self, ncol: usize, row: VisRowPos, col: VisColumnPos) -> bool {
        let (top, left) = self.0.row_col(ncol);
        let (bottom, right) = self.1.row_col(ncol);

        row.0 >= top.0 && row.0 <= bottom.0 && col.0 >= left.0 && col.0 <= right.0
    }

    pub fn contains_rect(&self, ncol: usize, other: Self) -> bool {
        let (top, left) = self.0.row_col(ncol);
        let (bottom, right) = self.1.row_col(ncol);

        let (other_top, other_left) = other.0.row_col(ncol);
        let (other_bottom, other_right) = other.1.row_col(ncol);

        other_top.0 >= top.0
            && other_bottom.0 <= bottom.0
            && other_left.0 >= left.0
            && other_right.0 <= right.0
    }

    pub fn from_points(ncol: usize, a: VisLinearIdx, b: VisLinearIdx) -> Self {
        let (a_r, a_c) = a.row_col(ncol);
        let (b_r, b_c) = b.row_col(ncol);

        let top = a_r.0.min(b_r.0);
        let bottom = a_r.0.max(b_r.0);
        let left = a_c.0.min(b_c.0);
        let right = a_c.0.max(b_c.0);

        Self(
            VisLinearIdx(top * ncol + left),
            VisLinearIdx(bottom * ncol + right),
        )
    }

    pub fn is_point(&self) -> bool {
        self.0 == self.1
    }

    pub fn union(&self, ncol: usize, other: Self) -> Self {
        let (top, left) = self.0.row_col(ncol);
        let (bottom, right) = self.1.row_col(ncol);

        let (other_top, other_left) = other.0.row_col(ncol);
        let (other_bottom, other_right) = other.1.row_col(ncol);

        let top = top.0.min(other_top.0);
        let left = left.0.min(other_left.0);
        let bottom = bottom.0.max(other_bottom.0);
        let right = right.0.max(other_right.0);

        Self(
            VisLinearIdx(top * ncol + left),
            VisLinearIdx(bottom * ncol + right),
        )
    }

    pub fn _from_row_col(ncol: usize, r: VisRowPos, c: VisColumnPos) -> Self {
        r.linear_index(ncol, c).pipe(|idx| Self(idx, idx))
    }
}

impl From<VisLinearIdx> for VisSelection {
    fn from(value: VisLinearIdx) -> Self {
        Self(value, value)
    }
}

impl VisLinearIdx {
    pub fn row_col(&self, ncol: usize) -> (VisRowPos, VisColumnPos) {
        let (row, col) = (self.0 / ncol, self.0 % ncol);
        (VisRowPos(row), VisColumnPos(col))
    }
}

impl VisRowPos {
    pub fn linear_index(&self, ncol: usize, col: VisColumnPos) -> VisLinearIdx {
        VisLinearIdx(self.0 * ncol + col.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectionModifier {
    None,
    Toggle,
    Extend,
}
