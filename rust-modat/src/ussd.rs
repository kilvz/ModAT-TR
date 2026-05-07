
impl crate::ModAtApp {
    pub(crate) fn send_ussd(&mut self, code: &str) {
        if let Some(ref tx) = self.serial_tx {
            let dcs = self.ussd_dcs.clone();
            let encoded = if self.ussd_plain_text {
                code.to_string()
            } else {
                Self::encode_ussd_code(code, &dcs)
            };
            let cmd = if dcs == "none" {
                format!("AT+CUSD=1,\"{}\"\r\n", encoded)
            } else {
                format!("AT+CUSD=1,\"{}\",{}\r\n", encoded, dcs)
            };
            let _ = tx.send(cmd);
            self.ussd_response.clear();
            self.ussd_raw_response.clear();
            self.ussd_buttons.clear();
            self.ussd_active = false;
            let ts = chrono::Local::now().format("%H:%M:%S").to_string();
            self.ussd_console.push_str(&format!("[{}] >>> AT+CUSD=1,\"{}\",{}\n", ts, encoded, dcs));
            if !self.ussd_history.contains(&code.to_string()) {
                if self.ussd_history.len() >= 5 { self.ussd_history.remove(0); }
                self.ussd_history.push(code.to_string());
            }
            self.log(&format!("USSD sent: {} (DCS: {}, encoded: {})", code, dcs, encoded), "system");
        } else {
            self.log("Cannot send USSD: not connected", "error");
        }
    }

    fn encode_ussd_code(code: &str, dcs: &str) -> String {
        match dcs {
            "15" | "72" | "0" => {
                // GSM 03.38 encode → 7-bit pack → hex (standard for Huawei modems)
                let gsm_bytes = Self::gsm_encode(code);
                let packed = crate::sms::pack_7bit(&gsm_bytes);
                packed.iter().map(|b| format!("{:02X}", b)).collect::<String>()
            }
            _ => code.to_string(),
        }
    }

    fn gsm_encode(text: &str) -> Vec<u8> {
        text.chars().map(|c| match c {
            '@' => 0x00, '£' => 0x01, '$' => 0x02, '¥' => 0x03,
            'è' => 0x04, 'é' => 0x05, 'ù' => 0x06, 'ì' => 0x07,
            'ò' => 0x08, 'Ç' => 0x09, '\n' => 0x0A, 'Ø' => 0x0B,
            'ø' => 0x0C, '\r' => 0x0D, 'Å' => 0x0E, 'å' => 0x0F,
            'Δ' => 0x10, '_' => 0x11, 'Φ' => 0x12, 'Γ' => 0x13,
            'Λ' => 0x14, 'Ω' => 0x15, 'Π' => 0x16, 'Ψ' => 0x17,
            'Σ' => 0x18, 'Θ' => 0x19, 'Ξ' => 0x1A, '\x1b' => 0x1B,
            'Æ' => 0x1C, 'æ' => 0x1D, 'ß' => 0x1E, 'É' => 0x1F,
            ' ' => 0x20, '!' => 0x21, '"' => 0x22, '#' => 0x23,
            '¤' => 0x24, '%' => 0x25, '&' => 0x26, '\'' => 0x27,
            '(' => 0x28, ')' => 0x29, '*' => 0x2A, '+' => 0x2B,
            ',' => 0x2C, '-' => 0x2D, '.' => 0x2E, '/' => 0x2F,
            '0'..='9' => (c as u32) as u8,
            ':' => 0x3A, ';' => 0x3B, '<' => 0x3C, '=' => 0x3D,
            '>' => 0x3E, '?' => 0x3F, '¡' => 0x40,
            'A'..='Z' => (c as u32) as u8,
            'Ä' => 0x5B, 'Ö' => 0x5C, 'Ñ' => 0x5D, 'Ü' => 0x5E, '§' => 0x5F,
            '¿' => 0x60,
            'a'..='z' => (c as u32) as u8,
            'ä' => 0x7B, 'ö' => 0x7C, 'ñ' => 0x7D, 'ü' => 0x7E, 'à' => 0x7F,
            _ => c as u8,
        }).collect()
    }

    fn gsm_decode(data: &[u8]) -> String {
        data.iter().map(|&b| match b {
            0x00 => '@', 0x01 => '£', 0x02 => '$', 0x03 => '¥',
            0x04 => 'è', 0x05 => 'é', 0x06 => 'ù', 0x07 => 'ì',
            0x08 => 'ò', 0x09 => 'Ç', 0x0A => '\n', 0x0B => 'Ø',
            0x0C => 'ø', 0x0D => '\r', 0x0E => 'Å', 0x0F => 'å',
            0x10 => 'Δ', 0x11 => '_', 0x12 => 'Φ', 0x13 => 'Γ',
            0x14 => 'Λ', 0x15 => 'Ω', 0x16 => 'Π', 0x17 => 'Ψ',
            0x18 => 'Σ', 0x19 => 'Θ', 0x1A => 'Ξ', 0x1B => '\x1b',
            0x1C => 'Æ', 0x1D => 'æ', 0x1E => 'ß', 0x1F => 'É',
            0x20..=0x3F => b as char,
            0x40 => '¡',
            0x41..=0x5A => b as char,
            0x5B => 'Ä', 0x5C => 'Ö', 0x5D => 'Ñ', 0x5E => 'Ü', 0x5F => '§',
            0x60 => '¿',
            0x61..=0x7A => b as char,
            0x7B => 'ä', 0x7C => 'ö', 0x7D => 'ñ', 0x7E => 'ü', 0x7F => 'à',
            _ => b as char,
        }).collect()
    }

