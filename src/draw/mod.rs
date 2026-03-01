use std::mem::{replace, take};

use egui::{
    Align, Color32, Event, Label, Layout, PointerButton, PopupAnchor, Rect, Response, RichText,
    Sense, Stroke, StrokeKind, Tooltip, Vec2b,
};
use egui_extras::Column;
use tap::prelude::{Pipe, Tap};

use crate::{
    DataTable, UiAction,
    viewer::{EmptyRowCreateContext, RowViewer},
};

use self::state::*;

use egui::scroll_area::ScrollBarVisibility;
use format as f;
use std::sync::Arc;

mod body;
pub(crate) mod state;
mod tsv;

/* -------------------------------------------- Style ------------------------------------------- */

/// Style configuration for the table.
// TODO: Implement more style configurations.
#[derive(Default, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Style {
    /// Background color override for selection. Default uses `visuals.selection.bg_fill`.
    pub bg_selected_cell: Option<egui::Color32>,

    /// Background color override for selected cell. Default uses `visuals.selection.bg_fill`.
    pub bg_selected_highlight_cell: Option<egui::Color32>,

    /// Foreground color override for selected cell. Default uses `visuals.strong_text_colors`.
    pub fg_selected_highlight_cell: Option<egui::Color32>,

    /// Foreground color for cells that are going to be selected when mouse is dropped.
    pub fg_drag_selection: Option<egui::Color32>,

    /* ·························································································· */
    /// Maximum number of undo history. This is applied when actual action is performed.
    ///
    /// Setting value '0' results in kinda appropriate default value.
    pub max_undo_history: usize,

    /// If specify this as [`None`], the heterogeneous row height will be used.
    pub table_row_height: Option<f32>,

    /// When enabled, single click on a cell will start editing mode. Default is `false` where
    /// double action(click 1: select, click 2: edit) is required.
    pub single_click_edit_mode: bool,

    /// How to align cell contents. Default is left-aligned.
    pub cell_align: egui::Align,

    /// Color to use for the stroke above/below focused row.
    /// If `None`, defaults to a darkened `warn_fg_color`.
    pub focused_row_stroke: Option<egui::Color32>,

    /// See [`ScrollArea::auto_shrink`] for details.
    pub auto_shrink: Vec2b,

    /// See ['ScrollArea::ScrollBarVisibility`] for details.
    pub scroll_bar_visibility: ScrollBarVisibility,
}

/* ------------------------------------------ Rendering ----------------------------------------- */

pub struct Renderer<'a, R, V: RowViewer<R>> {
    table: &'a mut DataTable<R>,
    viewer: &'a mut V,
    state: Option<Box<UiState<R>>>,
    style: Style,
    translator: Arc<dyn Translator>,
}

impl<R, V: RowViewer<R>> egui::Widget for Renderer<'_, R, V> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        self.show(ui)
    }
}

