use egui::{self};

impl crate::ModAtApp {
    pub(crate) fn render_phonebook_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Add Contact").clicked() {
                self.contact_name_input.clear();
                self.contact_number_input.clear();
                self.show_add_contact = true;
            }
            if ui.button("Refresh").clicked() {
                self.load_phonebook_local();
            }
        });

        let mut remove_idx = None;
        let mut use_contact = None;
        egui::ScrollArea::vertical()
            .max_height(400.0)
            .show(ui, |ui| {
                for (idx, contact) in self.phonebook_data.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(
                                self.phonebook_selected == Some(idx),
                                format!("{} - {}", contact.name, contact.number),
                            )
                            .clicked()
                        {
                            self.phonebook_selected = Some(idx);
                        }
                        if ui.button("Use").clicked() {
                            use_contact = Some(contact.clone());
                        }
                        if ui.button("Delete").clicked() {
                            remove_idx = Some(idx);
                        }
                    });
                }
            });

        if let Some(contact) = use_contact {
            self.append_contact_to_recipients(&contact);
        }

        if let Some(idx) = remove_idx {
            self.phonebook_data.remove(idx);
            self.save_phonebook_local();
            self.phonebook_selected = None;
        }
    }
}
