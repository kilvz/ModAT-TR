use crate::{Contact, InboxMessage, LogEntry};
use chrono::Local;
use std::io;
use std::path::PathBuf;

pub(crate) fn atomic_write(path: &PathBuf, content: &str) -> io::Result<()> {
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, content)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    std::fs::rename(tmp_path, path)
}

impl crate::ModAtApp {
    // ─── Logging ───
    pub(crate) fn log(&mut self, msg: &str, category: &str) {
        if self.log_paused {
            return;
        }
        self.log_cache_dirty = true;
        let ts = Local::now().format("%H:%M:%S").to_string();
        let entry = LogEntry {
            timestamp: ts,
            category: category.to_string(),
            message: msg.to_string(),
        };

        if category == "raw" {
            if self.raw_log_entries.len() >= self.max_raw_log_entries {
                self.raw_log_entries.pop_front();
            }
            self.raw_log_entries.push_back(entry);

            if let Some(readable) = self.readable_raw(msg) {
                let r_entry = LogEntry {
                    timestamp: Local::now().format("%H:%M:%S").to_string(),
                    category: "at".to_string(),
                    message: readable,
                };
                if self.important_log_entries.len() >= self.max_important_log_entries {
                    self.important_log_entries.pop_front();
                }
                self.important_log_entries.push_back(r_entry);
            }
        } else {
            if self.important_log_entries.len() >= self.max_important_log_entries {
                self.important_log_entries.pop_front();
            }
            self.important_log_entries.push_back(entry);
        }
    }

    pub(crate) fn clear_log(&mut self) {
        self.raw_log_entries.clear();
        self.important_log_entries.clear();
        self.log_cache_dirty = true;
    }

    pub(crate) fn rebuild_log_cache(&mut self) {
        if !self.log_cache_dirty {
            return;
        }
        let mode = self.log_mode.clone();
        let mut filtered: Vec<LogEntry> = Vec::new();
        for e in self.raw_log_entries.iter().chain(self.important_log_entries.iter()) {
            let keep = match mode.as_str() {
                "at" => e.category == "at",
                "raw" => e.category == "raw",
                "system" => matches!(e.category.as_str(), "system" | "sms" | "error"),
                "important" => matches!(e.category.as_str(), "sms" | "error"),
                _ => true,
            };
            if keep {
                filtered.push(e.clone());
            }
        }
        self.cached_filtered_log = filtered;
        self.log_cache_dirty = false;
    }

    pub(crate) fn filtered_log_entries(&self) -> &[LogEntry] {
        &self.cached_filtered_log
    }

    // ─── Settings ───
    pub(crate) fn save_settings(&mut self) {
        self.log("Saving settings...", "system");
        self.cfg.serial.manual_port = self.manual_port.clone();
        self.cfg.serial.baud = self.manual_baud.clone();
        self.cfg.serial.bypass_autodetect = self.manual_bypass.to_string();
        self.cfg.sms.phone = self.phone_number.clone();
        self.cfg.sms.sms_class = self.sms_class_options[self.sms_class].clone();
        self.cfg.sms.dcs = self.dcs_value.clone();
        self.cfg.sms.delivery_report = self.delivery_report.to_string();
        self.cfg.sms.log_mode = self.log_mode.clone();
        self.cfg.network.modem_ip = self.modem_ip.clone();
        self.cfg.network.username = self.modem_user.clone();
        self.cfg.network.password = crate::protect_secret(&self.modem_pass);
        self.cfg.network.switch_mode = self.switch_mode.clone();

        match serde_ini::to_string(&self.cfg) {
            Ok(content) => {
                if let Err(e) = atomic_write(&self.settings_file, &content) {
                    self.log(&format!("Failed to save settings: {}", e), "error");
                } else {
                    self.log("Settings saved successfully", "system");
                    self.warning_message = Some("Settings saved successfully.".to_string());
                }
            }
            Err(e) => self.log(&format!("Failed to save settings: {}", e), "error"),
        }
    }

    // ─── Inbox file ───
    pub(crate) fn load_inbox_file(&mut self) {
        self.log("Loading inbox from disk...", "system");
        if !self.inbox_file.exists() {
            self.inbox_messages = Vec::new();
            self.log("inbox.json not found; creating empty inbox.json", "system");
            self.save_inbox_file();
            return;
        }
        match std::fs::read_to_string(&self.inbox_file) {
            Ok(content) => match serde_json::from_str::<Vec<InboxMessage>>(&content) {
                Ok(data) => {
                    self.inbox_messages = data;
                    self.log(
                        &format!(
                            "Loaded {} saved messages from inbox file",
                            self.inbox_messages.len()
                        ),
                        "system",
                    );
                }
                Err(e) => {
                    self.inbox_messages = Vec::new();
                    self.log(&format!("Failed to parse inbox.json: {}", e), "error");
                }
            },
            Err(e) => self.log(&format!("Failed to load inbox.json: {}", e), "error"),
        }
        self.rebuild_inbox_display_items();
    }

