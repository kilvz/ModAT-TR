use crate::patterns::*;
use egui::Color32;

pub(crate) fn category_label(category: &str) -> &'static str {
    match category {
        "error" => "ERR",
        "sms" => "SMS",
        "system" => "SYS",
        "raw" => "RAW",
        "at" => " AT",
        _ => "INF",
    }
}

pub(crate) fn log_color(category: &str) -> Color32 {
    match category {
        "error" => Color32::from_rgb(255, 85, 85),
        "sms" => Color32::from_rgb(80, 250, 123),
        "system" => Color32::from_rgb(139, 233, 253),
        "raw" => Color32::from_rgb(98, 114, 164),
        _ => Color32::from_rgb(248, 248, 242),
    }
}

#[allow(dead_code)]
pub(crate) fn at_commands() -> Vec<(&'static str, &'static str)> {
    vec![
        // --- Universal / 3GPP Standard (Works on most modems) ---
        ("AT", "Universal: Test AT interface"),
        ("ATI", "Universal: Request product ID"),
               ("ATE0", "Universal: Echo off"),
        ("ATE1", "Universal: Echo on"),
        ("ATZ", "Universal: Reset to factory defaults"),
        ("AT&F", "Universal: Restore factory settings"),
        ("AT&V", "Universal: View current config"),
        ("AT+CLAC", "Universal: List all available AT commands"),
        ("AT+CGMI", "Universal: Request manufacturer identification"),
        ("AT+CGMM", "Universal: Request model identification"),
        ("AT+CGMR", "Universal: Request firmware revision"),
        ("AT+CGSN", "Universal: Request IMEI"),
        ("AT+CIMI", "Universal: Request IMSI"),
        ("AT+CCID", "Universal: Show ICCID of SIM"),
        ("AT+CPIN?", "Universal: Check SIM PIN status"),
        ("AT+CPIN=", "Universal: Enter SIM PIN"),
        ("AT+CPUK=", "Universal: Enter PUK code"),
        ("AT+CLCK=", "Universal: Facility lock"),
        ("AT+CPWD=", "Universal: Change password"),
        ("AT+CREG?", "Universal: Network registration status"),
        ("AT+CREG=0", "Universal: Disable +CREG URC"),
        ("AT+CREG=1", "Universal: Enable +CREG URC"),
        ("AT+CREG=2", "Universal: Enable +CREG URC with location"),
        ("AT+CGREG?", "Universal: GPRS registration status"),
        ("AT+CGREG=2", "Universal: Enable +CGREG URC with location"),
        ("AT+CEREG?", "Universal: EPS registration status"),
        ("AT+CEREG=2", "Universal: Enable +CEREG URC with location"),
        ("AT+COPS?", "Universal: Query current operator"),
        ("AT+COPS=?", "Universal: Scan available operators"),
        ("AT+COPS=0", "Universal: Auto operator selection"),
        ("AT+COPS=1,2,", "Universal: Manual operator select by MCC/MNC"),
        ("AT+CSQ", "Universal: Signal quality query"),
        ("AT+CESQ", "Universal: Extended signal quality"),
        ("AT+CIND?", "Universal: Indicator status"),
        ("AT+CNMI=", "Universal: New SMS notification mode"),
        ("AT+CMGF=0", "Universal: Set PDU mode"),
        ("AT+CMGF=1", "Universal: Set text mode"),
        ("AT+CMGL=4", "Universal: List all SMS"),
        ("AT+CMGL=\"ALL\"", "Universal: List all SMS (text mode)"),
        ("AT+CMGR=", "Universal: Read SMS at index"),
        ("AT+CMGD=", "Universal: Delete SMS at index"),
        ("AT+CMGD=1,4", "Universal: Delete all SMS"),
        ("AT+CMGS=", "Universal: Send SMS"),
        ("AT+CSCA?", "Universal: Show SMS service centre"),
        ("AT+CSCA=", "Universal: Set SMS service centre"),
        ("AT+CPMS?", "Universal: Preferred SMS storage"),
        ("AT+CPMS=\"SM\"", "Universal: Set storage to SIM"),
        ("AT+CPMS=\"ME\"", "Universal: Set storage to modem"),
        ("AT+CSMP?", "Universal: SMS text mode parameters"),
        ("AT+CFUN?", "Universal: Phone functionality status"),
        ("AT+CFUN=0", "Universal: Minimum functionality"),
        ("AT+CFUN=1", "Universal: Full functionality"),
        ("AT+CFUN=4", "Universal: Flight mode"),
        ("AT+CFUN=1,1", "Universal: Full + reset modem"),
        ("AT+CMEE=1", "Universal: Verbose errors"),
        ("AT+CMEE=2", "Universal: Verbose errors (with code names)"),
        ("AT+CGDCONT?", "Universal: Query PDP context / APN settings"),
        ("AT+CGDCONT=", "Universal: Set PDP context / APN settings"),
        ("AT+CGACT?", "Universal: PDP activation status"),
        ("AT+CGPADDR", "Universal: Show PDP IP address"),
        ("AT+CGATT?", "Universal: GPRS attach status"),
        ("AT+CGATT=1", "Universal: Attach GPRS"),
        ("AT+CGATT=0", "Universal: Detach GPRS"),
        ("ATD", "Universal: Dial a number"),
        ("ATH", "Universal: Hang up"),
        ("ATA", "Universal: Answer incoming call"),
        ("AT+CLCC", "Universal: List current calls"),
        ("AT+CUSD=1,", "Universal: Send USSD code"),
        ("AT+CUSD=0", "Universal: Cancel USSD"),

        // --- Huawei Specific (Often using ^ prefix) ---
        ("AT^HCSQ?", "Huawei: Signal quality (RSRP, SINR, RSRQ)"),
        ("AT^HCSQ=1", "Huawei: Enable ^HCSQ URC"),
        ("AT^HFREQINFO?", "Huawei: Current band/frequency info"),
        ("AT^HFREQINFO=1", "Huawei: Enable ^HFREQINFO URC"),
        ("AT^SYSINFOEX", "Huawei: Extended system info"),
        ("AT^SYSINFO", "Huawei: System info"),
        ("AT^CELLINFO?", "Huawei: Cell info"),
        ("AT^MONSC", "Huawei: Neighbour cell monitor"),
        ("AT^NRBAND?", "Huawei: NR (5G) band config"),
        ("AT^VERSION?", "Huawei: Firmware/component versions"),
        ("AT^GETPORTMODE", "Huawei: Current USB port mode"),
        ("AT^SETPORT?", "Huawei: Query USB port composition"),
        ("AT^SETPORT=", "Huawei: Set USB port composition"),
        ("AT^SETPORT=\"A1,A2;10,12,16,A1,A2\"", "Huawei: Debug/PC UI port mode"),
        ("AT^SETPORT=\"FF;10,12,16\"", "Huawei: Normal/Project port mode"),
        ("AT^U2DIAG?", "Huawei: Query USB mode"),
        ("AT^U2DIAG=0", "Huawei: Switch modem-only"),
        ("AT^U2DIAG=256", "Huawei: Switch HiLink+NDIS"),
        ("AT^CARDLOCK?", "Huawei: SIM/network lock status"),
        ("AT^CARDLOCK=", "Huawei: Enter unlock code"),
        ("AT^DATALOCK=", "Huawei: Data lock/unlock (OEM Code)"),
        ("AT^CVOICE?", "Huawei: Voice capability query"),
        ("AT^NDISDUP?", "Huawei: NDIS dial status"),
        ("AT^NDISDUP=", "Huawei: NDIS dial/connect command"),
        ("AT^DHCP?", "Huawei: DHCP/IP info"),
        ("AT^AUTHDATA?", "Huawei: Auth/APN profile query"),
        ("AT^CURC?", "Huawei: URC config query"),
        ("AT^CURC=", "Huawei: URC config"),
        ("AT^BOOT?", "Huawei: Boot mode query"),
        ("AT^GODLOAD", "Huawei: Enter download mode"),
        ("AT^FHVER", "Huawei: Firmware/hardware version"),
        ("AT^HWVER", "Huawei: Hardware version"),
        ("AT^SN", "Huawei: Serial number"),
        ("AT^IMEI?", "Huawei: IMEI query"),
        ("AT^ICCID?", "Huawei: ICCID query"),
        ("AT^RESET", "Huawei: Modem reset"),
        ("AT^RFSWITCH?", "Huawei: RF switch query"),
        ("AT^PREFMODE?", "Huawei: Preferred mode query"),
        ("AT^PREFMODE=", "Huawei: Preferred mode set"),
        ("AT^BAND?", "Huawei: Band query"),
        ("AT^BAND=", "Huawei: Band set"),
        ("AT^LTELOCK?", "Huawei: LTE cell/frequency lock query"),
        ("AT^LTELOCK=", "Huawei: LTE cell/frequency lock set"),
        ("AT^NWTIME?", "Huawei: Network time query"),
        ("AT^PLMN?", "Huawei: PLMN query"),
        ("AT^SYSCFG?", "Huawei: System config"),
        ("AT^SYSCFG=", "Huawei: Set system config"),
        ("AT^SYSCFGEX?", "Huawei: Extended system config"),
        ("AT^SYSCFGEX=", "Huawei: Set extended system config"),

        // --- Qualcomm / Snapdragon (Quectel, SIMCom, Telit, etc) ---
        ("AT+QENG=\"servingcell\"", "Qualcomm: Engineering mode info"),
        ("AT+QENG=\"neighbourcell\"", "Qualcomm: Neighbour cells info"),
        ("AT+QCAINFO", "Qualcomm: Carrier Aggregation info"),
        ("AT+QNWINFO", "Qualcomm: Network info (Band, EARFCN)"),
        ("AT+QRSRP", "Qualcomm: Reference Signal Received Power"),
        ("AT+QRSRQ", "Qualcomm: Reference Signal Received Quality"),
        ("AT+QSIMSTAT", "Qualcomm: SIM insertion status"),
        ("AT+QGMR", "Qualcomm: Firmware revision"),
        ("AT+QCFG=\"band\"", "Qualcomm: Band configuration"),
        ("AT+QCFG=\"nwscanmode\"", "Qualcomm: Network scan mode"),
        ("AT+QCFG=\"nwscanseq\"", "Qualcomm: Network scan sequence"),
        ("AT+QCFG=\"iotopmode\"", "Qualcomm: IoT operation mode"),
        ("AT$QCPMPREFE?", "Qualcomm: Preferred system query"),
        ("AT$QCRSRP?", "Qualcomm: RSRP information"),
        ("AT+QINACT", "Qualcomm: Check inactivity"),
        ("AT+QPOWD", "Qualcomm: Power down"),
        ("AT+QDSIM", "Qualcomm: Dual SIM configuration"),

        // --- MediaTek (MTK) ---
        ("AT+EGMR=1,7", "MediaTek: Request IMEI"),
        ("AT+EGMR=1,10", "MediaTek: Request Serial Number"),
        ("AT+ERSRP?", "MediaTek: RSRP status"),
        ("AT+ERSRQ?", "MediaTek: RSRQ status"),
        ("AT+EMPHINFO?", "MediaTek: Phone/Hardware info"),
        ("AT+ESIMS?", "MediaTek: SIM status"),
        ("AT+EMSR?", "MediaTek: Serving cell info"),
        ("AT+ECFG?", "MediaTek: Configuration query"),
        ("AT+EMLOC?", "MediaTek: Location info"),
        ("AT+ETXPW?", "MediaTek: Transmit power query"),
        ("AT+EPWROFF", "MediaTek: Power off"),
        ("AT+ERAT?", "MediaTek: RAT selection query"),
    ]
}

