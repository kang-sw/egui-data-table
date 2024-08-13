use std::{borrow::Cow, iter::repeat_with};

use egui::{Response, Sense, Widget};
use egui_data_table::{
    viewer::{default_hotkeys, CellWriteContext, RowCodec, UiActionContext},
    RowViewer,
};

/* ----------------------------------------- Data Scheme ---------------------------------------- */

struct Viewer {
    filter: String,
    row_protection: bool,
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

/* -------------------------------------------- Codec ------------------------------------------- */

struct Codec;

impl RowCodec<Row> for Codec {
    type DeserializeError = &'static str;

    fn encode_column(&mut self, src_row: &Row, column: usize, dst: &mut String) {
        match column {
            0 => dst.push_str(&src_row.0),
            1 => dst.push_str(&src_row.1.to_string()),
            2 => dst.push_str(&src_row.2.to_string()),
            3 => dst.push_str(match src_row.3 {
                Grade::A => "A",
                Grade::B => "B",
                Grade::C => "C",
                Grade::F => "F",
            }),
            _ => unreachable!(),
        }
    }

    fn decode_column(
        &mut self,
        src_data: &str,
        column: usize,
        dst_row: &mut Row,
    ) -> Result<(), Self::DeserializeError> {
        unimplemented!()
    }
}

/* ------------------------------------ Viewer Implementation ----------------------------------- */

impl RowViewer<Row> for Viewer {
    fn try_create_codec(&mut self, is_encoding: bool) -> Option<impl RowCodec<Row>> {
        if is_encoding {
            Some(Codec)
        } else {
            None
        }
    }

    fn num_columns(&mut self) -> usize {
        4
    }

    fn column_name(&mut self, column: usize) -> Cow<'static, str> {
        [
            "Name (Click to sort)",
            "Age",
            "Is Student (Not sortable)",
            "Grade",
        ][column]
            .into()
    }

    fn is_sortable_column(&mut self, column: usize) -> bool {
        [true, true, false, true][column]
    }

    fn compare_cell(&self, row_l: &Row, row_r: &Row, column: usize) -> std::cmp::Ordering {
        match column {
            0 => row_l.0.cmp(&row_r.0),
            1 => row_l.1.cmp(&row_r.1),
            2 => unreachable!(),
            3 => row_l.3.cmp(&row_r.3),
            _ => unreachable!(),
        }
    }

    fn new_empty_row(&mut self) -> Row {
        Row("".to_string(), 0, false, Grade::F)
    }

    fn set_cell_value(&mut self, src: &Row, dst: &mut Row, column: usize) {
        match column {
            0 => dst.0.clone_from(&src.0),
            1 => dst.1 = src.1,
            2 => dst.2 = src.2,
            3 => dst.3 = src.3,
            _ => unreachable!(),
        }
    }

    fn confirm_cell_write_by_ui(
        &mut self,
        current: &Row,
        _next: &Row,
        _column: usize,
        _context: CellWriteContext,
    ) -> bool {
        if !self.row_protection {
            return true;
        }

        !current.2
    }

    fn confirm_row_deletion_by_ui(&mut self, row: &Row) -> bool {
        if !self.row_protection {
            return true;
        }

        !row.2
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

    fn filter_row(&mut self, row: &Row) -> bool {
        row.0.contains(&self.filter)
    }

    fn hotkeys(
        &mut self,
        context: &UiActionContext,
    ) -> Vec<(egui::KeyboardShortcut, egui_data_table::UiAction)> {
        let hotkeys = default_hotkeys(context);
        self.hotkeys.clone_from(&hotkeys);
        hotkeys
    }

    fn persist_ui_state(&self) -> bool {
        true
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
                row_protection: false,
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
                ui.hyperlink_to(
                    " kang-sw/egui-data-table",
                    "https://github.com/kang-sw/egui-data-table",
                );

                ui.hyperlink_to(
                    "(source)",
                    "https://github.com/kang-sw/egui-data-table/blob/master/examples/demo.rs",
                );

                ui.separator();

                egui::widgets::global_dark_light_mode_buttons(ui);

                ui.separator();

                ui.label("Name Filter");
                ui.text_edit_singleline(&mut self.viewer.filter);

                ui.add(egui::Button::new("Drag me and drop on any cell").sense(Sense::drag()))
                    .on_hover_text(
                        "Dropping this will replace the cell \
                        content with some predefined value.",
                    )
                    .dnd_set_drag_payload(String::from("Hallo~"));

                ui.checkbox(&mut self.viewer.row_protection, "Row Proection")
                    .on_hover_text(
                        "If checked, any rows `Is Student` marked \
                        won't be deleted or overwritten by UI actions.",
                    );
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
                            .wrap_mode(egui::TextWrapMode::Wrap)
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
    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let start_result = eframe::WebRunner::new()
            .start(
                "the_canvas_id",
                web_options,
                Box::new(|_cc| Ok(Box::new(DemoApp::default()))),
            )
            .await;

        // Remove the loading text and spinner:
        let loading_text = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("loading_text"));
        if let Some(loading_text) = loading_text {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
