use chrono::Local;
use egui::{self, Color32, RichText};

use crate::ui::helpers::at_commands;

impl crate::ModAtApp {
    pub(crate) fn render_at_terminal_tab(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Image::new(egui::include_image!("../icon.png")).max_width(48.0).corner_radius(5.0));
                ui.vertical(|ui| {
                    ui.label(RichText::new("AT Command Terminal Guide").strong().underline());
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("Tab:").strong());
                        ui.label("Autocomplete command  ");
                        ui.label(RichText::new("Up/Down:").strong());
                        ui.label("Navigate history  ");
                        ui.label(RichText::new("Enter:").strong());
                        ui.label("Send command");
                    });
                });
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Prefixes:").strong());
                ui.label(RichText::new(" Universal (3GPP)").color(Color32::from_rgb(100, 200, 255)));
                ui.label(" | ");
                ui.label(RichText::new("^ Huawei").color(Color32::from_rgb(255, 100, 100)));
                ui.label(" | ");
                ui.label(RichText::new("+Q Qualcomm").color(Color32::from_rgb(100, 255, 100)));
                ui.label(" | ");
                ui.label(RichText::new("+E MediaTek").color(Color32::from_rgb(255, 200, 100)));
            });
        });
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Command:");
            let response = ui.text_edit_singleline(&mut self.at_command);

            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                if self.at_hist_idx < (self.at_history.len() as i32) - 1 {
                    self.at_hist_idx += 1;
                    if let Some(cmd) = self
                        .at_history
                        .get((self.at_history.len() as i32 - 1 - self.at_hist_idx) as usize)
                    {
                        self.at_command = cmd.clone();
                    }
                }
                ui.memory_mut(|mem| mem.request_focus(response.id));
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                if self.at_hist_idx > 0 {
                    self.at_hist_idx -= 1;
                    if let Some(cmd) = self
                        .at_history
                        .get((self.at_history.len() as i32 - 1 - self.at_hist_idx) as usize)
                    {
                        self.at_command = cmd.clone();
                    }
                } else {
                    self.at_command.clear();
                }
                ui.memory_mut(|mem| mem.request_focus(response.id));
            }

            if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                let input_upper = self.at_command.to_uppercase();
                for (cmd, _) in at_commands() {
                    if cmd.to_uppercase().starts_with(&input_upper) {
                        self.at_command = cmd.to_string();
                        break;
                    }
                }
            }

            if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.send_at_command();
                ui.memory_mut(|mem| mem.request_focus(response.id));
            }

            if ui.button("Send").clicked() {
                self.send_at_command();
            }
            if ui.button("Clear").clicked() {
                self.at_output.clear();
            }
        });

        ui.separator();
        let typed = self.at_command.to_uppercase();
        if typed.len() >= 2 {
            egui::CollapsingHeader::new("Autocomplete")
                .default_open(true)
                .show(ui, |ui| {
                    for (cmd, desc) in at_commands()
                        .into_iter()
                        .filter(|(cmd, _)| cmd.to_uppercase().starts_with(&typed))
                        .take(8)
                    {
                        if ui.button(format!("{:<30} {}", cmd, desc)).clicked() {
                            self.at_command = cmd.to_string();
                        }
                    }
                });
        }

        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(400.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.monospace(&self.at_output);
            });
    }

    pub(crate) fn send_at_command(&mut self) {
        if !self.at_command.is_empty() {
            let cmd = self.at_command.clone();
            self.at_history.push(cmd.clone());
            if self.at_history.len() > 100 {
                self.at_history.remove(0);
            }
            self.at_hist_idx = -1;
            let ts = Local::now().format("%H:%M:%S").to_string();
            self.at_output
                .push_str(&format!("\n[{}] >>> {}\n", ts, cmd));
            if self.connected && self.serial_tx.is_some() {
                let resp = self.send_at_multi(&cmd, 8);
                if resp.is_empty() {
                    self.at_output.push_str("  (no response)\n");
                } else {
                    self.at_output.push_str(&resp);
                    self.at_output.push('\n');
                }
            } else {
                self.at_output
                    .push_str("  [NOT CONNECTED - cannot send AT command]\n");
            }
            self.at_command.clear();
        }
    }
}
