use crate::patterns::*;
use crate::{DeliveryRecord, InboxMessage, PendingSend, SentMessageInfo};
use chrono::Local;
use std::thread;
use std::time::Duration;

pub(crate) fn pack_7bit(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut buffer: u64 = 0;
    let mut bits_in_buffer: u32 = 0;
    for &byte in data {
        buffer |= ((byte & 0x7F) as u64) << bits_in_buffer;
        bits_in_buffer += 7;
        while bits_in_buffer >= 8 {
            result.push((buffer & 0xFF) as u8);
            buffer >>= 8;
            bits_in_buffer -= 8;
        }
    }
    if bits_in_buffer > 0 {
        result.push((buffer & 0xFF) as u8);
    }
    result
}

fn decode_hex(hex_str: &str) -> Vec<u8> {
    let hex_str = hex_str.trim();
    if hex_str.is_empty() {
        return Vec::new();
    }
    let hex_str = if !hex_str.len().is_multiple_of(2) {
        format!("0{}", hex_str)
    } else {
        hex_str.to_string()
    };
    (0..hex_str.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&hex_str[i..i + 2], 16).ok())
        .collect()
}

fn decode_cms_error(response: &str) -> Option<String> {
    let code = RE_CMS_ERROR
        .captures(response)?
        .get(1)?
        .as_str()
        .parse::<u32>()
        .ok()?;
    let desc = match code {
        1 => "Unassigned number",
        8 => "Operator barred",
        10 => "Call barred",
        21 => "Transfer rejected",
        27 => "Destination out of service",
        28 => "Unidentified subscriber",
        29 => "Facility rejected",
        30 => "Unknown subscriber",
        38 => "Network out of order",
        41 => "Temporary failure",
        42 => "Congestion",
        47 => "Resources unavailable",
        50 => "Facility not subscribed",
        69 => "Requested facility not implemented",
        81 => "Invalid SM transfer reference value",
        95 => "Invalid message",
        96 => "Invalid mandatory information",
        97 => "Message type non-existent",
        98 => "Message not compatible with SM protocol",
        99 => "Information element non-existent",
        111 => "Protocol error",
        127 => "Interworking error",
        128 => "Telematic interworking not supported",
        129 => "SMS type 0 not supported",
        130 => "Cannot replace short message",
        143 => "Unspecified TP-PID error",
        144 => "Data coding scheme not supported",
        159 => "Unspecified TP-DCS error",
        160 => "Command cannot be acted on",
        161 => "Command unsupported",
        175 => "Unspecified TP-command error",
        176 => "TPDU not supported",
        192 => "SC busy",
        193 => "No SC subscription",
        194 => "SC system failure",
        195 => "Invalid SME address",
        196 => "Destination SME barred",
        197 => "SM rejected - duplicate SM",
        198 => "TP-VPF not supported",
        199 => "TP-VP not supported",
        208 => "SIM SMS storage full",
        209 => "No SMS storage in SIM",
        210 => "Error in MS",
        211 => "Memory capacity exceeded",
        212 => "SIM application toolkit busy",
        213 => "SIM data download error",
        255 => "Unspecified error cause",
        300 => "ME failure",
        301 => "SMS service reserved",
        302 => "Operation not allowed",
        303 => "Operation not supported",
        304 => "Invalid PDU mode / message ref exhausted - wait and retry",
        305 => "Invalid text mode parameter",
        310 => "SIM not inserted",
        311 => "SIM PIN required",
        312 => "PH-SIM PIN required",
        313 => "SIM failure",
        314 => "SIM busy - try again",
        315 => "SIM wrong",
        316 => "SIM PUK required",
        320 => "Memory failure",
        321 => "Invalid memory index",
        322 => "Memory full",
        330 => "SMSC address unknown",
        331 => "No network service",
        332 => "Network timeout",
        340 => "No +CNMA expected",
        500 => "Unknown error",
        _ => "Unknown CMS error",
    };
    Some(format!("+CMS ERROR {}: {}", code, desc))
}

