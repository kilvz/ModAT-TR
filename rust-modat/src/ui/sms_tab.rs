use egui::{self, Color32, RichText, Stroke, TextEdit};

use crate::ui::helpers::{category_label, log_color};

impl crate::ModAtApp {
    pub(crate) fn render_sms_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(10.0, 8.0);
        egui::Frame::group(ui.style())
            .fill(Color32::from_rgb(24, 28, 36))
            .stroke(Stroke::new(1.0, Color32::from_rgb(55, 65, 81)))
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Send SMS")
                            .heading()
                            .strong()
                            .color(Color32::from_rgb(139, 233, 253)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(&self.char_count)
                                .monospace()
                                .color(Color32::from_rgb(189, 147, 249)),
                        );
                    });
                });
                ui.separator();

                egui::Grid::new("sms_compose_grid")
                    .num_columns(2)
                    .spacing([12.0, 10.0])
                    .min_col_width(90.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Recipients").strong());
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [ui.available_width().max(180.0) - 115.0, 24.0],
                                TextEdit::singleline(&mut self.phone_number),
                            );
                            if ui.button("Add Contact").clicked() {
                                if self.phone_number.is_empty() {
                                    self.warning_message =
                                        Some("Enter a phone number first".to_string());
                                } else {
                                    self.contact_name_input.clear();
                                    self.contact_number_input = self
                                        .phone_number
                                        .split(',')
                                        .next()
                                        .unwrap_or("")
                                        .trim()
                                        .to_string();
                                    self.show_add_contact = true;
                                }
                            }
                        });
                        ui.end_row();

                        ui.label(RichText::new("Message").strong());
                        ui.add(
                            TextEdit::multiline(&mut self.message_text)
                                .desired_rows(6)
                                .desired_width(f32::INFINITY),
                        );
                        ui.end_row();

                        ui.label(RichText::new("Encoding").strong());
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Class:");
                            let old_class = self.sms_class;
                            egui::ComboBox::from_id_salt("sms_class_combo")
                                .width(150.0)
                                .selected_text(&self.sms_class_options[self.sms_class])
                                .show_ui(ui, |ui| {
                                    for (idx, option) in self.sms_class_options.iter().enumerate() {
                                        ui.selectable_value(&mut self.sms_class, idx, option);
                                    }
                                });
                            if self.sms_class != old_class {
                                self.sync_dcs_from_class();
                            }

                            ui.separator();
                            ui.label("DCS:");
                            let old_dcs = self.dcs_value.clone();
                            egui::ComboBox::from_id_salt("dcs_combo")
                                .width(230.0)
                                .selected_text(&self.dcs_value)
                                .show_ui(ui, |ui| {
                                    for option in &self.dcs_options {
                                        ui.selectable_value(
                                            &mut self.dcs_value,
                                            option.clone(),
                                            option,
                                        );
                                    }
                                });
                            if self.dcs_value != old_dcs {
                                self.sync_class_from_dcs();
                            }
                        });
                        ui.end_row();

                        ui.label("");
                        ui.checkbox(&mut self.delivery_report, "Request Delivery Report");
                        ui.end_row();
                    });

                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui.button(RichText::new("Send SMS").strong()).clicked() {
                        let phones: Vec<String> = self
                            .phone_number
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .collect();
                        let message = self.message_text.clone();
                        if phones.iter().all(|p| p.is_empty()) || message.trim().is_empty() {
                            self.warning_message =
                                Some("Enter phone number(s) and message".to_string());
                        } else if message.chars().count() > 160 {
                            self.warning_message =
                                Some("Max 160 chars for 7-bit encoding".to_string());
                        } else if !self.connected {
                            self.log("Not connected", "error");
                        } else {
                            self.send_sms(
                                phones.into_iter().filter(|p| !p.is_empty()).collect(),
                                message.trim().to_string(),
                                self.sms_class.to_string(),
                                self.delivery_report,
                            );
                        }
                    }
                    if ui.button("Invisible Ping").clicked() {
                        if self.connected {
                            self.invisible_ping();
                        } else {
                            self.log("Not connected", "error");
                        }
                    }
                    if ui.button("Clear Fields").clicked() {
                        self.clear_fields();
                    }
                });
            });

        ui.add_space(8.0);

        egui::Frame::group(ui.style())
            .fill(Color32::from_rgb(10, 14, 20))
            .stroke(Stroke::new(1.0, Color32::from_rgb(68, 71, 90)))
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Terminal Log")
                            .strong()
                            .color(Color32::from_rgb(80, 250, 123)),
                    );
                    ui.separator();
                    ui.label("View:");
                    ui.radio_value(&mut self.log_mode, "at".to_string(), "AT");
                    ui.radio_value(&mut self.log_mode, "system".to_string(), "System");
                    ui.radio_value(&mut self.log_mode, "important".to_string(), "Important");
                    ui.radio_value(&mut self.log_mode, "all".to_string(), "All");
                    ui.radio_value(&mut self.log_mode, "raw".to_string(), "RAW");
                    let pause_label = if self.log_paused { "▶ Resume" } else { "⏸ Pause" };
                    if ui.button(pause_label).clicked() {
                        self.log_paused = !self.log_paused;
                    }
                    if ui.button("Clear Log").clicked() {
                        self.clear_log();
                    }
                });
                let log_height = ui.available_height().max(220.0);
                if self.log_paused {
                    ui.colored_label(Color32::from_rgb(255, 165, 0), "⏸ LOG PAUSED — entries are not being recorded");
                }
                self.rebuild_log_cache();
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), log_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .stick_to_bottom(true)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.set_min_height(log_height);
                                for entry in self.filtered_log_entries() {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            RichText::new(format!("[{}]", entry.timestamp))
                                                .monospace()
                                                .color(Color32::from_rgb(98, 114, 164)),
                                        );
                                        ui.label(
                                            RichText::new(format!(
                                                "{:<3}",
                                                category_label(&entry.category)
                                            ))
                                            .monospace()
                                            .strong()
                                            .color(log_color(&entry.category)),
                                        );
                                        ui.label(
                                            RichText::new(&entry.message)
                                                .monospace()
                                                .color(log_color(&entry.category)),
                                        );
                                    });
                                }
                            });
                    },
                );
            });
    }
}
