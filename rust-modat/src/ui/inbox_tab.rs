use egui::{self, RichText};

impl crate::ModAtApp {
    pub(crate) fn render_inbox_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Load Inbox").clicked() {
                if self.connected {
                    self.load_inbox();
                } else {
                    self.warning_message = Some("Connect first".to_string());
                }
            }
            if ui.button("Reply").clicked() {
                self.reply_message();
            }
            if ui.button("Forward").clicked() {
                self.forward_message();
            }
            if ui.button("Delete").clicked() {
                self.delete_message();
            }
            if ui.button("Clear All SMS (Modem)").clicked() {
                self.clear_modem_sms();
            }
        });

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("View:");
            ui.radio_value(&mut self.inbox_view_mode, "simple".to_string(), "Simple");
            ui.radio_value(&mut self.inbox_view_mode, "decoded".to_string(), "Decoded");
            ui.radio_value(&mut self.inbox_view_mode, "raw".to_string(), "Raw PDU");
        });

        let mut selected_idx = self.inbox_selected;
        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                for (idx, label) in &self.inbox_display_items {
                    if ui
                        .selectable_label(self.inbox_selected == Some(*idx), label)
                        .clicked()
                    {
                        selected_idx = Some(*idx);
                    }
                }
            });
        self.inbox_selected = selected_idx;

        if let Some(idx) = self.inbox_selected {
            if let Some(msg) = self.inbox_messages.get(idx) {
                self.current_inbox_msg = Some(msg.clone());
            }
        }

        let mut mark_read_idx = None;
        if let Some(ref msg) = self.current_inbox_msg {
            let msg_index = msg.index;
            let msg_unread = msg.unread;
            let phone = if msg.phone.is_empty() {
                let (p, ..) = self.decode_sms_simple(&msg.pdu);
                p
            } else {
                msg.phone.clone()
            };
            let timestamp = if msg.timestamp.is_empty() {
                let (_, t, ..) = self.decode_sms_simple(&msg.pdu);
                t
            } else {
                msg.timestamp.clone()
            };
            let message = if let Some(ref pre) = msg.pre_decoded {
                pre.clone()
            } else {
                let (_, _, m, _) = self.decode_sms_simple(&msg.pdu);
                m
            };
            let dcs = if msg.pre_decoded.is_some() {
                0
            } else {
                let (_, _, _, d) = self.decode_sms_simple(&msg.pdu);
                d
            };

            ui.group(|ui| {
                match self.inbox_view_mode.as_str() {
                    "raw" => {
                        ui.label(RichText::new("PDU hex:").strong());
                        ui.monospace(&msg.pdu);
                    }
                    "decoded" => {
                        ui.label(format!("From: {}", phone));
                        ui.label(format!("DCS: 0x{:02X}", dcs));
                        ui.label(format!("Time: {}", timestamp));
                        ui.separator();
                        ui.label("Message:");
                        ui.monospace(&message);
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("From:").strong());
                            ui.label(&phone);
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Time:").strong());
                            ui.label(&timestamp);
                        });
                        ui.separator();
                        ui.label(RichText::new("Message:").strong());
                        egui::ScrollArea::vertical()
                            .max_height(150.0)
                            .show(ui, |ui| {
                                ui.monospace(&message);
                            });
                    }
                }

                if msg_unread && ui.button("Mark as Read").clicked() {
                    mark_read_idx = Some(msg_index);
                }
            });
        }

        if let Some(idx) = mark_read_idx {
            for msg in &mut self.inbox_messages {
                if msg.index == idx {
                    msg.unread = false;
                }
            }
            self.save_inbox_file();
            self.log(&format!("Marked message {} as read locally", idx), "system");
        }
    }
}
