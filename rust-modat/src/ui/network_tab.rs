use egui::{self, RichText};

impl crate::ModAtApp {
    pub(crate) fn render_network_info_tab(&mut self, ui: &mut egui::Ui) {
        if ui.button("Refresh Values").clicked() {
            if self.connected {
                self.log("Refreshing network info...", "info");
                self.get_modem_info_async();
            } else {
                self.warning_message = Some("Connect first".to_string());
            }
        }
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Signal Strength:");
            ui.colored_label(self.get_signal_color(), &self.signal);
        });

        ui.label(format!("Operator: {}", self.operator));
        ui.label(format!("Network: {}", self.network));
        ui.separator();
        ui.label(format!("Network Registration: {}", self.net_reg));
        ui.label(format!("TAC/LAC: {}", self.tac_lac));
        ui.label(format!("Cell ID: {}", self.cell_id));
        ui.label(format!("Technology: {}", self.net_tech));
        ui.separator();
        ui.label(format!("Band: {}", self.cell_band));
        ui.label(format!("DL EARFCN: {}", self.dl_earfcn));
        ui.label(format!("DL Freq: {}", self.dl_freq));
        ui.label(format!("DL BW: {}", self.dl_bw));
        ui.label(format!("UL EARFCN: {}", self.ul_earfcn));
        ui.label(format!("UL Freq: {}", self.ul_freq));
        ui.label(format!("UL BW: {}", self.ul_bw));
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("RSSI:");
            ui.colored_label(self.get_value_color(&self.rssi, (-70.0, -90.0), true), &self.rssi);
            ui.label("dBm");
        });
        ui.horizontal(|ui| {
            ui.label("RSRP:");
            ui.colored_label(self.get_value_color(&self.rsrp, (-80.0, -100.0), true), &self.rsrp);
            ui.label("dBm");
        });
        ui.horizontal(|ui| {
            ui.label("SINR:");
            ui.colored_label(self.get_value_color(&self.sinr, (15.0, 5.0), true), &self.sinr);
            ui.label("dB");
        });
        ui.horizontal(|ui| {
            ui.label("RSRQ:");
            ui.colored_label(self.get_value_color(&self.rsrq, (-10.0, -15.0), true), &self.rsrq);
            ui.label("dB");
        });

        ui.add_space(10.0);
        ui.separator();
        ui.collapsing("📶 Signal Quality Guide", |ui| {
            ui.label(RichText::new("RSRP (Reference Signal Received Power)").strong());
            ui.label("• Excellent: >= -80 dBm\n• Good: -80 to -90 dBm\n• Mid: -90 to -100 dBm\n• Poor: < -100 dBm");
            ui.add_space(5.0);

            ui.label(RichText::new("RSRQ (Reference Signal Received Quality)").strong());
            ui.label("• Excellent: >= -10 dB\n• Good: -10 to -15 dB\n• Mid: -15 to -20 dB\n• Poor: < -20 dB");
            ui.add_space(5.0);

            ui.label(RichText::new("SINR (Signal to Interference plus Noise Ratio)").strong());
            ui.label("• Excellent: >= 20 dB\n• Good: 13 to 20 dB\n• Mid: 0 to 13 dB\n• Poor: < 0 dB");
            ui.add_space(5.0);

            ui.label(RichText::new("RSSI (Received Signal Strength Indicator)").strong());
            ui.label("• Excellent: >= -65 dBm\n• Good: -65 to -75 dBm\n• Mid: -75 to -85 dBm\n• Poor: < -85 dBm");
        });
    }
}
