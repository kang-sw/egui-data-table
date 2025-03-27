//! Demonstrate usage of the [`Translator`] trait.
//!
//! Note that it's possible to use more advanced translation systems like Fluent and egui_i18n.
//! by providing something that implements the [`Translator`] trait, this is beyond the scope of this example.

use std::borrow::Cow;
use std::collections::HashMap;
use egui::{ComboBox, Response, Ui};
use egui_data_table::RowViewer;
use std::iter::repeat_with;
use std::sync::Arc;
use egui_data_table::draw::{EnglishTranslator, Translator};

#[derive(Default)]
struct CustomSpanishTranslator {}

impl Translator for CustomSpanishTranslator {
    fn translate(&self, key: &str) -> String {
        match key {
            // custom translations
            "language" => "Idioma",

            // languages
            "en_US" => "Inglés (Estados Unidos)",
            "es_ES" => "Español (España)",

            // tables
            "table-column-header-name" => "Nombre",
            "table-column-header-number" => "Número",
            "table-column-header-flag" => "Indicador",

            // cell context menu
            "context-menu-selection-copy" => "Selección: Copiar",
            "context-menu-selection-cut" => "Selección: Cortar",
            "context-menu-selection-clear" => "Selección: Limpiar",
            "context-menu-selection-fill" => "Selección: Rellenar",
            "context-menu-clipboard-paste" => "Portapapeles: Pegar",
            "context-menu-clipboard-insert" => "Portapapeles: Insertar",
            "context-menu-row-duplicate" => "Fila: Duplicar",
            "context-menu-row-delete" => "Fila: Eliminar",
            "context-menu-undo" => "Deshacer",
            "context-menu-redo" => "Rehacer",

            // column header context menu
            "context-menu-hide" => "Ocultar columna",
            "context-menu-hidden" => "Columnas ocultas",
            "context-menu-clear-sort" => "Borrar ordenación",

            _ => key,
        }.to_string()
    }
}

/// Allows additional translation keys, and can fall back to the EnglishTranslator supplied by this crate.
#[derive(Default)]
struct CustomEnglishTranslator {
    fallback_translator: EnglishTranslator
}

impl Translator for CustomEnglishTranslator {
    fn translate(&self, key: &str) -> String {
        match key {
            // custom translations
            "language" => "Language".to_string(),

            // languages
            "en_US" => "English (United States)".to_string(),
            "es_ES" => "Spanish (Spain)".to_string(),

            // tables
            "table-column-header-name" => "Name".to_string(),
            "table-column-header-number" => "Number".to_string(),
            "table-column-header-flag" => "Flag".to_string(),

            // using the fallback translator for other keys
            _ => self.fallback_translator.translate(key),
        }
    }
}

struct DemoApp {
    table: egui_data_table::DataTable<Row>,
    viewer: Viewer,

    selected_language_key: String,
    translators: HashMap<&'static str, Arc<dyn Translator>>,
}

impl Default for DemoApp {
    fn default() -> Self {

        let translators: HashMap<&'static str, Arc<dyn Translator>> = vec![
            ("en_US", Arc::new(CustomEnglishTranslator::default()) as Arc<dyn Translator>),
            ("es_ES", Arc::new(CustomSpanishTranslator::default()) as Arc<dyn Translator>),
        ].into_iter().collect();

        let selected_language = "en_US".to_string();

        let translator = translators[selected_language.as_str()].clone();

        let table = {
            let mut rng = fastrand::Rng::new();
            let mut name_gen = names::Generator::with_naming(names::Name::Numbered);

            repeat_with(move || {
                Row(
                    name_gen.next().unwrap(),
                    rng.i32(4..31),
                    rng.bool(),
                )
            })
        }
            .take(10)
            .collect();

        Self {
            table,
            viewer: Viewer { translator: translator.clone() },
            selected_language_key: selected_language.to_string(),
            translators,
        }
    }
}

#[derive(Debug, Clone)]
struct Row(String, i32, bool);

struct Viewer {
    translator: Arc<dyn Translator>,
}

impl Viewer {
    fn change_translator(&mut self, translator: Arc<dyn Translator>) {
        self.translator = translator;
    }
}

impl RowViewer<Row> for Viewer {
    fn num_columns(&mut self) -> usize {
        3
    }

    fn column_name(&mut self, column: usize) -> Cow<'static, str> {
        match column {
            0 => self.translator.translate("table-column-header-name").into(),
            1 => self.translator.translate("table-column-header-number").into(),
            2 => self.translator.translate("table-column-header-flag").into(),
            _ => unreachable!(),
        }
    }

    fn is_editable_cell(&mut self, column: usize, _row: usize, _row_value: &Row) -> bool {
        match column {
            0 => true,
            1 => true,
            2 => true,
            _ => unreachable!(),
        }
    }

    fn show_cell_view(&mut self, ui: &mut Ui, row: &Row, column: usize) {
        match column {
            0 => ui.label(&row.0),
            1 => ui.label(row.1.to_string()),
            2 => ui.checkbox(&mut { row.2 }, ""),
            _ => unreachable!(),
        };
    }

    fn show_cell_editor(
        &mut self,
        ui: &mut Ui,
        row: &mut Row,
        column: usize,
    ) -> Option<Response> {
        match column {
            0 => {
                egui::TextEdit::singleline(&mut row.0)
                    .show(ui)
                    .response
            }
            1 => ui.add(egui::DragValue::new(&mut row.1).speed(1.0)),
            2 => ui.checkbox(&mut row.2, ""),
            _ => unreachable!(),
        }
        .into()
    }

    fn set_cell_value(&mut self, src: &Row, dst: &mut Row, column: usize) {
        match column {
            0 => dst.0.clone_from(&src.0),
            1 => dst.1 = src.1,
            2 => dst.2 = src.2,
            _ => unreachable!(),
        }
    }

    fn new_empty_row(&mut self) -> Row {
        Row("".to_string(), 0, false)
    }
}

impl eframe::App for DemoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        let mut language_keys: Vec<&str> = self.translators.keys().copied().collect();
        language_keys.sort();

        let translator = self.translators[&self.selected_language_key.as_str()].clone();

        egui::TopBottomPanel::top("menubar").show(ctx, |ui| {
            ComboBox::from_label(translator.translate("language"))
                .selected_text(translator.translate(&self.selected_language_key))
                .show_ui(ui, |ui| {
                    for &language_key in &language_keys {
                        let language = translator.translate(language_key);
                        if ui.selectable_label(self.selected_language_key == language_key, language).clicked() {
                            self.selected_language_key = language_key.to_string();
                            self.viewer.change_translator(self.translators[&self.selected_language_key.as_str()].clone());
                        }
                    }
                });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            let renderer = egui_data_table::Renderer::new(
                &mut self.table,
                &mut self.viewer,
            )
                .with_translator(translator);

            ui.add(renderer);
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use eframe::App;
    env_logger::init();

    eframe::run_simple_native(
        "Translator demo",
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
