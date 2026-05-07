use chrono::Local;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScheduledSms {
    pub id: u64,
    pub recipients: Vec<String>,
    pub message: String,
    pub scheduled_time: String,
    pub sent: bool,
    pub repeat_minutes: Option<u32>,
    pub end_time: Option<String>,
    pub flash_sms: bool,
}

impl crate::ModAtApp {
    pub(crate) fn check_scheduled_sms(&mut self) {
        if !self.connected {
            return;
        }
        let now = Local::now().format("%Y-%m-%d %H:%M").to_string();
        let delivery_report = self.delivery_report;

        let mut to_send = Vec::new();
        for entry in &self.scheduled_messages {
            if !entry.sent && entry.scheduled_time <= now {
                to_send.push((entry.id, entry.recipients.clone(), entry.message.clone(), entry.repeat_minutes, entry.flash_sms));
            }
        }

        for (id, recipients, message, repeat_minutes, flash) in to_send {
            let sms_class = if flash { "0".to_string() } else { "1".to_string() };
            self.send_sms(recipients.clone(), message.clone(), sms_class, delivery_report);
            if let Some(entry) = self.scheduled_messages.iter_mut().find(|e| e.id == id) {
                if let Some(minutes) = repeat_minutes {
                    let next = Local::now() + chrono::Duration::minutes(minutes as i64);
                    let next_str = next.format("%Y-%m-%d %H:%M").to_string();
                    if let Some(ref end) = entry.end_time {
                        if next_str > *end {
                            entry.sent = true;
                        } else {
                            entry.scheduled_time = next_str;
                        }
                    } else {
                        entry.scheduled_time = next_str;
                    }
                } else {
                    entry.sent = true;
                }
            }
            self.log(&format!("Sent scheduled SMS #{}", id), "sms");
        }
        self.save_scheduled();
    }

    pub(crate) fn load_scheduled(&mut self) {
        if self.scheduled_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&self.scheduled_file) {
                if let Ok(entries) = serde_json::from_str::<Vec<ScheduledSms>>(&content) {
                    self.scheduled_messages = entries;
                    self.next_schedule_id = self
                        .scheduled_messages
                        .iter()
                        .map(|e| e.id)
                        .max()
                        .map_or(1, |id| id + 1);
                    self.log("Loaded scheduled messages", "system");
                }
            }
        }
    }

    pub(crate) fn save_scheduled(&mut self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.scheduled_messages) {
            let _ = std::fs::write(&self.scheduled_file, json);
        }
    }
}