    pub(crate) fn reply_ussd(&mut self, option: usize) {
        if let Some(ref tx) = self.serial_tx {
            let dcs = self.ussd_dcs.clone();
            let code = option.to_string();
            let encoded = if self.ussd_plain_text {
                code.clone()
            } else {
                Self::encode_ussd_code(&code, &dcs)
            };
            let cmd = format!("AT+CUSD=1,\"{}\",{}\r\n", encoded, dcs);
            let _ = tx.send(cmd);
            self.ussd_response.clear();
            let ts = chrono::Local::now().format("%H:%M:%S").to_string();
            self.ussd_console.push_str(&format!("[{}] >>> AT+CUSD=1,\"{}\",{}\n", ts, encoded, dcs));
            self.log(&format!("USSD reply: {} (DCS: {})", option, dcs), "system");
        } else {
            self.log("Cannot reply USSD: not connected", "error");
        }
    }

    pub(crate) fn cancel_ussd(&mut self) {
        if let Some(ref tx) = self.serial_tx {
            let _ = tx.send("AT+CUSD=2\r\n".to_string());
        }
        self.ussd_response.clear();
        self.ussd_raw_response.clear();
        self.ussd_buttons.clear();
        self.ussd_active = false;
        let ts = chrono::Local::now().format("%H:%M:%S").to_string();
        self.ussd_console.push_str(&format!("[{}] >>> AT+CUSD=2 (cancel)\n", ts));
        self.log("USSD session cancelled", "system");
    }

    pub(crate) fn handle_cusd_urc(&mut self, line: &str) {
        let ts = chrono::Local::now().format("%H:%M:%S").to_string();
        self.ussd_console.push_str(&format!("[{}] <<< {}\n", ts, line));
        self.log(&format!("USSD URC received: {}", line), "system");

        let status: i32 = line
            .strip_prefix("+CUSD:")
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(-1);

        let response_text = if let Some(start) = line.find('"') {
            if let Some(end) = line[start+1..].find('"') {
                line[start+1..start+1+end].to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        match status {
            0 => {
                self.ussd_response = if response_text.is_empty() {
                    "USSD: No further action required".to_string()
                } else {
                    self.decode_ussd_text(&response_text)
                };
                self.ussd_raw_response = response_text;
                self.ussd_active = false;
                self.ussd_buttons.clear();
            }
            2 => {
                if !response_text.is_empty() {
                    self.ussd_response = self.decode_ussd_text(&response_text);
                    self.ussd_raw_response = response_text;
                    self.ussd_active = true;
                    self.ussd_buttons.clear();
                    self.extract_ussd_buttons(&self.ussd_response.clone());
                } else {
                    self.ussd_response = "USSD: Session ended (no response from network)".to_string();
                    self.ussd_raw_response.clear();
                    self.ussd_active = false;
                    self.ussd_buttons.clear();
                }
            }
            1 => {
                self.ussd_active = true;
                if !response_text.is_empty() {
                    self.ussd_response = self.decode_ussd_text(&response_text);
                    self.ussd_raw_response = response_text;
                    self.ussd_buttons.clear();
                    self.extract_ussd_buttons(&self.ussd_response.clone());
                }
            }
            _ => {
                self.ussd_response = format!("USSD: unknown status {}", status);
                self.ussd_active = false;
            }
        }
    }

    fn decode_ussd_text(&self, text: &str) -> String {
        // Try GSM 7-bit packed hex first (most common)
        if text.len().is_multiple_of(2) && text.chars().all(|c| c.is_ascii_hexdigit()) {
            let bytes: Vec<u8> = (0..text.len())
                .step_by(2)
                .filter_map(|i| u8::from_str_radix(&text[i..i+2], 16).ok())
                .collect();
            if !bytes.is_empty() {
                // Try 7-bit unpack
                let unpacked = Self::gsm_7bit_unpack(&bytes);
                return Self::gsm_decode(&unpacked);
            }
        }
        text.to_string()
    }

    fn gsm_7bit_unpack(data: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        let mut buffer: u64 = 0;
        let mut bits: u32 = 0;
        for &byte in data {
            buffer |= (byte as u64) << bits;
            bits += 8;
            while bits >= 7 {
                result.push((buffer & 0x7F) as u8);
                buffer >>= 7;
                bits -= 7;
            }
        }
        if bits > 0 {
            result.push((buffer & 0x7F) as u8);
        }
        result
    }

    fn extract_ussd_buttons(&mut self, decoded: &str) {
        let mut buttons: Vec<String> = Vec::new();
        let normalized = decoded.replace("\r\n", "\n").replace('\r', "\n");
        for line in normalized.split('\n') {
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }
            let chars: Vec<char> = trimmed.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                let at_start = i == 0;
                let after_ws = i > 0 && chars[i - 1].is_whitespace();
                if chars[i].is_ascii_digit()
                    && (at_start || after_ws)
                    && i + 1 < chars.len()
                    && (chars[i+1] == '.' || chars[i+1] == ')' || chars[i+1] == '-' || chars[i+1] == ' ')
                {
                    let start = i;
                    i += 1;
                    while i < chars.len() && !(
                        i + 1 < chars.len()
                        && chars[i].is_ascii_digit()
                        && (chars[i+1] == '.' || chars[i+1] == ')' || chars[i+1] == '-')
                        && (i == 0 || chars[i - 1].is_whitespace())
                    ) {
                        i += 1;
                    }
                    let text = trimmed[start..i].trim().to_string();
                    if text.len() > 1 {
                        buttons.push(text);
                    }
                } else {
                    i += 1;
                }
            }
        }
        if !buttons.is_empty() {
            self.ussd_buttons = buttons;
        }
    }
}
