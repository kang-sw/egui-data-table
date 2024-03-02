use std::iter::repeat_with;

use egui_spreadsheet::ui::RowViewer;

/* ----------------------------------------- Data Types ----------------------------------------- */

struct Viewer;

struct Row(String, i32, bool, Grade);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Grade {
    A,
    B,
    C,
    F,
}

/* ------------------------------------ Viewer Implementation ----------------------------------- */

impl RowViewer<Row> for Viewer {
    const COLUMNS: usize = 4;

    fn column_name(&mut self, column: usize) -> &str {
        ["Name", "Age", "Is Student", "Grade"][column]
    }

    fn is_sortable_column(&mut self, column: usize) -> bool {
        [true, true, true, false][column]
    }

    fn compare_column(&mut self, row_l: &Row, row_r: &Row, column: usize) -> std::cmp::Ordering {
        match column {
            0 => row_l.0.cmp(&row_r.0),
            1 => row_l.1.cmp(&row_r.1),
            2 => row_l.2.cmp(&row_r.2),
            _ => unreachable!(),
        }
    }

    fn empty_row(&mut self) -> Row {
        Row("".to_string(), 0, false, Grade::F)
    }

    fn clone_column(&mut self, src: &Row, dst: &mut Row, column: usize) {
        match column {
            0 => dst.0 = src.0.clone(),
            1 => dst.1 = src.1,
            2 => dst.2 = src.2,
            3 => dst.3 = src.3,
            _ => unreachable!(),
        }
    }

    fn clone_row(&mut self, src: &Row) -> Row {
        Row(src.0.clone(), src.1, src.2, src.3)
    }

    fn clear_column(&mut self, row: &mut Row, column: usize) {
        match column {
            0 => row.0.clear(),
            1 => row.1 = 0,
            2 => row.2 = false,
            3 => row.3 = Grade::F,
            _ => unreachable!(),
        }
    }

    fn draw_column_edit(&mut self, ui: &mut egui::Ui, row: &mut Row, column: usize, _active: bool) {
        match column {
            0 => {
                ui.text_edit_singleline(&mut row.0);
            }
            1 => {
                ui.add(egui::widgets::DragValue::new(&mut row.1).speed(1.0));
            }
            2 => {
                ui.checkbox(&mut row.2, "");
            }
            3 => {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut row.3, Grade::A, "A");
                    ui.radio_value(&mut row.3, Grade::B, "B");
                    ui.radio_value(&mut row.3, Grade::C, "C");
                    ui.radio_value(&mut row.3, Grade::F, "F");
                });
            }
            _ => unreachable!(),
        }
    }
}

/* ------------------------------------------ View Loop ----------------------------------------- */

struct DemoApp {
    sheet: egui_spreadsheet::Spreadsheet<Row>,
}

impl Default for DemoApp {
    fn default() -> Self {
        Self {
            sheet: repeat_with(|| Row("".to_string(), 0, false, Grade::F))
                .take(100)
                .collect(),
        }
    }
}

impl DemoApp {
    fn tick(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| self.sheet.show(ui, "asdF", &mut Viewer));
    }
}

/* --------------------------------------- App Entrypoint --------------------------------------- */

#[cfg(target_arch = "x86_64")]
fn main() {
    eframe::run_simple_native(
        "Spreadsheet Demo",
        eframe::NativeOptions {
            default_theme: eframe::Theme::Dark,

            ..Default::default()
        },
        {
            let mut app = DemoApp::default();
            move |ctx, frame| {
                app.tick(ctx, frame);
            }
        },
    )
    .unwrap();
}
