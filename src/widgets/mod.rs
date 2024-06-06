/* ============================================================================================== */
/*                                      COLUMN CONTROL PANEL                                      */
/* ============================================================================================== */

/// Widget to control active column visibility of
pub struct ColumnControlPanel<'a, R, V> {
    table: &'a mut crate::DataTable<R>,
    viewer: &'a mut V,
}

impl<'a, R, V> egui::Widget for ColumnControlPanel<'a, R, V>
where
    V: crate::RowViewer<R>,
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Self { table, viewer } = self;
        let Some(mut state) = table.ui.take() else {
            return ui.label("Not Available");
        };

        let num_cols = viewer.num_columns();
        let mut column_visibility = vec![false; num_cols];
        let response = ui.columns(2, |cols| {
            // Left(column 0) is the list of visible columns. Right(column 1) is the list of hidden
            // columns. By dragging a column from one list to the other, the user can change the
            // visibility of the column. Additionally, visible column list provides sort priority
            // and column order controls.

            // Render the visible columns list
            {
                let ui = &mut cols[0];

                ui.label("👁");
                ui.separator();

                ui.indent("Visible", |ui| {
                    for &column in state.vis_cols() {
                        column_visibility[column.0] = true;

                        // TODO: Display the column name

                        // TODO: Sort order / priority control

                        // TODO: Implement drag and drop
                    }
                });
            }

            {
                let ui = &mut cols[1];

                ui.label("✖");
                ui.separator();

                ui.indent("Hidden", |ui| {
                    for column in column_visibility
                        .iter()
                        .enumerate()
                        .filter(|(_, &visible)| !visible)
                    {
                        // TODO: Display the column name

                        // TODO: Implement drag and drop
                    }
                });
            }

            todo!()
        });

        // Put back the ui state
        table.ui = Some(state);

        response
    }
}

impl<'a, R, V> ColumnControlPanel<'a, R, V> {
    /// Create a new column control panel
    pub fn new(table: &'a mut crate::DataTable<R>, viewer: &'a mut V) -> Self {
        Self { table, viewer }
    }
}
