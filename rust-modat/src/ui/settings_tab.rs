use egui::{self, Color32};

impl crate::ModAtApp {
    pub(crate) fn render_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.label("Serial Port");
            ui.horizontal(|ui| {
                ui.label("COM Port:");
                egui::ComboBox::from_id_salt("manual_com_port_combo")
                    .selected_text(&self.manual_port)
                    .show_ui(ui, |ui| {
                        for port in &self.com_ports {
                            if ui
                                .selectable_label(self.manual_port == *port, port)
                                .clicked()
                            {
                                self.manual_port = port.clone();
                            }
                        }
                    });
                let connect_label = if self.connected {
                    "Disconnect"
                } else {
                    "Connect Directly"
                };
                if ui.button(connect_label).clicked() {
                    if self.connected {
                        self.disconnect();
                        self.detect_modem_mode_async();
                    } else {
                        let port = self.get_manual_port_device();
                        let baud = self.manual_baud.parse::<u32>().unwrap_or(9600);
                        if port.is_empty() {
                            self.log("No COM port selected in Settings", "error");
                        } else {
                            self.log(
                                &format!("Connecting directly to {} at {} baud...", port, baud),
                                "system",
                            );
                            self.status_text = "Connecting...".to_string();
                            self.status_color = Color32::from_rgb(255, 165, 0);
                            if self.connect_serial(&port, baud) {
                                self.current_tab = 0;
                            }
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Baud Rate:");
                egui::ComboBox::from_id_salt("manual_baud_combo")
                    .selected_text(&self.manual_baud)
                    .show_ui(ui, |ui| {
                        for baud in &["9600", "19200", "38400", "57600", "115200"] {
                            if ui
                                .selectable_label(self.manual_baud == *baud, *baud)
                                .clicked()
                            {
                                self.manual_baud = baud.to_string();
                            }
                        }
                    });
            });
            ui.checkbox(&mut self.manual_bypass, "Bypass Auto-detect");
        });

        ui.group(|ui| {
            ui.label("SMS Settings");
            ui.horizontal(|ui| {
                ui.label("Default Phone:");
                ui.text_edit_singleline(&mut self.phone_number);
            });
            ui.horizontal(|ui| {
                ui.label("Log Mode:");
                egui::ComboBox::from_id_salt("settings_log_mode_combo")
                    .selected_text(&self.log_mode)
                    .show_ui(ui, |ui| {
                        for mode in &["readable", "important", "all", "raw"] {
                            ui.selectable_value(&mut self.log_mode, mode.to_string(), *mode);
                        }
                    });
            });
        });

        ui.group(|ui| {
            ui.label("Network Settings");
            ui.horizontal(|ui| {
                ui.label("Modem IP:");
                ui.text_edit_singleline(&mut self.modem_ip);
                if ui.button("Auto-Detect IP").clicked() {
                    self.autodetect_modem_ip();
                }
            });
            ui.horizontal(|ui| {
                ui.label("Username:");
                ui.text_edit_singleline(&mut self.modem_user);
            });
            ui.horizontal(|ui| {
                ui.label("Password:");
                ui.add(egui::TextEdit::singleline(&mut self.modem_pass).password(true));
            });
            ui.horizontal(|ui| {
                ui.label("Switch Mode:");
                egui::ComboBox::from_id_salt("settings_switch_mode_combo")
                    .selected_text(&self.switch_mode)
                    .show_ui(ui, |ui| {
                        for mode in &["Project Mode", "Debug Mode"] {
                            ui.selectable_value(&mut self.switch_mode, mode.to_string(), *mode);
                        }
                    });
            });
            ui.label(format!("Detected Serial Profile: {}", self.serial_profile_label()));
        });

        if ui.button("Save Settings").clicked() {
            self.save_settings();
        }
    }
}
