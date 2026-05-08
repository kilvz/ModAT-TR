use egui::{self, Color32, RichText, Stroke, TextEdit};

impl crate::ModAtApp {
    pub(crate) fn render_ussd_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(10.0, 8.0);
        egui::Frame::group(ui.style())
            .fill(Color32::from_rgb(24, 28, 36))
            .stroke(Stroke::new(1.0, Color32::from_rgb(55, 65, 81)))
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("USSD").heading().strong().color(Color32::from_rgb(139, 233, 253)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.ussd_active {
                            ui.colored_label(Color32::GREEN, "\u{25CF} Active");
                        } else {
                            ui.colored_label(Color32::GRAY, "\u{25CF} Idle");
                        }
                    });
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("History:");
                    if !self.ussd_history.is_empty() {
                        egui::ComboBox::from_id_salt("ussd_history_combo")
                            .selected_text(if let Some(last) = self.ussd_history.last() { last.as_str() } else { "" })
                            .show_ui(ui, |ui| {
                                for code in &self.ussd_history {
                                    if ui.selectable_label(false, code.as_str()).clicked() {
                                        self.ussd_input = code.clone();
                                    }
                                }
                            });
                    }
                });

        if ui.button(RichText::new(if self.ussd_bookmarks_open { "▼ Bookmarks" } else { "▶ Bookmarks" }).color(Color32::from_rgb(189, 147, 249))).clicked() {
            self.ussd_bookmarks_open = !self.ussd_bookmarks_open;
        }
        if self.ussd_bookmarks_open {
            ui.group(|ui| {
            ui.horizontal(|ui| {
                if ui.small_button("Edit").clicked() {
                    let _ = std::process::Command::new("notepad.exe")
                        .arg(self.ussd_bookmarks_file.to_string_lossy().to_string())
                        .spawn();
                }
                ui.label(RichText::new("ussd_bookmarks.json").small().color(Color32::from_rgb(139, 233, 253)));
                if ui.small_button("Reload").clicked() {
                    self.reload_ussd_bookmarks();
                }
            });
            ui.add_space(4.0);
            for group in self.ussd_bookmarks.clone() {
                ui.label(RichText::new(&group.operator).strong());
                ui.horizontal_wrapped(|ui| {
                    for entry in &group.bookmarks {
                        if ui.button(format!("{} ({})", entry.name, entry.code)).clicked() {
                            self.send_ussd(&entry.code);
                            self.ussd_bookmarks_open = false;
                        }
                    }
                });
                ui.add_space(4.0);
            }
            });
        }

                ui.horizontal(|ui| {
                    ui.label("DCS:");
                    let dcs_opts = [("15", "GSM 7-bit"), ("none", "None"), ("72", "GSM 7-bit (alt)"), ("0", "Packed 7-bit"), ("68", "Data"), ("17", "UCS2"), ("24", "8-bit")];
                    egui::ComboBox::from_id_salt("ussd_dcs_combo")
                        .selected_text(self.ussd_dcs.to_string())
                        .show_ui(ui, |ui| {
                            for (val, name) in &dcs_opts {
                                ui.selectable_value(&mut self.ussd_dcs, val.to_string(), format!("{} - {}", val, name));
                            }
                        });
                    ui.checkbox(&mut self.ussd_plain_text, "Plain");
                });

                ui.horizontal(|ui| {
                    ui.add_sized(
                        [ui.available_width() - 220.0, 24.0],
                        TextEdit::singleline(&mut self.ussd_input).hint_text("*123#"),
                    );
                    if ui.button(RichText::new("Send USSD").strong()).clicked() {
                        let code = self.ussd_input.trim().to_string();
                        if !code.is_empty() {
                            self.send_ussd(&code);
                        }
                    }
                });

                if self.ussd_active && !self.ussd_buttons.is_empty() {
                    ui.separator();
                    ui.label(RichText::new("Reply:").strong().color(Color32::from_rgb(80, 250, 123)));
                    ui.horizontal_wrapped(|ui| {
                        let buttons = self.ussd_buttons.clone();
                        for (idx, text) in buttons.iter().enumerate() {
                            if ui.button(text.as_str()).clicked() {
                                self.reply_ussd(idx + 1);
                            }
                        }
                    });
                }

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Response:").strong().color(Color32::from_rgb(80, 250, 123)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.checkbox(&mut self.ussd_view_raw, "RAW");
                    });
                });
                egui::ScrollArea::vertical().max_height((ui.available_height() - 60.0).max(80.0)).auto_shrink([false, false]).show(ui, |ui| {
                    let display = if self.ussd_view_raw { &self.ussd_raw_response } else { &self.ussd_response };
                    if display.is_empty() {
                        ui.colored_label(Color32::from_rgb(98, 114, 164), "Response will appear here...");
                    } else {
                        ui.monospace(display);
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Console:").strong().color(Color32::from_rgb(189, 147, 249)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Clear").clicked() {
                            self.ussd_console.clear();
                        }
                    });
                });
                egui::ScrollArea::vertical()
                    .max_height(ui.available_height().max(100.0) - 40.0)
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.ussd_console.is_empty() {
                            ui.colored_label(Color32::from_rgb(98, 114, 164), "No USSD activity yet.");
                        } else {
                            ui.monospace(&self.ussd_console);
                        }
                    });

                ui.separator();
                if self.ussd_active
                    && ui.button(RichText::new("Cancel USSD").color(Color32::from_rgb(255, 100, 100))).clicked()
                {
                    self.cancel_ussd();
                }
            });
    }
}