    pub(crate) fn save_inbox_file(&mut self) {
        self.rebuild_inbox_display_items();
        match serde_json::to_string_pretty(&self.inbox_messages) {
            Ok(content) => {
                if let Err(e) = atomic_write(&self.inbox_file, &content) {
                    self.log(&format!("Failed to save inbox.json: {}", e), "error");
                } else {
                    self.log(&format!("Inbox saved ({} messages)", self.inbox_messages.len()), "system");
                }
            }
            Err(e) => self.log(&format!("Failed to serialize inbox.json: {}", e), "error"),
        }
    }

    pub(crate) fn rebuild_inbox_display_items(&mut self) {
        let items = self
            .inbox_messages
            .iter()
            .enumerate()
            .map(|(idx, msg)| {
                let phone = if msg.phone.is_empty() {
                    let (phone, _, _, _) = self.decode_sms_simple(&msg.pdu);
                    phone
                } else {
                    msg.phone.clone()
                };
                let timestamp = if msg.timestamp.is_empty() {
                    let (_, timestamp, _, _) = self.decode_sms_simple(&msg.pdu);
                    timestamp
                } else {
                    msg.timestamp.clone()
                };
                let ts = if timestamp.len() >= 16 {
                    &timestamp[..16]
                } else {
                    &timestamp
                };
                let status = if msg.unread { "📩" } else { "📨" };
                (idx, format!("{} {} - {}", status, phone, ts))
            })
            .collect();
        self.inbox_display_items = items;
    }

    // ─── Phonebook ───
    pub(crate) fn load_phonebook_local(&mut self) {
        if !self.contacts_file.exists() {
            self.phonebook_data = Vec::new();
            self.log("contacts.json not found; creating empty contacts.json", "system");
            self.save_phonebook_local();
            return;
        }

        match std::fs::read_to_string(&self.contacts_file) {
            Ok(content) => {
                match serde_json::from_str::<Vec<Contact>>(&content) {
                    Ok(data) => {
                        self.phonebook_data = data;
                        self.log(
                            &format!("Loaded {} contacts from contacts.json", self.phonebook_data.len()),
                            "system",
                        );
                    }
                    Err(e) => {
                        self.phonebook_data = Vec::new();
                        self.log(&format!("Failed to parse contacts.json: {}", e), "error");
                    }
                }
            }
            Err(e) => self.log(&format!("Failed to load contacts.json: {}", e), "error"),
        }
    }

    pub(crate) fn save_phonebook_local(&mut self) {
        match serde_json::to_string_pretty(&self.phonebook_data) {
            Ok(content) => {
                if let Err(e) = atomic_write(&self.contacts_file, &content) {
                    self.log(&format!("Failed to save contacts.json: {}", e), "error");
                } else {
                    self.log(&format!("Phonebook saved ({} contacts)", self.phonebook_data.len()), "system");
                }
            }
            Err(e) => self.log(&format!("Failed to serialize contacts.json: {}", e), "error"),
        }
    }

    pub(crate) fn add_current_phone_as_contact(&mut self, name: String) {
        let phone = self
            .phone_number
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if name.trim().is_empty() {
            self.warning_message = Some("Please enter a name".to_string());
            return;
        }
        if phone.is_empty() {
            self.warning_message = Some("Enter a phone number first".to_string());
            return;
        }
        self.phonebook_data.push(Contact {
            name: name.trim().to_string(),
            number: phone.clone(),
        });
        self.save_phonebook_local();
        self.log(
            &format!("Added contact: {} ({})", name.trim(), phone),
            "info",
        );
    }

    pub(crate) fn add_manual_contact(&mut self, name: String, number: String) {
        if name.trim().is_empty() || number.trim().is_empty() {
            self.warning_message = Some("Please fill both fields".to_string());
            return;
        }
        self.phonebook_data.push(Contact {
            name: name.trim().to_string(),
            number: number.trim().to_string(),
        });
        self.save_phonebook_local();
        self.log(
            &format!("Added contact: {} ({})", name.trim(), number.trim()),
            "info",
        );
    }

    pub(crate) fn append_contact_to_recipients(&mut self, contact: &Contact) {
        let current = self.phone_number.trim();
        if !current.is_empty() {
            self.phone_number = format!("{},{}", current, contact.number);
        } else {
            self.phone_number = contact.number.clone();
        }
        self.log(
            &format!("Added {} ({}) to recipients", contact.name, contact.number),
            "info",
        );
        self.current_tab = 0;
    }

    pub(crate) fn resolve_contact_name(&self, phone: &str) -> String {
        let normalized = phone.trim();
        if normalized.is_empty() { return normalized.to_string(); }
        for contact in &self.phonebook_data {
            if contact.number.trim() == normalized {
                return format!("{} ({})", contact.name, normalized);
            }
        }
        normalized.to_string()
    }
}
