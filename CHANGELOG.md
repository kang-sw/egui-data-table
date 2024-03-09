# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog],
and this project adheres to [Semantic Versioning].

## [Unreleased]

### Added

- New API: `RowViewer::clone_row_as_copied_base`
  - Replaces call to plain `clone_row` when it's triggered by user to copy contents of given row.

### Changed

- **BREAKING** 
  - `viewer::UiAction` is now `#[non_exhaustive]`
    - New enum variant `UiAction::InsertEmptyRows(NonZeroUsize)`, an action for inserting number of empty rows.

## [0.1.2] - 2024-03-09

Add more controls for viewer.

### Added

- New `RowViewer` APIs for detailed control of user interaction.
  - `RowViewer::confirm_cell_write`
    - New enum `viewer::CellWriteContext`
  - `RowViewer::confirm_row_deletion`
  - `RowViewer::clone_row_for_insertion`
  - `RowViewer::on_highlight_cell`
  - `RowViewer::new_empty_row_for`
    - New enum `viewer::EmptyRowCreateContext`

### Changed

- Insert `cargo-semver-checks` on Cargo Publish task.

## [0.1.1] - 2024-03-07

### Added

- Initial implementation with features
  - [x] Undo/Redo for every editions
  - [x] Show/Hide/Reorder columns
  - [x] Row duplication / removal
  - [x] Keyboard navigation
  - [x] Internal clipboard support

## [Wishlist]

- [ ] System clipboard support
- [ ] Tutorials documentation
- [ ] Tidy light mode visuals

<!-- Links -->
[keep a changelog]: https://keepachangelog.com/en/1.0.0/
[semantic versioning]: https://semver.org/spec/v2.0.0.html

<!-- Versions -->
[unreleased]: https://github.com/kang-sw/egui-data-table/compare/v0.0.2...HEAD
[0.0.2]: https://github.com/kang-sw/egui-data-table/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/kang-sw/egui-data-table/releases/tag/v0.0.1