impl crate::ModAtApp {
    fn encode_phone(&self, phone: &str) -> String {
        let mut phone = phone.replace('+', "");
        if phone.len() % 2 == 1 {
            phone.push('F');
        }
        let mut encoded = String::new();
        let chars: Vec<char> = phone.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if i + 1 < chars.len() {
                encoded.push(chars[i + 1]);
                encoded.push(chars[i]);
            } else {
                encoded.push(chars[i]);
                encoded.push('F');
            }
            i += 2;
        }
        encoded
    }

    fn decode_phone_number(&self, data: &[u8], addr_type: u8) -> String {
        let mut digits = Vec::new();
        for &byte in data {
            let d1 = byte & 0x0F;
            let d2 = (byte >> 4) & 0x0F;
            digits.push(d1.to_string());
            if d2 != 0x0F {
                digits.push(d2.to_string());
            }
        }
        let mut phone = digits.join("");
        if addr_type & 0x70 == 0x10 {
            phone.insert(0, '+');
        }
        phone
    }

    fn decode_timestamp(&self, data: &[u8]) -> String {
        fn bcd(b: u8) -> u32 {
            ((b & 0x0F) as u32) * 10 + (((b >> 4) & 0x0F) as u32)
        }
        if data.len() < 7 {
            return "Unknown".to_string();
        }
        let year = bcd(data[0]);
        let month = bcd(data[1]);
        let day = bcd(data[2]);
        let hour = bcd(data[3]);
        let minute = bcd(data[4]);
        let second = bcd(data[5]);
        let tz = (data[6] & 0x0F) as i32 * 10 + ((data[6] >> 4) & 0x0F) as i32;
        let sign = if data[6] & 0x80 != 0 { '+' } else { '-' };
        format!(
            "20{:02}-{:02}-{:02} {:02}:{:02}:{:02} {}{}",
            year,
            month,
            day,
            hour,
            minute,
            second,
            sign,
            tz / 4
        )
    }

    fn decode_7bit(&self, data: &[u8], udl: usize) -> String {
        let mut result = Vec::new();
        let mut shift: u32 = 0;
        let mut current: u64 = 0;
        for &byte in data {
            current |= (byte as u64) << shift;
            shift += 8;
            while shift >= 7 && result.len() < udl {
                result.push((current & 0x7F) as u8);
                current >>= 7;
                shift -= 7;
            }
        }
        if shift > 0 && result.len() < udl {
            result.push((current & 0x7F) as u8);
        }
        result.iter().map(|&c| c as char).collect()
    }

    fn decode_ucs2(&self, data: &[u8], _udl: usize) -> String {
        String::from_utf16(
            &data
                .chunks_exact(2)
                .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                .collect::<Vec<u16>>(),
        )
        .unwrap_or_else(|_| "(binary)".to_string())
    }

    pub(crate) fn decode_sms_simple(&self, pdu_hex: &str) -> (String, String, String, u8) {
        let pdu_bytes = decode_hex(pdu_hex);
        if pdu_bytes.is_empty() {
            return (String::new(), String::new(), String::new(), 0);
        }
        let mut offset = 0;

        // Check for SMSC
        if pdu_bytes[0] > 0
            && pdu_bytes[0] < 20
            && offset + pdu_bytes[0] as usize + 1 < pdu_bytes.len()
            && matches!(pdu_bytes[1], 0x91 | 0x81 | 0x80 | 0xA1)
        {
            offset += 1 + pdu_bytes[0] as usize;
        }

        if offset >= pdu_bytes.len() {
            return (String::new(), String::new(), String::new(), 0);
        }

        let pdu_type = pdu_bytes[offset];
        offset += 1;
        let mti = pdu_type & 0x03;

        // Skip message reference for SMS-SUBMIT
        if mti == 0x01 {
            offset += 1;
        }

        if offset >= pdu_bytes.len() {
            return (String::new(), String::new(), String::new(), 0);
        }

        let addr_len = pdu_bytes[offset] as usize;
        offset += 1;

        if offset >= pdu_bytes.len() {
            return (String::new(), String::new(), String::new(), 0);
        }

        let addr_type = pdu_bytes[offset];
        offset += 1;

        let addr_digits = addr_len.div_ceil(2);
        let phone = if offset + addr_digits <= pdu_bytes.len() {
            self.decode_phone_number(&pdu_bytes[offset..offset + addr_digits], addr_type)
        } else {
            String::new()
        };
        offset += addr_digits;

        if offset + 1 >= pdu_bytes.len() {
            return (phone, String::new(), String::new(), 0);
        }

        let _pid = pdu_bytes[offset];
        offset += 1;

        let dcs = pdu_bytes[offset];
        offset += 1;

        let mut timestamp = String::new();
        if mti == 0x00 && offset + 7 <= pdu_bytes.len() {
            timestamp = self.decode_timestamp(&pdu_bytes[offset..offset + 7]);
            offset += 7;
        }

        if offset >= pdu_bytes.len() {
            return (phone, timestamp, String::new(), dcs);
        }

        let tp_udl = pdu_bytes[offset] as usize;
        offset += 1;

        let message = if tp_udl > 0 {
            let ud_bytes = if matches!(dcs, 0x08 | 0x0C | 0x18 | 0x1C) {
                tp_udl * 2
            } else {
                (tp_udl * 7).div_ceil(8)
            };

            if offset + ud_bytes <= pdu_bytes.len() {
                let ud_data = &pdu_bytes[offset..offset + ud_bytes];
                if matches!(dcs, 0x08 | 0x0C | 0x18 | 0x1C) {
                    self.decode_ucs2(ud_data, tp_udl)
                } else {
                    self.decode_7bit(ud_data, tp_udl)
                }
            } else if offset < pdu_bytes.len() {
                let ud_data = &pdu_bytes[offset..];
                if matches!(dcs, 0x08 | 0x0C | 0x18 | 0x1C) {
                    self.decode_ucs2(ud_data, tp_udl)
                } else {
                    self.decode_7bit(ud_data, tp_udl)
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        (phone, timestamp, message, dcs)
    }

    fn build_pdu(
        &mut self,
        phone: &str,
        message: &str,
        sms_class: &str,
        delivery_report: bool,
        invisible: bool,
    ) -> Result<(String, usize), String> {
        let normalized_phone = phone.trim().replace([' ', '-'], "");
        if normalized_phone.is_empty()
            || !normalized_phone
                .chars()
                .enumerate()
                .all(|(idx, c)| c.is_ascii_digit() || (idx == 0 && c == '+'))
        {
            return Err(format!("Invalid phone number: {}", phone));
        }
        if !invisible && !message.is_ascii() {
            return Err("Message contains non-ASCII characters. UCS2 SMS sending is not implemented yet.".to_string());
        }

        let mut pdu_type: u8 = 0x01;
        if delivery_report {
            pdu_type |= 0x20;
            self.log("Delivery report requested (TP-SRR set)", "raw");
        }
        let mr = "00";

        let da_len = normalized_phone.replace('+', "").len();
        let da_type: u8 = if normalized_phone.starts_with('+') { 0x91 } else { 0x81 };
        let tp_da = format!("{:02X}{:02X}{}", da_len, da_type, self.encode_phone(&normalized_phone));

        let pid = if invisible { "40" } else { "00" };

        let dcs = if sms_class == "0" {
            0x50u8
        } else {
            let dcs_str = self
                .dcs_value
                .split_whitespace()
                .next()
                .unwrap_or("0x50")
                .to_string();
            u8::from_str_radix(dcs_str.trim_start_matches("0x"), 16).unwrap_or(0x50)
        };

        let (tp_udl, pdu_no_smsc) = if invisible || message.is_empty() {
            (
                "00".to_string(),
                format!("{:02X}{}{}{}{:02X}{}", pdu_type, mr, tp_da, pid, dcs, "00"),
            )
        } else {
            let msg_chars: Vec<u8> = message
                .chars()
                .filter(|c| (*c as u32) < 128)
                .map(|c| c as u8)
                .collect();
            let packed_ud = pack_7bit(&msg_chars);
            let tp_udl = format!("{:02X}", msg_chars.len());
            let tp_ud = packed_ud
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<String>();
            let pdu = format!(
                "{:02X}{}{}{}{:02X}{}{}",
                pdu_type, mr, tp_da, pid, dcs, tp_udl, tp_ud
            );
            (tp_udl, pdu)
        };

        let pdu_length = decode_hex(&pdu_no_smsc).len();

        self.log(&format!("PDU: {}", pdu_no_smsc), "raw");
        self.log(&format!("DCS: 0x{:02X}, UDL: {}", dcs, tp_udl), "raw");

        Ok((pdu_no_smsc, pdu_length))
    }

    pub(crate) fn send_sms(
        &mut self,
        phones: Vec<String>,
        message: String,
        sms_class: String,
        delivery_report: bool,
    ) {
        let mut sent = 0usize;
        let total = phones.len();
        self.log(&format!("Sending SMS to {} recipient(s)...", total), "sms");
        for phone in phones {
            let display = self.resolve_contact_name(&phone);
            self.log(&format!("Sending to {}...", display), "sms");
            let Ok((pdu, length)) = self.build_pdu(&phone, &message, &sms_class, delivery_report, false) else {
                self.log(&format!("Failed to {}: invalid phone number or unsupported message encoding", display), "error");
                continue;
            };
            self.log(&format!("PDU built for {} ({} chars)", display, length), "system");
            let msg_type = if sms_class == "0" { "flash" } else { "normal" }.to_string();
            let pending = PendingSend {
                phone,
                pdu,
                length,
                msg_type,
                content: message.clone(),
            };
            if self.send_pdu_message(pending) {
                sent += 1;
            }
        }
        if sent == total {
            self.log(&format!("All {} message(s) sent", sent), "sms");
        } else {
            self.log(&format!("Sent {}/{} message(s)", sent, total), "error");
        }
    }

    fn send_pdu_message(&mut self, pending: PendingSend) -> bool {
        let display_phone = self.resolve_contact_name(&pending.phone);
        self.set_serial_busy(true);
        self.drain_response_rx();
        thread::sleep(Duration::from_millis(300));
        self.drain_response_rx();
        self.log("Waiting for modem prompt (> for PDU)...", "system");
        if let Some(ref tx) = self.serial_tx {
            let cmd = format!("AT+CMGS={}", pending.length);
            let _ = tx.send(format!("{}\r\n", cmd));
            self.log(&format!("Sent: {}", cmd), "raw");
        } else {
            self.log("Error: Not connected", "error");
            self.set_serial_busy(false);
            return false;
        }

        let (got_prompt, buf) = self.wait_for_prompt(6);
        if !got_prompt {
            if let Some(cms) = decode_cms_error(&buf) {
                self.log(&format!("Failed to {}: {}", display_phone, cms), "error");
            } else {
                self.log(
                    &format!(
                        "Failed to {}: No prompt from modem (got: {:?})",
                        display_phone, buf
                    ),
                    "error",
                );
            }
            self.set_serial_busy(false);
            return false;
        }

        self.log("Got prompt, sending PDU...", "raw");
        if let Some(ref tx) = self.serial_tx {
            let _ = tx.send(format!("{}\x1A", pending.pdu));
        }

        let response = self.wait_for_send_confirmation(15);
        if response.contains("OK") || response.contains("+CMGS") {
            self.log(&format!("Sent to {}", display_phone), "sms");
            if let Some(cap) = RE_CMGS.captures(&response) {
                if let Ok(mr) = cap[1].parse::<u32>() {
                    self.sent_messages.insert(
                        mr,
                        SentMessageInfo {
                            msg_type: pending.msg_type.clone(),
                            phone: pending.phone.clone(),
                            content: pending.content.clone(),
                        },
                    );
                    let preview = if pending.content.chars().count() > 40 {
                        format!(
                            "{}...",
                            pending.content.chars().take(40).collect::<String>()
                        )
                    } else if pending.content.is_empty() {
                        "(invisible ping)".to_string()
                    } else {
                        pending.content.clone()
                    };
                    self.add_delivery_row(mr, pending.phone, pending.msg_type, preview);
                }
            }
            self.set_serial_busy(false);
            true
        } else {
            let msg =
                decode_cms_error(&response).unwrap_or_else(|| response.trim().to_string());
            self.log(&format!("Failed to {}: {}", display_phone, msg), "error");
            self.set_serial_busy(false);
            false
        }
    }

    pub(crate) fn load_inbox(&mut self) {
        self.log("Loading inbox from modem...", "system");
        let response = self.send_at("AT+CMGL=4", 10);
        self.log("Sent AT+CMGL=4 to modem", "system");
        self.log(
            &format!(
                "Raw response: {:?}",
                response.chars().take(300).collect::<String>()
            ),
            "raw",
        );

        {
            let mut messages = Vec::new();
            for cap in RE_CMGL.captures_iter(&response) {
                let index: usize = cap[1].parse().unwrap_or(0);
                let status = cap[2].to_string();
                let pdu = cap[4].to_string();

                let (phone, timestamp, _, _dcs) = self.decode_sms_simple(&pdu);
                let msg = InboxMessage {
                    index,
                    status: status.clone(),
                    pdu: pdu.clone(),
                    phone: phone.clone(),
                    timestamp: timestamp.clone(),
                    unread: status == "1",
                };
                messages.push(msg);
                self.log(&format!("Loaded message from {}", phone), "system");
            }
            self.inbox_messages = messages;
            self.save_inbox_file();
            self.log(
                &format!("Loaded {} messages from modem", self.inbox_messages.len()),
                "system",
            );
        }
    }

    pub(crate) fn invisible_ping(&mut self) {
        let phones: Vec<String> = self
            .phone_number
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if phones.is_empty() {
            self.warning_message = Some("Enter phone number".to_string());
            return;
        }

        self.log(&format!("Sending invisible ping to {} recipient(s)...", phones.len()), "sms");

        for phone in phones {
            let display = self.resolve_contact_name(&phone);
            self.log(&format!("Pinging {}...", display), "sms");
            let pdu_type = 0x01 | 0x20;
            let mr = "00";
            let da_len = phone.replace('+', "").len();
            let da_type = if phone.starts_with('+') { 0x91 } else { 0x81 };
            let tp_da = format!("{:02X}{:02X}{}", da_len, da_type, self.encode_phone(&phone));
            let pdu = format!("{:02X}{}{}400000", pdu_type, mr, tp_da);
            let length = decode_hex(&pdu).len();

            self.log(&format!("Ping PDU: {}", pdu), "raw");
            self.log("DCS: 0x00, UDL: 0", "raw");
            self.send_pdu_message(PendingSend {
                phone,
                pdu,
                length,
                msg_type: "ping".to_string(),
                content: String::new(),
            });
        }

        self.log("Ping SMS sent. Check delivery reports tab for network delivery status.", "sms");
    }

    pub(crate) fn selected_inbox_message(&self) -> Option<InboxMessage> {
        self.inbox_selected
            .and_then(|idx| self.inbox_messages.get(idx).cloned())
    }

    pub(crate) fn reply_message(&mut self) {
        if let Some(msg) = self.selected_inbox_message() {
            let (phone, _, _, _dcs) = self.decode_sms_simple(&msg.pdu);
            self.phone_number = phone.clone();
            self.current_tab = 0;
            self.log(&format!("Replying to {}", phone), "info");
        }
    }

    pub(crate) fn forward_message(&mut self) {
        if let Some(msg) = self.selected_inbox_message() {
            let (_, _, message, _dcs) = self.decode_sms_simple(&msg.pdu);
            self.message_text = message;
            self.current_tab = 0;
            self.log("Message forwarded to compose box", "info");
        }
    }

    pub(crate) fn delete_message(&mut self) {
        if let Some(list_idx) = self.inbox_selected {
            if let Some(msg) = self.inbox_messages.get(list_idx).cloned() {
                let resp = self.send_at(&format!("AT+CMGD={}", msg.index), 5);
                if resp.contains("OK") || !self.connected {
                    self.inbox_messages.remove(list_idx);
                    self.inbox_selected = None;
                    self.current_inbox_msg = None;
                    self.save_inbox_file();
                    self.log(&format!("Deleted message at index {}", msg.index), "system");
                } else {
                    self.log(&format!("Failed to delete: {}", resp), "error");
                }
            }
        }
    }

    pub(crate) fn clear_modem_sms(&mut self) {
        let resp = self.send_at("AT+CMGD=1,4", 10);
        if resp.contains("OK") {
            self.inbox_messages.clear();
            self.inbox_selected = None;
            self.current_inbox_msg = None;
            self.save_inbox_file();
            self.log("All modem SMS cleared", "system");
        } else {
            self.log(&format!("Failed to clear modem SMS: {}", resp), "error");
        }
    }

    pub(crate) fn clear_modem_delivery(&mut self) {
        self.log(
            "Clearing delivery reports from modem is disabled because the modem command can delete inbox SMS. Use Clear Reports to clear the local list.",
            "error",
        );
    }

    pub(crate) fn parse_delivery_report(&mut self, pdu_hex: &str) {
        let pdu_bytes = decode_hex(pdu_hex);
        if pdu_bytes.len() < 10 {
            return;
        }

        // Parse CDS PDU - SMS-DELIVERY-REPORT
        let mut offset = 0;

        // Skip SMSC if present
        let smsc_len = pdu_bytes[offset] as usize;
        if smsc_len > 0 && smsc_len < 20 {
            offset += 1 + smsc_len;
        }

        if offset >= pdu_bytes.len() {
            return;
        }

        let _pdu_type = pdu_bytes[offset];
        offset += 1;

        if offset >= pdu_bytes.len() {
            return;
        }

        let mr = pdu_bytes[offset] as u32;
        offset += 1;

        // Parse phone number
        if offset >= pdu_bytes.len() {
            return;
        }
        let addr_len = pdu_bytes[offset];
        offset += 1;

        if offset >= pdu_bytes.len() {
            return;
        }
        let addr_type = pdu_bytes[offset];
        offset += 1;

        let addr_digits = (addr_len as usize).div_ceil(2);
        let phone = if offset + addr_digits <= pdu_bytes.len() {
            self.decode_phone_number(&pdu_bytes[offset..offset + addr_digits], addr_type)
        } else {
            "Unknown".to_string()
        };
        offset += addr_digits;

        // Parse sent timestamp (SCTS) - 7 bytes
        let sent_time = if offset + 7 <= pdu_bytes.len() {
            let t = self.decode_timestamp(&pdu_bytes[offset..offset + 7]);
            offset += 7;
            t
        } else {
            "Unknown".to_string()
        };

        // Parse discharge time - 7 bytes
        let discharge_time = if offset + 7 <= pdu_bytes.len() {
            let t = self.decode_timestamp(&pdu_bytes[offset..offset + 7]);
            offset += 7;
            t
        } else {
            "Unknown".to_string()
        };

        let status_byte = if offset < pdu_bytes.len() {
            pdu_bytes[offset]
        } else {
            0xFF
        };
        let status_text = match status_byte {
            0x00 => "Delivered".to_string(),
            0x20 => "Expired".to_string(),
            0x40 => "Deleted by sender".to_string(),
            0x60 => "Replaced by sender".to_string(),
            0x61 => "Congestion".to_string(),
            0x62 => "Busy".to_string(),
            0x63 => "No response".to_string(),
            0x64 => "Service rejected".to_string(),
            0x65 => "QoS not available".to_string(),
            0x66 => "Error in TE".to_string(),
            0x7F => "Remote procedure error".to_string(),
            0xFF => "Status unknown".to_string(),
            _ => format!("Unknown (0x{:02X})", status_byte),
        };

        let msg_info = self.sent_messages.get(&mr).cloned();
        let readable = if let Some(info) = &msg_info {
            if info.msg_type == "ping" {
                if status_byte == 0x00 {
                    "Ping: Delivery confirmed".to_string()
                } else {
                    format!("Ping: {}", status_text)
                }
            } else if status_byte == 0x00 {
                "Delivered".to_string()
            } else {
                status_text.clone()
            }
        } else {
            status_text.clone()
        };

        let tag = if status_byte == 0x00 {
            "delivered"
        } else {
            "failed"
        }
        .to_string();
        let display_phone = self.resolve_contact_name(&phone);
        let detail = if let Some(info) = &msg_info {
            format!(
                "Phone: {}\nOriginal To: {}\nContent: {}\nSent At: {}\nStatus Time: {}\nStatus: {}\nPDU: {}",
                display_phone, info.phone, info.content, sent_time, discharge_time, status_text, pdu_hex
            )
        } else {
            format!(
                "Phone: {}\nSent At: {}\nStatus Time: {}\nStatus: {}\nPDU: {}",
                display_phone, sent_time, discharge_time, status_text, pdu_hex
            )
        };
        self.update_delivery_row(mr, readable.clone(), discharge_time.clone(), tag, detail);
        self.log(&format!("Delivery report: {} - {}", display_phone, readable), "sms");
    }

    fn add_delivery_row(
        &mut self,
        mr: u32,
        phone: String,
        msg_type: String,
        content_preview: String,
    ) {
        let sent_ts = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let display_phone = self.resolve_contact_name(&phone);
        self.dr_records.push(DeliveryRecord {
            mr,
            phone: display_phone,
            msg_type,
            status: "Pending".to_string(),
            sent: sent_ts,
            updated: "".to_string(),
            content: content_preview,
            detail: "".to_string(),
            tag: "pending".to_string(),
        });
    }

    fn update_delivery_row(
        &mut self,
        mr: u32,
        readable_status: String,
        discharge_time: String,
        tag: String,
        detail: String,
    ) {
        if let Some(rec) = self.dr_records.iter_mut().find(|rec| rec.mr == mr) {
            rec.status = readable_status;
            rec.updated = discharge_time;
            rec.tag = tag;
            rec.detail = detail;
            return;
        }
        self.dr_records.push(DeliveryRecord {
            mr,
            phone: "Unknown".to_string(),
            msg_type: "?".to_string(),
            status: readable_status,
            sent: "-".to_string(),
            updated: discharge_time,
            content: String::new(),
            detail,
            tag,
        });
    }

    pub(crate) fn clear_delivery_reports(&mut self) {
        self.dr_records.clear();
        self.dr_selected = None;
        self.dr_detail_text.clear();
    }

    pub(crate) fn handle_direct_sms(&mut self, pdu_hex: &str) {
        self.log("Handling PDU for inbox...", "system");
        let pdu_bytes = decode_hex(pdu_hex);
        if pdu_bytes.is_empty() {
            self.log("Failed to decode PDU hex", "error");
            return;
        }

        let mut offset = 0;
        // Check for SMSC
        if pdu_bytes[0] > 0
            && pdu_bytes[0] < 20
            && offset + (pdu_bytes[0] as usize) < pdu_bytes.len()
        {
            offset += 1 + pdu_bytes[0] as usize;
            self.log(&format!("Skipped SMSC (offset={})", offset), "raw");
        }

        if offset >= pdu_bytes.len() {
            self.log("PDU too short after SMSC skip", "error");
            return;
        }

        let pdu_type = pdu_bytes[offset];
        let mti = pdu_type & 0x03;
        self.log(&format!("PDU Mti: 0x{:02X}", mti), "raw");

        if mti == 0x02 {
            self.log("PDU is a Status Report", "system");
            self.parse_delivery_report(pdu_hex);
        } else {
            let (phone, timestamp, message, _dcs) = self.decode_sms_simple(pdu_hex);
            self.log(&format!("Decoded SMS from {}: {}", phone, message), "sms");
            self.inbox_messages.push(InboxMessage {
                index: 0,
                status: "REC UNREAD".to_string(),
                pdu: pdu_hex.to_string(),
                phone: phone.clone(),
                timestamp,
                unread: true,
            });
            self.save_inbox_file();
            self.log(
                &format!(
                    "New SMS from {}",
                    if phone.is_empty() {
                        "Unknown".to_string()
                    } else {
                        phone
                    }
                ),
                "sms",
            );
        }
    }

    pub(crate) fn handle_cds_response(&mut self, line: &str) {
        self.log(&format!("Received delivery report for {}", if line.len() > 30 { &line[..30] } else { line }), "sms");
        self.expecting_cds_pdu = true;
        if let Some(cap) = RE_CDS_PDU.captures(line) {
            self.expecting_cds_pdu = false;
            self.parse_delivery_report(&cap[1]);
        }
    }

    pub(crate) fn handle_cdsi(&mut self, line: &str) {
        if let Some(cap) = RE_CDSI.captures(line) {
            let index = cap[2].parse::<usize>().unwrap_or(0);
            if let Some(ref tx) = self.serial_tx {
                let _ = tx.send(format!("AT+CMGR={}\r\n", index));
                self.log(&format!("Reading stored delivery report at index {}...", index), "system");
            }
        }
    }

    pub(crate) fn handle_cmti(&mut self, line: &str) {
        if let Some(cap) = RE_CMTI.captures(line) {
            let index = cap[2].parse::<usize>().unwrap_or(0);
            if let Some(ref tx) = self.serial_tx {
                let _ = tx.send(format!("AT+CMGR={}\r\n", index));
                self.log(&format!("Reading new message at index {}...", index), "system");
            }
        }
    }
}