impl crate::ModAtApp {
    pub(crate) fn readable_raw(&self, raw: &str) -> Option<String> {
        if let Some(m) = RE_HCSQ_ALT.captures(raw) {
            let tech = &m[1];
            let rssi = m[2].parse::<i32>().unwrap_or(0) - 120;
            let rsrp = m[3].parse::<i32>().unwrap_or(0) - 140;
            let sinr = (m[4].parse::<f32>().unwrap_or(0.0) * 0.2) - 20.0;
            let rsrq = (m[5].parse::<f32>().unwrap_or(0.0) * 0.5) - 19.5;
            return Some(format!(
                "Signal [{}] RSSI:{}dBm  RSRP:{}dBm  SINR:{:.1}dB  RSRQ:{:.1}dB",
                tech, rssi, rsrp, sinr, rsrq
            ));
        }
        if let Some(m) = RE_RSSI.captures(raw) {
            return Some(format!("RSSI level: {}/31", &m[1]));
        }
        if let Some(m) = RE_SIMPLE_CREG.captures(raw) {
            let stat = match &m[1] {
                "0" => "Not registered",
                "1" => "Registered (Home)",
                "2" => "Searching",
                "3" => "Denied",
                "5" => "Roaming",
                v => v,
            };
            let extra = m
                .get(2)
                .map(|lac| {
                    format!(
                        "  TAC:{}  CID:{}",
                        lac.as_str(),
                        m.get(3).map(|v| v.as_str()).unwrap_or("")
                    )
                })
                .unwrap_or_default();
            return Some(format!("Network: {}{}", stat, extra));
        }
        if raw.contains("^HFREQINFO:") {
            return Some("Frequency info updated (see Network Info tab)".to_string());
        }
        None
    }

