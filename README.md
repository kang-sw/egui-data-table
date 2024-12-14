[![Latest version](https://img.shields.io/crates/v/egui-data-table.svg)](https://crates.io/crates/egui-data-table)
[![Documentation](https://docs.rs/egui-data-table/badge.svg)](https://docs.rs/egui-data-table)

# Data table UI implementation for egui

MSRV is 1.75, with RPITIT

[Demo Web Page](https://kang-sw.github.io/egui-data-table/)

# Features

- [x] Undo/Redo for every editions
- [x] Show/Hide/Reorder columns
- [x] Row duplication / removal
- [x] Keyboard navigation
- [x] Internal clipboard support
- [x] System clipboard support
- [ ] Tutorials documentation
- [ ] Tidy light mode visuals

# Usage

In `Cargo.toml`, add `egui-data-table` to your dependencies section

```toml
[dependencies]
egui-data-table = "0.1"
```

Minimal example:

```rust no_run
// Use same version of `egui` with this crate!
use egui_data_table::egui;

// Don't need to implement any trait on row data itself.
struct MyRowData(i32, String, bool);

// Every logic is defined in `Viewer`
struct MyRowViewer;

// There are several methods that MUST be implemented to make the viewer work correctly.
impl egui_data_table::RowViewer<MyRowData> for MyRowViewer {
    fn num_columns(&mut self) -> usize {
        3
    }
    
    fn show_cell_view(&mut self, ui: &mut egui::Ui, row: &MyRowData, column: usize) {
        let _ = match column {
            0 => ui.label(format!("{}", row.0)),
            1 => ui.label(&row.1),
            2 => ui.checkbox(&mut { row.2 }, ""),
            _ => unreachable!()
        };
    }
    
    fn show_cell_editor(
        &mut self,
        ui: &mut egui::Ui,
        row: &mut MyRowData,
        column: usize,
    ) -> Option<egui::Response> {
        match column {
            0 => ui.add(egui::DragValue::new(&mut row.0).speed(1.0)),
            1 => {
                egui::TextEdit::multiline(&mut row.1)
                    .desired_rows(1)
                    .code_editor()
                    .show(ui)
                    .response
            }
            2 => ui.checkbox(&mut row.2, ""),
            _ => unreachable!()
        }
        .into() // To make focusing work correctly, valid response must be returned.
    }
    
    fn set_cell_value(&mut self, src: &MyRowData, dst: &mut MyRowData, column: usize) {
        match column {
            0 => dst.0 = src.0,
            1 => dst.1 = src.1.clone(),
            2 => dst.2 = src.2,
            _ => unreachable!()
        }
    }
    
    fn new_empty_row(&mut self) -> MyRowData {
        // Instead of requiring `Default` trait for row data types, the viewer is
        // responsible of providing default creation method.
        MyRowData(0, Default::default(), false)
    }
    
    // fn clone_row(&mut self, src: &MyRowData) -> MyRowData 
    // ^^ Overriding this method is optional. In default, it'll utilize `set_cell_value` which 
    //    would be less performant during huge duplication of lines.
}

fn show(ui: &mut egui::Ui, table: &mut egui_data_table::DataTable<MyRowData>) {
    ui.add(egui_data_table::Renderer::new(
        table,
        &mut { MyRowViewer },
    ));
}
```

For more details / advanced usage, see [demo](./examples/demo.rs)
