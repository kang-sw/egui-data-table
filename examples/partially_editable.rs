//! Demonstrate the partially editable features.
//! 
//! Sometimes, some of the data you need to work with is not always editable, this example uses API features
//! to prevent new rows being added/deleted and to prevent some cells from being edited/cleared or pasted into.
//!
//! See [`Viewer::is_editable_cell`], [`Viewer::allow_row_insertions`] and [`Viewer::allow_row_deletions`] 

use std::borrow::Cow;
use egui::{Response, Ui};
use egui_data_table::RowViewer;
use std::collections::HashMap;
use tap::Tap;

#[derive(
    Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct Part {
    pub manufacturer: String,
    pub mpn: String,
}

impl Part {
    pub fn new(manufacturer: String, mpn: String) -> Self {
        Self { manufacturer, mpn }
    }
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PartWithState {
    pub part: Part,
    pub processes: Vec<String>,
}

#[derive(Debug)]
struct PartStatesRow {
    part: Part,
    enabled_processes: HashMap<String, bool>,
}

struct DemoApp {
    table: egui_data_table::DataTable<PartStatesRow>,
    viewer: Viewer,
}

impl Default for DemoApp {
    fn default() -> Self {
        let parts_states = vec![
            PartWithState {
                part: Part::new(
                    "Manufacturer 1".to_string(),
                    "MFR1MPN1".to_ascii_lowercase(),
                ),
                processes: vec!["pnp".to_string()],
            },
            PartWithState {
                part: Part::new(
                    "Manufacturer 2".to_string(),
                    "MFR2MPN1".to_ascii_lowercase(),
                ),
                processes: vec!["pnp".to_string()],
            },
            PartWithState {
                part: Part::new(
                    "Manufacturer 2".to_string(),
                    "MFR2MPN2".to_ascii_lowercase(),
                ),
                processes: vec!["manual".to_string()],
            },
        ];

        let processes: Vec<String> = vec!["manual".to_string(), "pnp".to_string()];

        let table = parts_states
            .iter()
            .map(|part_state| {
                let enabled_processes = processes
                    .iter()
                    .map(|process| (process.clone(), part_state.processes.contains(process)))
                    .collect::<HashMap<String, bool>>();

                PartStatesRow {
                    part: part_state.part.clone(),
                    enabled_processes,
                }
            })
            .collect();

        Self {
            table,
            viewer: Viewer::default(),
        }
    }
}

#[derive(Default)]
struct Viewer {
    pub enable_row_insertion: bool,
    pub enable_row_deletion: bool,
}

impl RowViewer<PartStatesRow> for Viewer {
    fn num_columns(&mut self) -> usize {
        3
    }

    fn column_name(&mut self, column: usize) -> Cow<'static, str> {
        match column {
            0 => "Manufacturer".into(),
            1 => "MPN".into(),
            2 => "Processes".into(),
            _ => unreachable!(),
        }
    }

    fn is_editable_cell(&mut self, column: usize, _row: usize, _row_value: &PartStatesRow) -> bool {
        match column {
            0 => false,
            1 => false,
            2 => true,
            _ => unreachable!(),
        }
    }

    fn allow_row_insertions(&mut self) -> bool {
        self.enable_row_insertion
    }

    fn allow_row_deletions(&mut self) -> bool {
        self.enable_row_deletion
    }

    fn show_cell_view(&mut self, ui: &mut Ui, row: &PartStatesRow, column: usize) {
        match column {
            0 => {
                ui.label(&row.part.manufacturer);
            }
            1 => {
                ui.label(&row.part.mpn);
            }
            2 => {
                let processes = row
                    .enabled_processes
                    .iter()
                    .filter_map(|(process, enabled)| {
                        if *enabled {
                            Some(process.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<String>>()
                    .tap_mut(|processes|processes.sort());
                let label = processes.join(", ");
                ui.label(label);
            }
            _ => unreachable!(),
        }
    }

    fn show_cell_editor(
        &mut self,
        ui: &mut Ui,
        row: &mut PartStatesRow,
        column: usize,
    ) -> Option<Response> {
        match column {
            2 => {
                let ui = ui.add(|ui: &mut Ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (name, enabled) in row.enabled_processes.iter_mut() {
                            ui.checkbox(enabled, name.clone());
                        }
                    })
                    .response
                });
                Some(ui)
            }
            _ => None,
        }
    }

    fn set_cell_value(&mut self, src: &PartStatesRow, dst: &mut PartStatesRow, column: usize) {
        match column {
            0 => dst.part.manufacturer = src.part.manufacturer.clone(),
            1 => dst.part.mpn = src.part.mpn.clone(),
            2 => dst.enabled_processes = src.enabled_processes.clone(),
            _ => unreachable!(),
        }
    }

    fn new_empty_row(&mut self) -> PartStatesRow {
        PartStatesRow {
            part: Part {
                manufacturer: "".to_string(),
                mpn: "".to_string(),
            },
            enabled_processes: Default::default(),
        }
    }
}

impl eframe::App for DemoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menubar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.checkbox(
                    &mut self.viewer.enable_row_insertion,
                    "Enable Row Insertion",
                );
                ui.checkbox(&mut self.viewer.enable_row_deletion, "Enable Row Deletion");
            });
        });

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            egui::Sides::new().show(ui, |_ui| {
            }, |ui|{
                let mut has_modifications = self.table.has_user_modification();
                ui.add_enabled(false, egui::Checkbox::new(&mut has_modifications, "Has modifications"));

                ui.add_enabled_ui(has_modifications, |ui| {
                    if ui.button("Clear").clicked() {
                        self.table.clear_user_modification_flag();
                    }
                });
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

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use eframe::App;
    env_logger::init();

    eframe::run_simple_native(
        "Partially editable demo",
        eframe::NativeOptions {
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