impl<'a, R, V: RowViewer<R>> Renderer<'a, R, V> {
    pub fn new(table: &'a mut DataTable<R>, viewer: &'a mut V) -> Self {
        if table.rows.is_empty() && viewer.allow_row_insertions() {
            table.push(viewer.new_empty_row_for(EmptyRowCreateContext::InsertNewLine));
        }

        Self {
            state: Some(table.ui.take().unwrap_or_default().tap_mut(|state| {
                state.validate_identity(viewer);
            })),
            table,
            viewer,
            style: Default::default(),
            translator: Arc::new(EnglishTranslator::default()),
        }
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn with_style_modify(mut self, f: impl FnOnce(&mut Style)) -> Self {
        f(&mut self.style);
        self
    }

    pub fn with_table_row_height(mut self, height: f32) -> Self {
        self.style.table_row_height = Some(height);
        self
    }

    pub fn with_max_undo_history(mut self, max_undo_history: usize) -> Self {
        self.style.max_undo_history = max_undo_history;
        self
    }

    /// Sets a custom translator for the instance.
    /// # Example
    ///
    /// ```
    /// // Define a simple translator
    /// struct EsEsTranslator;
    /// impl Translator for EsEsTranslator {
    ///     fn translate(&self, key: &str) -> String {
    ///         match key {
    ///             "hello" => "Hola".to_string(),
    ///             "world" => "Mundo".to_string(),
    ///             _ => key.to_string(),
    ///         }
    ///     }
    /// }
    ///
    /// let renderer = Renderer::new(&mut table, &mut viewer)
    ///     .with_translator(Arc::new(EsEsTranslator));
    /// ```
    #[cfg(not(doctest))]
    pub fn with_translator(mut self, translator: Arc<dyn Translator>) -> Self {
        self.translator = translator;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> Response {
        egui::ScrollArea::horizontal()
            .show(ui, |ui| self.impl_show(ui))
            .inner
    }

    fn impl_show(mut self, ui: &mut egui::Ui) -> Response {
        let ctx = &ui.ctx().clone();
        let ui_id = ui.id();
        let style = ui.style().clone();
        let painter = ui.painter().clone();
        let visual = &style.visuals;
        let viewer = &mut *self.viewer;
        let s = self.state.as_mut().unwrap();
        let mut resp_total = None::<Response>;
        let mut resp_ret = None::<Response>;
        let mut commands = Vec::<Command<R>>::new();
        let ui_layer_id = ui.layer_id();

        // NOTE: unlike RED and YELLOW which can be acquirable through 'error_bg_color' and
        // 'warn_bg_color', there's no 'green' color which can be acquired from inherent theme.
        // Following logic simply gets 'green' color from current background's brightness.
        let green = if visual.window_fill.g() > 128 {
            Color32::DARK_GREEN
        } else {
            Color32::GREEN
        };

        let mut builder = egui_extras::TableBuilder::new(ui).column(Column::auto());

        let iter_vis_cols_with_flag = s
            .vis_cols()
            .iter()
            .enumerate()
            .map(|(index, column)| (column, index + 1 == s.vis_cols().len()));

        for (column, flag) in iter_vis_cols_with_flag {
            builder = builder.column(viewer.column_render_config(column.0, flag));
        }

        if replace(&mut s.cci_want_move_scroll, false) {
            let interact_row = s.interactive_cell().0;
            builder = builder.scroll_to_row(interact_row.0, None);
        }

        builder
            .columns(Column::auto(), s.num_columns() - s.vis_cols().len())
            .drag_to_scroll(false) // Drag is used for selection;
            .striped(true)
            .cell_layout(egui::Layout::default().with_cross_align(self.style.cell_align))
            .max_scroll_height(f32::MAX)
            .auto_shrink(self.style.auto_shrink)
            .scroll_bar_visibility(self.style.scroll_bar_visibility)
            .sense(Sense::click_and_drag().tap_mut(|s| s.set(Sense::FOCUSABLE, true)))
            .header(20., |mut h| {
                h.col(|_ui| {
                    // TODO: Add `Configure Sorting` button
                });

                let has_any_hidden_col = s.vis_cols().len() != s.num_columns();

                for (vis_col, &col) in s.vis_cols().iter().enumerate() {
                    let vis_col = VisColumnPos(vis_col);
                    let mut painter = None;
                    let (col_rect, resp) = h.col(|ui| {
                        egui::Sides::new().show(
                            ui,
                            |ui| {
                                ui.add(Label::new(viewer.column_name(col.0)).selectable(false));
                            },
                            |ui| {
                                if let Some(pos) = s.sort().iter().position(|(c, ..)| c == &col) {
                                    let is_asc = s.sort()[pos].1.0 as usize;

                                    ui.colored_label(
                                        [green, Color32::RED][is_asc],
                                        RichText::new(
                                            format!("{}{}", ["↘", "↗"][is_asc], pos + 1,),
                                        )
                                        .monospace(),
                                    );
                                } else {
                                    // calculate the maximum width for the sort indicator
                                    let max_sort_indicator_width =
                                        (s.num_columns() + 1).to_string().len() + 1;
                                    // when the sort indicator is present, create a label the same size as the sort indicator
                                    // so that the columns don't resize when sorted.
                                    ui.add(
                                        Label::new(
                                            RichText::new(" ".repeat(max_sort_indicator_width))
                                                .monospace(),
                                        )
                                        .selectable(false),
                                    );
                                }
                            },
                        );

                        painter = Some(ui.painter().clone());
                    });

                    // Set drag payload for column reordering.
                    resp.dnd_set_drag_payload(vis_col);

                    if resp.dragged() {
                        Tooltip::always_open(
                            ctx.clone(),
                            ui_layer_id,
                            "_EGUI_DATATABLE__COLUMN_MOVE__".into(),
                            PopupAnchor::Pointer,
                        )
                        .gap(12.0)
                        .show(|ui| {
                            let colum_name = viewer.column_name(col.0);
                            ui.label(colum_name);
                        });
                    }

                    if resp.hovered() && viewer.is_sortable_column(col.0) {
                        if let Some(p) = &painter {
                            p.rect_filled(
                                col_rect,
                                egui::CornerRadius::ZERO,
                                visual.selection.bg_fill.gamma_multiply(0.2),
                            );
                        }
                    }

                    if viewer.is_sortable_column(col.0) && resp.clicked_by(PointerButton::Primary) {
                        let mut sort = s.sort().to_owned();
                        match sort.iter_mut().find(|(c, ..)| c == &col) {
                            Some((_, asc)) => match asc.0 {
                                true => asc.0 = false,
                                false => sort.retain(|(c, ..)| c != &col),
                            },
                            None => {
                                sort.push((col, IsAscending(true)));
                            }
                        }

                        commands.push(Command::SetColumnSort(sort));
                    }

                    if resp.dnd_hover_payload::<VisColumnPos>().is_some() {
                        if let Some(p) = &painter {
                            p.rect_filled(
                                col_rect,
                                egui::CornerRadius::ZERO,
                                visual.selection.bg_fill.gamma_multiply(0.5),
                            );
                        }
                    }

                    if let Some(payload) = resp.dnd_release_payload::<VisColumnPos>() {
                        commands.push(Command::CcReorderColumn {
                            from: *payload,
                            to: vis_col
                                .0
                                .pipe(|v| v + (payload.0 < v) as usize)
                                .pipe(VisColumnPos),
                        })
                    }

                    resp.context_menu(|ui| {
                        if ui
                            .button(self.translator.translate("context-menu-hide"))
                            .clicked()
                        {
                            commands.push(Command::CcHideColumn(col));
                        }

                        if !s.sort().is_empty()
                            && ui
                                .button(self.translator.translate("context-menu-clear-sort"))
                                .clicked()
                        {
                            commands.push(Command::SetColumnSort(Vec::new()));
                        }

                        if has_any_hidden_col {
                            ui.separator();
                            ui.label(self.translator.translate("context-menu-hidden"));

                            for col in (0..s.num_columns()).map(ColumnIdx) {
                                if !s.vis_cols().contains(&col)
                                    && ui.button(viewer.column_name(col.0)).clicked()
                                {
                                    commands.push(Command::CcShowColumn {
                                        what: col,
                                        at: vis_col,
                                    });
                                }
                            }
                        }
                    });
                }

                // Account for header response to calculate total response.
                resp_total = Some(h.response());
            })
            .tap_mut(|table| {
                table.ui_mut().separator();
            })
            .body(|body: egui_extras::TableBody<'_>| {
                resp_ret = Some(
                    self.impl_show_body(body, painter, commands, ctx, &style, ui_id, resp_total),
                );
            });

        resp_ret.unwrap_or_else(|| ui.label("??"))
    }
}

impl<R, V: RowViewer<R>> Drop for Renderer<'_, R, V> {
    fn drop(&mut self) {
        self.table.ui = self.state.take();
    }
}

/* ------------------------------------------- Translations ------------------------------------- */

pub trait Translator {
    /// Translates a given key into its corresponding string representation.
    ///
    /// If the translation key is unknown, return the key as a [`String`]
    fn translate(&self, key: &str) -> String;
}

#[derive(Default)]
pub struct EnglishTranslator {}

impl Translator for EnglishTranslator {
    fn translate(&self, key: &str) -> String {
        match key {
            // cell context menu
            "context-menu-selection-copy" => "Selection: Copy",
            "context-menu-selection-cut" => "Selection: Cut",
            "context-menu-selection-clear" => "Selection: Clear",
            "context-menu-selection-fill" => "Selection: Fill",
            "context-menu-clipboard-paste" => "Clipboard: Paste",
            "context-menu-clipboard-insert" => "Clipboard: Insert",
            "context-menu-row-duplicate" => "Row: Duplicate",
            "context-menu-row-delete" => "Row: Delete",
            "context-menu-undo" => "Undo",
            "context-menu-redo" => "Redo",

            // column header context menu
            "context-menu-hide" => "Hide",
            "context-menu-hidden" => "Hidden",
            "context-menu-clear-sort" => "Clear sort",
            _ => key,
        }
        .to_string()
    }
}
