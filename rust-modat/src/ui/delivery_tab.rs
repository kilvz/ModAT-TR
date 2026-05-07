use egui::{self};

impl crate::ModAtApp {
    pub(crate) fn render_delivery_reports_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Clear Reports").clicked() {
                self.clear_delivery_reports();
            }
            if ui.button("Clear Delivery Reports (Modem)").clicked() {
                if self.connected {
                    self.clear_modem_delivery();
                } else {
                    self.warning_message = Some("Connect to the modem first.".to_string());
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("To");
            ui.separator();
            ui.label("Type");
            ui.separator();
            ui.label("Status");
            ui.separator();
            ui.label("Sent At");
            ui.separator();
            ui.label("Updated");
            ui.separator();
            ui.label("Content Preview");
        });

        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                for (idx, dr) in self.dr_records.iter().enumerate() {
                    let label = format!(
                        "{} | {} | {} | {} | {} | {}",
                        dr.phone, dr.msg_type, dr.status, dr.sent, dr.updated, dr.content
                    );

                    if ui
                        .selectable_label(self.dr_selected == Some(idx), label)
                        .clicked()
                    {
                        self.dr_selected = Some(idx);
                    }
                }
            });

        if let Some(idx) = self.dr_selected {
            if let Some(dr) = self.dr_records.get(idx) {
                ui.group(|ui| {
                    ui.label(if dr.detail.is_empty() { "Pending delivery report" } else { &dr.detail });
                });
            }
        }
    }
}