    pub(crate) fn get_signal_color(&self) -> Color32 {
        if self.signal == "---" || self.signal.contains("No signal") {
            return Color32::GRAY;
        }
        if let Some(m) = RE_SIGNAL_BARS.captures(&self.signal) {
            if let Ok(csq) = m[1].parse::<i32>() {
                if csq >= 20 {
                    return Color32::GREEN;
                } else if csq >= 12 {
                    return Color32::YELLOW;
                } else {
                    return Color32::RED;
                }
            }
        }
        Color32::GRAY
    }

    pub(crate) fn get_value_color(
        &self,
        val_str: &str,
        thresholds: (f32, f32),
        higher_is_better: bool,
    ) -> Color32 {
        if val_str == "---" {
            return Color32::GRAY;
        }
        if let Ok(val) = val_str.parse::<f32>() {
            if higher_is_better {
                if val >= thresholds.0 {
                    Color32::GREEN
                } else if val >= thresholds.1 {
                    Color32::YELLOW
                } else {
                    Color32::RED
                }
            } else {
                if val <= thresholds.0 {
                    Color32::GREEN
                } else if val <= thresholds.1 {
                    Color32::YELLOW
                } else {
                    Color32::RED
                }
            }
        } else {
            Color32::GRAY
        }
    }

    // ─── Char count ───
    pub(crate) fn update_char_count(&mut self) {
        let length = self.message_text.len();
        let segments = std::cmp::max(1, length.div_ceil(160));
        let remaining = std::cmp::max(0, 160 * segments - length);
        self.char_count = format!("{} / {} ({} seg)", length, remaining, segments);
    }

    pub(crate) fn sync_dcs_from_class(&mut self) {
        self.dcs_value = match self.sms_class {
            0 => "0x50 (Class 0 - 7bit) [OK]".to_string(),
            1 => "0x11 (Class 1 - 7bit) [OK]".to_string(),
            2 => "0x12 (Class 2 - 7bit) [OK]".to_string(),
            3 => "0x13 (Class 3 - 7bit) [OK]".to_string(),
            _ => "0x50 (Class 0 - 7bit) [OK]".to_string(),
        };
    }

    pub(crate) fn sync_class_from_dcs(&mut self) {
        let prefix = self
            .dcs_value
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase();
        self.sms_class = match prefix.as_str() {
            "0x50" | "0x10" | "0xf0" => 0,
            "0x11" | "0x01" | "0xf1" => 1,
            "0x12" | "0x02" | "0xf2" => 2,
            "0x13" | "0x03" | "0xf3" => 3,
            _ => self.sms_class,
        };
    }

    pub(crate) fn clear_fields(&mut self) {
        self.message_text.clear();
        self.update_char_count();
    }
}
