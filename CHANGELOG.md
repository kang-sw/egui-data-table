# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog],
and this project adheres to [Semantic Versioning].

## [Unreleased]

## [0.7.0]

### Added
- Translator/i18n support with a new `internationalization.rs` example.
- New demo features:
  - "Has modifications" checkbox and button to clear the state.
  - Column headers in the `partially_editable` example.
- Module-level documentation for the `partially_editable` example.
- Expanded `is_editable_cell` API to allow row-based cell editability checks, demonstrated with a "locked" column in the demo.

### Changed
- Improved `on_row_updated` API to pass old and new row values for change detection.
- Moved sort indicator to the right for better readability and alignment.
- Improved partial editing support.
- Ensured consistent sorting of values in the "processes" column.

### Fixed
- Prevented drag-and-drop onto non-editable cells.
- Fixed placeholder width for the sort indicator to handle multi-character indicators.
- Fixed a typo in the "Row protection" checkbox in the demo.

### Exposed
- Auto-shrink settings and scroll bar visibility in the API and demo.

### Prevented
- New row insertion when the table is empty and row insertion is disabled.

## [0.6.2]

### Added
- New viewer API `Viewer::is_editable_cell`, `Viewer::allow_row_insertions`, `Viewer::allow_row_deletions`

### Fixed
- Now editing cell correctly lose its focus when clicking outside of the table.
- Now default view of editing cell is correctly hidden.


## [0.6.0]

### Changed

- **BREAKING** Refactor `Viewer::column_render_config` to take additional parameter.

## [0.5.1]

### Added

- Implement `Clone`, `Deref`, `DerefMut` for `DataTable` widget.
- Implement `Serialize`, `Deserialize` for `DataTable` widget.

### Changed

- Manual dirty flag clearing now deprecated.

### Fixed


## [0.5.0] 

### Added

- New style flag to control editor behavior
  - `Style::single_click_edit_mode`: Make single click available to start edit mode.

### Removed

- `viewer::TrivialConfig` was removed.
  - Configs are integrated inside the `Style` of renderer. 

## [0.4.1] - 2024-12-14

### Added

- Introduce `crate::Style` struct, which defines set of properties to control internal
  behavior & visuals

### Changed

- Change various default visuals.

## [0.4.0] - 2024-11-21

### Changed

- Bump upstream dependency `egui` version 0.29

### Fixed

- Fix incorrect drag and drop area calculation logic

## [0.3.1] - 2024-08-18

### Added

- System clipboard support
- New trait item: `Codec`

## [0.3.0] - 2024-07-04

### Changed

- Upgraded EGUI dependency version to 0.28
- Remove function RPITIT in table viewer trait.

## [0.2.2] - 2024-05-11

### Added 

- New `Cargo.toml` feature `persistency`
- New API: `Viewer::persist_ui_state`
  - To persist UI state over sessions, return `true` on this trait method.  

## [0.1.4] - 2024-04-07

### Added

- New API: `RowViewer::clone_row_as_copied_base`
  - Replaces call to plain `clone_row` when it's triggered by user to copy contents of given row.

### Changed

- **BREAKING** 
  - `viewer::UiAction` is now `#[non_exhaustive]`
    - New enum variant `UiAction::InsertEmptyRows(NonZeroUsize)`, an action for inserting number of empty rows.
- Dependencies
  - egui 0.26 -> 0.27
  
## [0.1.3] - 2024-03-25

### Fixed

- Panic on row removal due to invalid index access 

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
