use chrono::Local;
use egui::{self, Color32, RichText, TextEdit};
use egui::Stroke;

impl crate::ModAtApp {
    pub(crate) fn render_scheduled_sms_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(10.0, 8.0);

        egui::Frame::group(ui.style())
            .fill(Color32::from_rgb(24, 28, 36))
            .stroke(Stroke::new(1.0, Color32::from_rgb(55, 65, 81)))
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Scheduled Messages")
                            .heading()
                            .strong()
                            .color(Color32::from_rgb(139, 233, 253)),
                    );
                    if ui.button(RichText::new("Add Schedule").strong()).clicked() {
                        self.show_add_schedule = true;
                        self.sched_date = Local::now().format("%Y-%m-%d").to_string();
                        self.sched_time = Local::now().format("%H:%M").to_string();
                        self.sched_repeat_input = "0".to_string();
                        self.sched_repeat_unit = 0;
                        self.sched_end_time.clear();
                        self.sched_flash_sms = false;
                    }
                });
                ui.separator();

                let mut to_delete: Option<u64> = None;
                let now = Local::now().format("%Y-%m-%d %H:%M").to_string();

                for entry in &self.scheduled_messages {
                    if !entry.sent && entry.scheduled_time < now {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!("#{}", entry.id))
                                    .monospace()
                                    .color(Color32::from_rgb(189, 147, 249)),
                            );
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            let recipients = entry.recipients.join(", ");
                            ui.label(
                                RichText::new(format!("To: {}", recipients))
                                    .color(Color32::from_rgb(229, 231, 235)),
                            );
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            let preview = if entry.message.chars().count() > 40 {
                                format!(
                                    "{}...",
                                    entry.message.chars().take(40).collect::<String>()
                                )
                            } else {
                                entry.message.clone()
                            };
                            ui.label(
                                RichText::new(preview)
                                    .color(Color32::from_rgb(156, 163, 175)),
                            );
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            let mut time_text = entry.scheduled_time.clone();
                            if let Some(mins) = entry.repeat_minutes {
                                time_text.push_str(&format!(" (every {}m)", mins));
                            }
                            ui.label(
                                RichText::new(&time_text)
                                    .monospace()
                                    .color(Color32::from_rgb(80, 250, 123)),
                            );
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            let (status_color, status_text) = if entry.sent {
                                (Color32::from_rgb(80, 250, 123), "Sent")
                            } else {
                                (Color32::from_rgb(255, 165, 0), "Pending")
                            };
                            ui.label(
                                RichText::new(status_text).color(status_color),
                            );
                        });
                        ui.separator();
                        if ui.button(RichText::new("Delete").color(Color32::from_rgb(255, 85, 85))).clicked() {
                            to_delete = Some(entry.id);
                        }
                    });
                }

                if let Some(id) = to_delete {
                    self.scheduled_messages.retain(|e| e.id != id);
                    self.save_scheduled();
                }

                if self.scheduled_messages.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("No scheduled messages.")
                            .color(Color32::from_rgb(156, 163, 175)),
                    );
                }
            });

        if self.show_add_schedule {
            ui.add_space(8.0);
            egui::Frame::group(ui.style())
                .fill(Color32::from_rgb(31, 41, 55))
                .stroke(Stroke::new(1.0, Color32::from_rgb(67, 56, 202)))
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("New Schedule")
                            .strong()
                            .color(Color32::from_rgb(139, 233, 253)),
                    );
                    ui.separator();

                    ui.label(RichText::new("Recipients").strong());

                    // Recipient tags
                    if !self.sched_recipient_list.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            let mut to_remove: Option<usize> = None;
                            for (i, recipient) in self.sched_recipient_list.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(recipient)
                                            .color(Color32::from_rgb(80, 250, 123))
                                            .background_color(Color32::from_rgb(31, 41, 55)),
                                    );
                                    if ui.small_button("✕").clicked() {
                                        to_remove = Some(i);
                                    }
                                });
                            }
                            if let Some(i) = to_remove {
                                self.sched_recipient_list.remove(i);
                            }
                        });
                        ui.add_space(4.0);
                    }

                    // Text input for new recipient
                    let enter_pressed = ui.horizontal(|ui| {
                        let resp = ui.add(
                            TextEdit::singleline(&mut self.sched_recipient_input)
                                .hint_text("Type a number and press Enter")
                                .desired_width(f32::INFINITY),
                        );
                        let pressed = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if pressed {
                            resp.request_focus();
                        }
                        pressed
                    });

                    if enter_pressed.inner {
                        let trimmed = self.sched_recipient_input.trim().to_string();
                        if !trimmed.is_empty() && !self.sched_recipient_list.contains(&trimmed) {
                            self.sched_recipient_list.push(trimmed);
                        }
                        self.sched_recipient_input.clear();
                    }

                    // Add from Phonebook button
                    ui.horizontal(|ui| {
                        if ui.button("Add from Phonebook").clicked() {
                            self.sched_show_phonebook_dropdown = !self.sched_show_phonebook_dropdown;
                        }
                    });

                    if self.sched_show_phonebook_dropdown {
                        egui::Frame::group(ui.style())
                            .fill(Color32::from_rgb(24, 28, 36))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(55, 65, 81)))
                            .inner_margin(4.0)
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(150.0)
                                    .show(ui, |ui| {
                                        if self.phonebook_data.is_empty() {
                                            ui.label(
                                                RichText::new("Phonebook is empty")
                                                    .color(Color32::from_rgb(156, 163, 175)),
                                            );
                                        }
                                        for contact in &self.phonebook_data {
                                            let label = if contact.name.is_empty() {
                                                contact.number.clone()
                                            } else {
                                                format!("{} — {}", contact.name, contact.number)
                                            };
                                            if ui.button(RichText::new(label).color(Color32::from_rgb(229, 231, 235))).clicked() {
                                                let num = contact.number.trim().to_string();
                                                if !num.is_empty() && !self.sched_recipient_list.contains(&num) {
                                                    self.sched_recipient_list.push(num);
                                                }
                                                self.sched_show_phonebook_dropdown = false;
                                            }
                                        }
                                    });
                            });
                    }
                    ui.add_space(4.0);

                    ui.label(RichText::new("Message").strong());
                    ui.add(
                        TextEdit::multiline(&mut self.sched_message)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(4.0);

                    ui.label(RichText::new("Date (YYYY-MM-DD)").strong());
                    ui.horizontal(|ui| {
                        ui.add(TextEdit::singleline(&mut self.sched_date).desired_width(120.0));
                        if ui.button("Today").clicked() {
                            self.sched_date = Local::now().format("%Y-%m-%d").to_string();
                        }
                    });
                    ui.add_space(4.0);

                    ui.label(RichText::new("Time (HH:MM)").strong());
                    ui.horizontal(|ui| {
                        ui.add(TextEdit::singleline(&mut self.sched_time).desired_width(80.0));
                        if ui.button("Now").clicked() {
                            self.sched_time = Local::now().format("%H:%M").to_string();
                        }
                    });
                    ui.add_space(4.0);

                    ui.label(RichText::new("Repeat (0 = once)").strong());
                    ui.horizontal(|ui| {
                        ui.add(TextEdit::singleline(&mut self.sched_repeat_input).desired_width(60.0));
                        ui.label("per");
                        ui.radio_value(&mut self.sched_repeat_unit, 0u8, "min").clicked();
                        ui.radio_value(&mut self.sched_repeat_unit, 1u8, "hr").clicked();
                        ui.radio_value(&mut self.sched_repeat_unit, 2u8, "day").clicked();
                    });
                    ui.add_space(4.0);

                    if self.sched_repeat_input.parse::<u32>().unwrap_or(0) > 0 {
                        ui.label(RichText::new("End date/time (YYYY-MM-DD HH:MM, optional)").strong());
                        ui.horizontal(|ui| {
                            ui.add(TextEdit::singleline(&mut self.sched_end_time).desired_width(200.0));
                            if ui.button("Clear").clicked() {
                                self.sched_end_time.clear();
                            }
                        });
                        ui.add_space(4.0);
                    }

                    ui.checkbox(&mut self.sched_flash_sms, "Flash SMS (Class 0)");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("Save").strong()).clicked() {
                            let recipients = self.sched_recipient_list.clone();

                            if recipients.is_empty() {
                                self.warning_message = Some("Enter at least one recipient".to_string());
                            } else if self.sched_message.trim().is_empty() {
                                self.warning_message = Some("Enter a message".to_string());
                            } else if self.sched_date.is_empty() || self.sched_time.is_empty() {
                                self.warning_message = Some("Enter date and time".to_string());
                            } else {
                                let scheduled_time = format!(
                                    "{} {}",
                                    self.sched_date.trim(),
                                    self.sched_time.trim()
                                );
                                let base_minutes: Option<u32> = {
                                    let parsed = self.sched_repeat_input.trim().parse::<u32>().unwrap_or(0);
                                    if parsed > 0 {
                                        let multiplier = match self.sched_repeat_unit {
                                            1 => 60,     // hours
                                            2 => 1440,   // days
                                            _ => 1,      // minutes
                                        };
                                        Some(parsed * multiplier)
                                    } else {
                                        None
                                    }
                                };
                                let end_time = if base_minutes.is_some() && !self.sched_end_time.trim().is_empty() {
                                    Some(self.sched_end_time.trim().to_string())
                                } else {
                                    None
                                };
                                self.scheduled_messages.push(crate::scheduled::ScheduledSms {
                                    id: self.next_schedule_id,
                                    recipients,
                                    message: self.sched_message.trim().to_string(),
                                    scheduled_time,
                                    sent: false,
                                    repeat_minutes: base_minutes,
                                    end_time,
                                    flash_sms: self.sched_flash_sms,
                                });
                                self.next_schedule_id = self.next_schedule_id.wrapping_add(1);
                                self.save_scheduled();
                                self.sched_recipient_list.clear();
                                self.sched_recipient_input.clear();
                                self.sched_show_phonebook_dropdown = false;
                                self.sched_message.clear();
                                self.sched_date.clear();
                                self.sched_time.clear();
                                self.sched_repeat_input.clear();
                                self.sched_repeat_unit = 0;
                                self.sched_end_time.clear();
                                self.sched_flash_sms = false;
                                self.show_add_schedule = false;
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_add_schedule = false;
                            self.sched_recipient_list.clear();
                            self.sched_recipient_input.clear();
                            self.sched_show_phonebook_dropdown = false;
                            self.sched_message.clear();
                            self.sched_date.clear();
                            self.sched_time.clear();
                            self.sched_repeat_input.clear();
                            self.sched_repeat_unit = 0;
                            self.sched_end_time.clear();
                            self.sched_flash_sms = false;
                        }
                    });
                });
        }
    }
}
