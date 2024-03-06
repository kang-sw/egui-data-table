use std::{borrow::Cow, iter::repeat_with};

use egui::{Response, Sense, Widget};
use egui_data_table::{
    viewer::{default_hotkeys, UiActionContext},
    RowViewer,
};

/* ----------------------------------------- Data Scheme ---------------------------------------- */

struct Viewer {
    filter: String,
    hotkeys: Vec<(egui::KeyboardShortcut, egui_data_table::UiAction)>,
}

#[derive(Debug, Clone)]
struct Row(String, i32, bool, Grade);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Grade {
    A,
    B,
    C,
    F,
}

/* ------------------------------------ Viewer Implementation ----------------------------------- */

impl RowViewer<Row> for Viewer {
    fn num_columns(&mut self) -> usize {
        4
    }

    fn column_name(&mut self, column: usize) -> Cow<'static, str> {
        ["Name (Click To Sort)", "Age", "Is Student", "Grade"][column].into()
    }

    fn is_sortable_column(&mut self, column: usize) -> bool {
        [true, true, false, true][column]
    }

    fn create_cell_comparator(&mut self) -> fn(&Row, &Row, usize) -> std::cmp::Ordering {
        fn cmp(row_l: &Row, row_r: &Row, column: usize) -> std::cmp::Ordering {
            match column {
                0 => row_l.0.cmp(&row_r.0),
                1 => row_l.1.cmp(&row_r.1),
                2 => unreachable!(),
                3 => row_l.3.cmp(&row_r.3),
                _ => unreachable!(),
            }
        }

        cmp
    }

    fn new_empty_row(&mut self) -> Row {
        Row("".to_string(), 0, false, Grade::F)
    }

    fn set_cell_value(&mut self, src: &Row, dst: &mut Row, column: usize) {
        match column {
            0 => dst.0 = src.0.clone(),
            1 => dst.1 = src.1,
            2 => dst.2 = src.2,
            3 => dst.3 = src.3,
            _ => unreachable!(),
        }
    }

    fn show_cell_view(&mut self, ui: &mut egui::Ui, row: &Row, column: usize) {
        let _ = match column {
            0 => ui.label(&row.0),
            1 => ui.label(&row.1.to_string()),
            2 => ui.checkbox(&mut { row.2 }, ""),
            3 => ui.label(match row.3 {
                Grade::A => "A",
                Grade::B => "B",
                Grade::C => "C",
                Grade::F => "F",
            }),

            _ => unreachable!(),
        };
    }

    fn on_cell_view_response(
        &mut self,
        _row: &Row,
        _column: usize,
        resp: &egui::Response,
    ) -> Option<Box<Row>> {
        resp.dnd_release_payload::<String>()
            .map(|x| Box::new(Row((*x).clone(), 9999, false, Grade::A)))
    }

    fn show_cell_editor(
        &mut self,
        ui: &mut egui::Ui,
        row: &mut Row,
        column: usize,
    ) -> Option<Response> {
        match column {
            0 => {
                egui::TextEdit::multiline(&mut row.0)
                    .desired_rows(1)
                    .code_editor()
                    .show(ui)
                    .response
            }
            1 => ui.add(egui::DragValue::new(&mut row.1).speed(1.0)),
            2 => ui.checkbox(&mut row.2, ""),
            3 => {
                let grade = &mut row.3;
                ui.horizontal_wrapped(|ui| {
                    ui.radio_value(grade, Grade::A, "A")
                        | ui.radio_value(grade, Grade::B, "B")
                        | ui.radio_value(grade, Grade::C, "C")
                        | ui.radio_value(grade, Grade::F, "F")
                })
                .inner
            }
            _ => unreachable!(),
        }
        .into()
    }

    fn row_filter_hash(&mut self) -> &impl std::hash::Hash {
        &self.filter
    }

    fn create_row_filter(&mut self) -> impl Fn(&Row) -> bool {
        |r| r.0.contains(&self.filter)
    }

    fn hotkeys(
        &mut self,
        context: &UiActionContext,
    ) -> Vec<(egui::KeyboardShortcut, egui_data_table::UiAction)> {
        let hotkeys = default_hotkeys(context);
        self.hotkeys = hotkeys.clone();
        hotkeys
    }
}

/* ------------------------------------------ View Loop ----------------------------------------- */

struct DemoApp {
    table: egui_data_table::DataTable<Row>,
    viewer: Viewer,
}

impl Default for DemoApp {
    fn default() -> Self {
        Self {
            table: {
                let mut rng = fastrand::Rng::new();
                let mut name_gen = names::Generator::with_naming(names::Name::Numbered);

                repeat_with(move || {
                    Row(
                        name_gen.next().unwrap(),
                        rng.i32(4..31),
                        rng.bool(),
                        match rng.i32(0..=3) {
                            0 => Grade::A,
                            1 => Grade::B,
                            2 => Grade::C,
                            _ => Grade::F,
                        },
                    )
                })
            }
            .take(100000)
            .collect(),
            viewer: Viewer {
                filter: String::new(),
                hotkeys: Vec::new(),
            },
        }
    }
}

impl eframe::App for DemoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        fn is_send<T: Send>(_: &T) {}
        fn is_sync<T: Sync>(_: &T) {}

        is_send(&self.table);
        is_sync(&self.table);

        egui::TopBottomPanel::top("MenuBar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_dark_light_mode_buttons(ui);

                ui.separator();

                ui.label("Name Filter");
                ui.text_edit_singleline(&mut self.viewer.filter);

                ui.add(egui::Label::new("Drag me and drop on any cell").sense(Sense::drag()))
                    .dnd_set_drag_payload(String::from("Hallo~"));
            })
        });

        egui::SidePanel::left("Hotkeys")
            .default_width(500.)
            .show(ctx, |ui| {
                ui.vertical_centered_justified(|ui| {
                    ui.heading("Hotkeys");
                    ui.separator();
                    ui.add_space(0.);

                    for (k, a) in &self.viewer.hotkeys {
                        egui::Button::new(format!("{a:?}"))
                            .shortcut_text(ctx.format_shortcut(k))
                            .wrap(false)
                            .sense(Sense::hover())
                            .ui(ui);
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(egui_data_table::Renderer::new(
                &mut self.table,
                &mut self.viewer,
            ));
        });
    }
}

/* --------------------------------------- App Entrypoint --------------------------------------- */

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use eframe::App;

    eframe::run_simple_native(
        "Spreadsheet Demo",
        eframe::NativeOptions {
            default_theme: eframe::Theme::Dark,
            centered: true,

            ..Default::default()
        },
        {
            let mut app = DemoApp::default();
            move |ctx, frame| {
                app.update(ctx, frame);
            }
        },
    )
    .unwrap();
}

#[cfg(target_arch = "wasm32")]
fn main() {
    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "egui_data_table_demo",
                web_options,
                Box::new(|_| Box::new(DemoApp::default())),
            )
            .await
            .expect("failed to start eframe");
    });
}
