use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use egui::Color32;

use crate::patterns::*;
use crate::serial::SerialPortWrapper;
use crate::{AppEvent, FullModemInfo};

// ─── Hardware Watcher ───

pub(crate) fn spawn_hardware_watcher(tx: mpsc::Sender<AppEvent>) {
    thread::spawn(move || {
        let mut last_ports = serialport::available_ports()
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.port_name)
            .collect::<HashSet<_>>();
        let mut last_rndis = crate::ModAtApp::remote_ndis_adapter_available();

        loop {
            thread::sleep(Duration::from_secs(2));
            let current_ports = serialport::available_ports()
                .unwrap_or_default()
                .into_iter()
                .map(|p| p.port_name)
                .collect::<HashSet<_>>();
            let current_rndis = crate::ModAtApp::remote_ndis_adapter_available();

            if current_ports != last_ports || current_rndis != last_rndis {
                let _ = tx.send(AppEvent::Log(format!("COM ports changed ({} available), RNDIS: {}", current_ports.len(), if current_rndis { "Available" } else { "Not Available" }), "system".to_string()));
                let _ = tx.send(AppEvent::HardwareChanged);
                last_ports = current_ports;
                last_rndis = current_rndis;
            }
        }
    });
}

// ─── Modem detection/probing/COM port management ───

pub(crate) fn map_operator(raw: &str) -> String {
    match raw {
        "51010" => "Telkomsel".to_string(),
        "51001" => "Indosat".to_string(),
        "51011" => "XL".to_string(),
        "51089" => "Tri".to_string(),
        "51009" => "Smartfren".to_string(),
        other => other.to_string(),
    }
}

impl crate::ModAtApp {
    // ─── Serial busy ───
    pub(crate) fn set_serial_busy(&self, busy: bool) {
        if let Ok(mut serial_busy) = self.serial_busy.lock() {
            *serial_busy = busy;
        }
    }

    // ─── Response drain ───
    pub(crate) fn drain_response_rx(&mut self) {
        if let Some(ref rx) = self.response_rx {
            if let Ok(rx) = rx.lock() {
                while rx.try_recv().is_ok() {}
            }
        }
    }

    // ─── Serial Communication ───
    pub(crate) fn send_at(&mut self, cmd: &str, timeout_secs: u64) -> String {
        self.set_serial_busy(true);
        self.drain_response_rx();
        let result = if let Some(ref tx) = self.serial_tx {
            let _ = tx.send(format!("{}\r\n", cmd));

            if let Some(ref rx_mutex) = self.response_rx {
                if let Ok(rx) = rx_mutex.lock() {
                    let start = Instant::now();
                    let mut response = String::new();
                    while start.elapsed() < Duration::from_secs(timeout_secs) {
                        while let Ok(msg) = rx.try_recv() {
                            response.push_str(&msg);
                            if response.contains("OK\r\n") || response.contains("ERROR") {
                                drop(rx);
                                let response = response.trim().to_string();
                                self.set_serial_busy(false);
                                return response;
                            }
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                    response.trim().to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        self.set_serial_busy(false);
        result
    }

    pub(crate) fn send_at_multi(&mut self, cmd: &str, timeout_secs: u64) -> String {
        self.set_serial_busy(true);
        self.drain_response_rx();
        let result = if let Some(ref tx) = self.serial_tx {
            let _ = tx.send(format!("{}\r\n", cmd));

            if let Some(ref rx_mutex) = self.response_rx {
                if let Ok(rx) = rx_mutex.lock() {
                    let start = Instant::now();
                    let mut last_data = Instant::now();
                    let mut response = String::new();
                    while start.elapsed() < Duration::from_secs(timeout_secs) {
                        let mut got_data = false;
                        while let Ok(msg) = rx.try_recv() {
                            response.push_str(&msg);
                            got_data = true;
                        }
                        if got_data {
                            last_data = Instant::now();
                        }
                        if !response.is_empty() && last_data.elapsed() > Duration::from_secs(1) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                    response.trim().to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        self.set_serial_busy(false);
        result
    }

    pub(crate) fn wait_for_prompt(&mut self, timeout_secs: u64) -> (bool, String) {
        let mut pending_logs = Vec::new();
        let result = if let Some(ref rx_mutex) = self.response_rx {
            if let Ok(rx) = rx_mutex.lock() {
                let start = Instant::now();
                let mut accumulated = String::new();
                let mut found = false;
                let mut prompt = false;
                while start.elapsed() < Duration::from_secs(timeout_secs) {
                    while let Ok(msg) = rx.try_recv() {
                        pending_logs.push(format!("Prompt buf: {:?}", msg));
                        accumulated.push_str(&msg);
                        if accumulated.contains('>') {
                            found = true;
                            prompt = true;
                            break;
                        }
                        let before_prompt = accumulated.split('>').next().unwrap_or("");
                        if accumulated.contains("+CMS ERROR") || before_prompt.contains("ERROR") {
                            found = true;
                            prompt = false;
                            break;
                        }
                    }
                    if found { break; }
                    thread::sleep(Duration::from_millis(50));
                }
                (prompt, accumulated)
            } else {
                (false, String::new())
            }
        } else {
            (false, String::new())
        };
        for l in &pending_logs {
            self.log(l, "raw");
        }
        result
    }

    pub(crate) fn wait_for_send_confirmation(&mut self, timeout_secs: u64) -> String {
        let mut pending_logs = Vec::new();
        let response = if let Some(ref rx_mutex) = self.response_rx {
            if let Ok(rx) = rx_mutex.lock() {
                let start = Instant::now();
                let mut response = String::new();
                let mut found = false;
                while start.elapsed() < Duration::from_secs(timeout_secs) {
                    while let Ok(msg) = rx.try_recv() {
                        pending_logs.push(format!("Send resp: {:?}", msg));
                        response.push_str(&msg);
                        if response.contains("OK\r\n") || response.contains("ERROR") {
                            found = true;
                            break;
                        }
                    }
                    if found { break; }
                    thread::sleep(Duration::from_millis(100));
                }
                response
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        for l in &pending_logs {
            self.log(l, "raw");
        }
        response
    }

    // ─── Connection ───
    pub(crate) fn connect_serial(&mut self, port_name: &str, baud: u32) -> bool {
        match serialport::new(port_name, baud)
            .timeout(Duration::from_millis(50))
            .open()
        {
            Ok(port) => {
                let wrapper = SerialPortWrapper::new(port);
                let arc_port = Arc::new(Mutex::new(wrapper));

                self.log(&format!("Serial port {} opened", port_name), "system");

                let (tx, rx) = mpsc::channel::<String>();
                let (resp_tx, resp_rx) = mpsc::channel::<String>();
                let (ui_tx, ui_rx) = mpsc::channel::<String>();

                let (worker_tx, mut worker_rx) = mpsc::channel();

                self.serial_tx = Some(tx.clone());
                self.response_rx = Some(Arc::new(Mutex::new(resp_rx)));
                self.ui_rx = Some(ui_rx);
                self.serial_port = Some(arc_port.clone());
                self.connected_port = Some(port_name.to_string());

                self.reader_running = true;
                let port_clone = arc_port.clone();
                let resp_tx_clone = resp_tx.clone();
                let ui_tx_clone = ui_tx.clone();
                let worker_tx_clone = worker_tx.clone();
                let running = Arc::new(Mutex::new(true));
                let running_clone = running.clone();
                self.reader_flag = Some(running);

                let handle = thread::spawn(move || {
                    let mut buffer = [0u8; 1024];
                    let mut leftover = String::new();
                    while *running_clone.lock().unwrap() {
                        let mut port = port_clone.lock().unwrap();
                        while let Ok(cmd) = rx.try_recv() {
                            let _ = port.write_all(cmd.as_bytes());
                            let _ = port.flush();
                        }
                        if let Ok(n) = port.read(&mut buffer) {
                            if n > 0 {
                                if let Ok(s) = String::from_utf8(buffer[..n].to_vec()) {
                                    let _ = resp_tx_clone.send(s.clone());
                                    let _ = ui_tx_clone.send(s.clone());
                                    let _ = worker_tx_clone.send(s.clone());

                                    // Parse for delivery reports (+CDS)
                                    let combined = format!("{}{}", leftover, s);
                                    leftover.clear();

                                    // Check for +CDS messages
                                    if let Some(cds_pos) = combined.find("+CDS:") {
                                        if let Some(newline_pos) = combined[cds_pos..].find('\n') {
                                            let cds_line =
                                                &combined[cds_pos..cds_pos + newline_pos];
                                            // Send to UI for processing
                                            let _ = resp_tx_clone
                                                .send(format!("CDS_DETECTED: {}\n", cds_line));
                                        }
                                    }
                                }
                            }
                        }
                        drop(port);
                        thread::sleep(Duration::from_millis(10));
                    }
                });

                self.reader_thread = Some(handle);
                self.log(
                    &format!("Connecting to {} at {}...", port_name, baud),
                    "system",
                );

                let at_resp = self.send_at("AT", 3);
                if !at_resp.contains("OK") {
                    self.log("Connection failed: Modem not responding", "error");
                    self.disconnect();
                    return false;
                }
                self.log("AT handshake OK", "system");
                let cpin_resp = self.send_at("AT+CPIN?", 3);
                if !cpin_resp.contains("READY") {
                    self.log("Connection failed: SIM not ready", "error");
                    self.disconnect();
                    return false;
                }
                self.log("SIM ready", "system");
                let cmgf_resp = self.send_at("AT+CMGF=0", 3);
                if !cmgf_resp.contains("OK") {
                    self.log("Connection failed: Failed to set PDU mode", "error");
                    self.disconnect();
                    return false;
                }
                self.log("PDU mode set", "system");

                self.connected = true;
                self.status_text = format!("Connected: {}", port_name);
                self.status_color = Color32::GREEN;
                self.connected_port = Some(port_name.to_string());
                let detected_profile = Self::detect_port_profile_static(port_name);
                self.serial_profile = if detected_profile == "Unknown" {
                    self.switch_mode.clone()
                } else {
                    detected_profile
                };
                self.update_serial_profile_label();
                self.log("Connected successfully", "system");
                self.refresh_rndis_status_async();

                // Spawn sequential info worker
                let tx = self.app_event_tx.clone();
                let serial_tx = self.serial_tx.clone();
                let reader_flag = self.reader_flag.clone();
                let serial_busy = self.serial_busy.clone();

                thread::spawn(move || {
                    let mut info = FullModemInfo::default();
                    let wait_resp = |rx: &mut mpsc::Receiver<String>, timeout: u64| -> String {
                        let start = Instant::now();
                        let mut full_resp = String::new();
                        while start.elapsed().as_secs() < timeout {
                            if let Ok(line) = rx.try_recv() {
                                full_resp.push_str(&line);
                                if full_resp.contains("OK") || full_resp.contains("ERROR") {
                                    return full_resp;
                                }
                            }
                            thread::sleep(Duration::from_millis(10));
                        }
                        full_resp
                    };

                    loop {
                        if let Some(ref flag) = reader_flag {
                            if let Ok(running) = flag.lock() {
                                if !*running { break; }
                            }
                        }

                        if let Some(ref s_tx) = serial_tx {
                            if serial_busy.lock().map(|busy| *busy).unwrap_or(true) {
                                thread::sleep(Duration::from_millis(250));
                                continue;
                            }

                            // CSQ
                            let _ = s_tx.send("AT+CSQ\r\n".to_string());
                            let resp = wait_resp(&mut worker_rx, 2);
                            if let Some(m) = RE_CSQ.captures(&resp) {
                                if let Ok(csq) = m[1].parse::<i32>() {
                                    info.signal = if csq == 99 { "No signal".to_string() } else { format!("{}/5 ({})", std::cmp::min(5, csq / 6), csq) };
                                }
                            }

                            // COPS
                            let _ = s_tx.send("AT+COPS=3,0\r\n".to_string());
                            let _ = wait_resp(&mut worker_rx, 1);
                            let _ = s_tx.send("AT+COPS?\r\n".to_string());
                            let resp = wait_resp(&mut worker_rx, 2);
                            if let Some(m) = RE_COPS_QUOTED.captures(&resp) {
                                info.operator = map_operator(&m[1]);
                            }

                            // SYSINFOEX
                            let _ = s_tx.send("AT^SYSINFOEX\r\n".to_string());
                            let resp = wait_resp(&mut worker_rx, 2);
                            if let Some(m) = RE_SYSINFOEX.captures(&resp) {
                                info.network = m[1].to_string();
                            }

                            // HCSQ
                            let _ = s_tx.send("AT^HCSQ?\r\n".to_string());
                            let resp = wait_resp(&mut worker_rx, 2);
                            if let Some(m) = RE_HCSQ.captures(&resp) {
                                info.net_tech = m[1].to_string();
                                let rssi = m[2].parse::<i32>().unwrap_or(0) - 120;
                                let rsrp = m[3].parse::<i32>().unwrap_or(0) - 140;
                                let sinr = (m[4].parse::<f32>().unwrap_or(0.0) * 0.2) - 20.0;
                                let rsrq = (m[5].parse::<f32>().unwrap_or(0.0) * 0.5) - 19.5;
                                info.rssi = format!("{}", rssi);
                                info.rsrp = format!("{}", rsrp);
                                info.sinr = format!("{:.1}", sinr);
                                info.rsrq = format!("{:.1}", rsrq);
                            }

                            // HFREQINFO
                            let _ = s_tx.send("AT^HFREQINFO?\r\n".to_string());
                            let resp = wait_resp(&mut worker_rx, 2);
                            if let Some(m) = RE_HFREQINFO.captures(&resp) {
                                info.cell_band = format!("B{}", &m[1]);
                                info.dl_earfcn = m[2].to_string();
                                info.dl_freq = m[3].parse::<f32>().map(|v| format!("{:.1}", v / 10.0)).unwrap_or_else(|_| m[3].to_string());
                                info.dl_bw = m[4].parse::<f32>().map(|v| format!("{:.1}", v / 1000.0)).unwrap_or_else(|_| m[4].to_string());
                                info.ul_earfcn = m[5].to_string();
                                info.ul_freq = m[6].parse::<f32>().map(|v| format!("{:.1}", v / 10.0)).unwrap_or_else(|_| m[6].to_string());
                                info.ul_bw = m[7].parse::<f32>().map(|v| format!("{:.1}", v / 1000.0)).unwrap_or_else(|_| m[7].to_string());
                            }

                            // CREG
                            let _ = s_tx.send("AT+CREG?\r\n".to_string());
                            let resp = wait_resp(&mut worker_rx, 2);
                            if let Some(m) = RE_CREG.captures(&resp) {
                                let stat = &m[1];
                                info.net_reg = match stat {
                                    "0" => "Not reg".to_string(),
                                    "1" => "Registered (Home)".to_string(),
                                    "2" => "Searching...".to_string(),
                                    "3" => "Denied".to_string(),
                                    "5" => "Roaming".to_string(),
                                    _ => format!("Unknown ({})", stat),
                                };
                                if let Some(lac) = m.get(2) { info.tac_lac = lac.as_str().to_string(); }
                                if let Some(ci) = m.get(3) { info.cell_id = ci.as_str().to_string(); }
                            }

                            let _ = tx.send(AppEvent::ModemInfo(Box::new(info.clone())));
                        }

                        thread::sleep(Duration::from_secs(3));
                    }
                });

                self.log("Starting modem initialization...", "system");
                self.init_modem();
                true
            }
            Err(e) => {
                self.log(&format!("Connection failed: {}", e), "error");
                self.status_text = "Connection failed".to_string();
                self.status_color = Color32::RED;
                false
            }
        }
    }

    // ─── Disconnect ───
    pub(crate) fn disconnect(&mut self) {
        self.log(&format!("Disconnecting from {}...", self.connected_port.clone().unwrap_or_else(|| "unknown".to_string())), "system");
        self.connected = false;
        self.mode_detection_pending = false;
        self.reader_running = false;
        if let Some(flag) = &self.reader_flag {
            if let Ok(mut running) = flag.lock() {
                *running = false;
            }
        }
        self.serial_port = None;
        self.serial_tx = None;
        self.response_rx = None;
        if let Some(handle) = self.reader_thread.take() {
            let _ = handle.join();
        }
        self.reader_flag = None;
        self.status_text = "Disconnected".to_string();
        self.status_color = Color32::RED;
        self.signal = "---".to_string();
        self.operator = "---".to_string();
        self.network = "---".to_string();
        self.log("Disconnected", "system");
    }

    // ─── CNMI setup ───
    pub(crate) fn setup_cnmi_async(&self) {
        let tx = self.serial_tx.clone();
        thread::spawn(move || {
            if let Some(tx) = tx {
                // We don't wait for response here to keep it non-blocking,
                // just fire the best candidates. The modem will respond with OK/ERROR.
                let _ = tx.send("AT+CNMI=2,2,0,0,0\r\n".to_string());
                thread::sleep(Duration::from_millis(200));
                let _ = tx.send("AT+CNMI?\r\n".to_string());
            }
        });
    }

    // ─── Modem info ───
    pub(crate) fn get_modem_info_async(&self) {
        if !self.connected { return; }
        if let Some(ref tx) = self.serial_tx {
            let _ = tx.send("AT+CSQ\r\n".to_string());
            let _ = tx.send("AT+COPS=3,0\r\n".to_string());
            let _ = tx.send("AT+COPS?\r\n".to_string());
            let _ = tx.send("AT^SYSINFOEX\r\n".to_string());
            let _ = tx.send("AT^HCSQ?\r\n".to_string());
            let _ = tx.send("AT^HFREQINFO?\r\n".to_string());
            let _ = tx.send("AT+CREG?\r\n".to_string());
        }
    }

    // ─── COM Port refresh ───
    pub(crate) fn refresh_com_ports(&mut self, silent: bool) {
        if !silent { self.log("Refreshing COM ports...", "system"); }
        let tx = self.app_event_tx.clone();
        let previous_device = self.get_manual_port_device();
        let connected_device = self.connected_port.clone().unwrap_or_default();
        
        thread::spawn(move || {
            let ports = serialport::available_ports().unwrap_or_default();
            let friendly_names = Self::windows_serial_friendly_names();
            let mut label_list = Vec::new();
            let mut label_map = HashMap::new();
            let mut connected_label = String::new();
            let mut fc_pc_ui_label = String::new();
            let mut previous_label = String::new();

            for p in &ports {
                let desc = if let Some(name) = friendly_names.get(&p.port_name) {
                    name.clone()
                } else {
                    match &p.port_type {
                        serialport::SerialPortType::UsbPort(info) => format!(
                            "{} {} {}",
                            info.manufacturer.clone().unwrap_or_default(),
                            info.product.clone().unwrap_or_default(),
                            info.serial_number.clone().unwrap_or_default()
                        ),
                        _ => "Unknown device".to_string(),
                    }
                };

                let label = desc.clone();
                
                let label_lower = label.to_lowercase();
                if Self::port_matches_profile(&label_lower, "Project Mode")
                    && fc_pc_ui_label.is_empty()
                {
                    fc_pc_ui_label = label.clone();
                }
                if !connected_device.is_empty() && p.port_name == connected_device {
                    connected_label = label.clone();
                }
                if !previous_device.is_empty() && p.port_name == previous_device {
                    previous_label = label.clone();
                }
                label_map.insert(label.clone(), p.port_name.clone());
                label_list.push(label);
            }

            let profile = Self::detect_serial_profile_static();

            let manual_port = if !connected_label.is_empty() {
                connected_label
            } else if !fc_pc_ui_label.is_empty() {
                fc_pc_ui_label
            } else if !previous_label.is_empty() {
                previous_label
            } else {
                label_list.first().cloned().unwrap_or_default()
            };

            let _ = tx.send(AppEvent::ComPorts(label_list, manual_port, profile, label_map));
            if !silent {
                let _ = tx.send(AppEvent::Log(format!("Found {} COM port(s)", ports.len()), "system".to_string()));
            }
        });
    }

    // ─── Get manual port device ───
    pub(crate) fn get_manual_port_device(&self) -> String {
        let label = &self.manual_port;
        if let Some(port) = self.port_label_map.get(label) {
            return port.clone();
        }

        RE_COM_PORT
            .find(label)
            .map(|m| m.as_str().to_uppercase())
            .unwrap_or_default()
    }

    // ─── Serial profile label ───
    fn serial_profile_label_for_ports(ports: &[String], fallback: &str) -> String {
        let mut has_debug = false;
        let mut has_project = false;

        for label in ports {
            let label_lower = label.to_lowercase();
            if Self::port_matches_profile(&label_lower, "Debug Mode") {
                has_debug = true;
            }
            if Self::port_matches_profile(&label_lower, "Project Mode") {
                has_project = true;
            }
        }

        if has_debug {
            "Debug Mode".to_string()
        } else if has_project {
            "Project Mode".to_string()
        } else {
            fallback.to_string()
        }
    }

    fn update_serial_profile_label(&mut self) {
        self.displayed_serial_profile =
            Self::serial_profile_label_for_ports(&self.com_ports, &self.serial_profile);
    }

    pub(crate) fn serial_profile_label(&self) -> &str {
        if !self.connected {
            return "Unknown";
        }
        &self.displayed_serial_profile
    }

    // ─── Port description ───
    fn port_description_static(
        p: &serialport::SerialPortInfo,
        friendly_names: &HashMap<String, String>,
    ) -> String {
        if let Some(name) = friendly_names.get(&p.port_name) {
            return name.clone();
        }
        match &p.port_type {
            serialport::SerialPortType::UsbPort(info) => format!(
                "{} {} {}",
                info.manufacturer.clone().unwrap_or_default(),
                info.product.clone().unwrap_or_default(),
                info.serial_number.clone().unwrap_or_default()
            ),
            _ => "Unknown device".to_string(),
        }
    }


    // ─── Port matches profile ───
    fn port_matches_profile(desc_lower: &str, profile: &str) -> bool {
        match profile {
            "Project Mode" => {
                desc_lower.contains("fc - pc ui")
                    || desc_lower.contains("fc pc ui")
                    || desc_lower.contains("pc ui")
                    || desc_lower.contains("pcui")
                    || desc_lower.contains("application interface")
                    || desc_lower.contains("huawei mobile connect")
            }
            "Debug Mode" => {
                desc_lower.contains("seriala")
                    || desc_lower.contains("serial a")
                    || desc_lower.contains("serialb")
                    || desc_lower.contains("serial b")
                    || desc_lower.contains("serialc")
                    || desc_lower.contains("serial c")
                    || desc_lower.contains("shalla")
                    || desc_lower.contains("shall a")
                    || desc_lower.contains("shallb")
                    || desc_lower.contains("shall b")
            }
            _ => false,
        }
    }

    // ─── Windows serial friendly names ───
    fn windows_serial_friendly_names() -> HashMap<String, String> {
        let mut map = HashMap::new();
        let output = Command::new("powershell")
            .creation_flags(0x08000000)
            .args([
                "-NoProfile",
                "-Command",
                "Get-CimInstance Win32_PnPEntity | Where-Object { $_.Name -match 'COM\\d+' } | ForEach-Object { $_.Name }",
            ])
            .output();
        let Ok(output) = output else {
            return map;
        };
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
            if let Some(cap) = RE_FRIENDLY_COM.captures(line) {
                map.insert(cap[1].to_string(), line.to_string());
            }
        }
        map
    }

    // ─── Detect serial profile ───
    fn detect_serial_profile_static() -> String {
        let ports = serialport::available_ports().unwrap_or_default();
        let friendly_names = Self::windows_serial_friendly_names();
        let mut has_project = false;
        let mut has_debug = false;
        for p in ports {
            let desc = Self::port_description_static(&p, &friendly_names).to_lowercase();
            if Self::port_matches_profile(&desc, "Project Mode") {
                has_project = true;
            }
            if Self::port_matches_profile(&desc, "Debug Mode") {
                has_debug = true;
            }
        }
        if has_debug {
            "Debug Mode".to_string()
        } else if has_project {
            "Project Mode".to_string()
        } else {
            "Unknown".to_string()
        }
    }

    // ─── Detect port profile ───
    fn detect_port_profile_static(port_name: &str) -> String {
        let friendly_names = Self::windows_serial_friendly_names();
        let ports = serialport::available_ports().unwrap_or_default();
        let Some(port) = ports.iter().find(|p| p.port_name == port_name) else {
            return "Unknown".to_string();
        };
        let desc = Self::port_description_static(port, &friendly_names).to_lowercase();
        if Self::port_matches_profile(&desc, "Debug Mode") {
            "Debug Mode".to_string()
        } else if Self::port_matches_profile(&desc, "Project Mode") {
            "Project Mode".to_string()
        } else {
            "Unknown".to_string()
        }
    }

    // ─── Remote NDIS adapter ───
    pub(crate) fn remote_ndis_adapter_available() -> bool {
        let output = Command::new("powershell")
            .creation_flags(0x08000000)
            .args([
                "-NoProfile",
                "-Command",
                "Get-NetAdapter | Where-Object { $_.InterfaceDescription -match 'RNDIS|Remote NDIS' -or $_.Name -match 'RNDIS|Remote NDIS' } | Select-Object -First 1",
            ])
            .output();
        let Ok(output) = output else {
            return false;
        };
        !String::from_utf8_lossy(&output.stdout).trim().is_empty()
    }

    // ─── Refresh RNDIS status ───
    fn refresh_rndis_status_async(&self) {
        let tx = self.app_event_tx.clone();
        thread::spawn(move || {
            let rndis_adapter = Self::remote_ndis_adapter_available();
            let rndis_status = if rndis_adapter { "Available" } else { "Not Available" };
            let _ = tx.send(AppEvent::RndisStatus(rndis_status.to_string()));
        });
    }

    // ─── Detect modem mode async ───
    pub(crate) fn detect_modem_mode_async(&mut self) {
        if self.mode_detection_pending || self.connection_in_progress {
            return;
        }
        self.mode_detection_pending = true;
        self.log("Detecting modem mode...", "system");
        self.last_mode_detection = Instant::now();
        let tx = self.app_event_tx.clone();
        let ip = if self.modem_ip.trim().is_empty() {
            "192.168.8.1".to_string()
        } else {
            self.modem_ip.trim().to_string()
        };
        let is_connected = self.connected;

        thread::spawn(move || {
            let mut hilink_active = false;
            if Self::hilink_alive_static(&ip, &tx) {
                let _ = tx.send(AppEvent::RndisStatus("Available".to_string()));
                if !is_connected {
                    let _ = tx.send(AppEvent::Mode("HiLink".to_string()));
                }
                hilink_active = true;
                let _ = tx.send(AppEvent::Log("HiLink is reachable".to_string(), "system".to_string()));
            } else {
                let rndis_adapter = Self::remote_ndis_adapter_available();
                let rndis_status = if rndis_adapter { "Available".to_string() } else { "Not Available".to_string() };
                let _ = tx.send(AppEvent::RndisStatus(rndis_status));
                let _ = tx.send(AppEvent::Log("Modem web server not responding yet, checking serial ports...".to_string(), "system".to_string()));
            }

            if !is_connected && !hilink_active {
                let profile = Self::detect_serial_profile_static();
                if profile != "Unknown" {
                    let _ = tx.send(AppEvent::Mode("Serial".to_string()));
                    let _ = tx.send(AppEvent::Log(format!("Mode detection complete: {}", profile), "system".to_string()));
                    let _ = tx.send(AppEvent::SerialProfile(profile));
                } else {
                    let _ = tx.send(AppEvent::Mode("Unknown".to_string()));
                    let _ = tx.send(AppEvent::Log(format!("Mode detection complete: {}", profile), "system".to_string()));
                    let _ = tx.send(AppEvent::SerialProfile("Unknown".to_string()));
                }
            } else {
                // If connected or HiLink, we still update profile label but don't probe
                let profile = Self::detect_serial_profile_static();
                let _ = tx.send(AppEvent::Log(format!("Profile updated (connected): {}", profile), "system".to_string()));
                let _ = tx.send(AppEvent::SerialProfile(profile));
                // Clear pending flag without changing mode
                let _ = tx.send(AppEvent::StatusPendingCleared);
            }
        });
    }

    // ─── Smart connect ───
    pub(crate) fn smart_connect(&mut self, _preferred_port: Option<String>) {
        if self.connected {
            self.log("Already connected", "system");
            return;
        }

        self.log("Smart connect started", "system");

        let switch_mode = self.switch_mode.clone();
        let ip = if self.modem_ip.trim().is_empty() {
            "192.168.8.1".to_string()
        } else {
            self.modem_ip.trim().to_string()
        };
        let modem_user = self.modem_user.clone();
        let modem_pass = self.modem_pass.clone();
        let baud = self.manual_baud.parse::<u32>().unwrap_or(9600);
        let manual_port = self.get_manual_port_device();
        let current_profile = self.serial_profile_label().to_string();
        let tx = self.app_event_tx.clone();

        self.status_text = "Connecting...".to_string();
        self.status_color = Color32::from_rgb(255, 165, 0);
        self.connection_in_progress = true;

        thread::spawn(move || {
            let cleanup = || { let _ = tx.send(AppEvent::ConnectionFinished); };

            if Self::remote_ndis_adapter_available() {
                let detected = Self::detect_serial_profile_static();
                let _ = tx.send(AppEvent::Log(format!("RNDIS available, detected profile: {}", detected), "system".to_string()));
                if detected == switch_mode {
                    let _ = tx.send(AppEvent::Log(format!("Already in {} mode — connecting directly", switch_mode), "system".to_string()));
                    let ports = serialport::available_ports().unwrap_or_default();
                    let friendly = Self::windows_serial_friendly_names();
                    for p in &ports {
                        let desc = Self::port_description_static(p, &friendly).to_lowercase();
                        if desc.contains("fc - pc ui") || desc.contains("pc ui") {
                            let _ = tx.send(AppEvent::Log(format!("Found {} port {}", switch_mode, p.port_name), "system".to_string()));
                            let _ = tx.send(AppEvent::ConnectPort(p.port_name.clone(), baud));
                            cleanup();
                            return;
                        }
                    }
                    let _ = tx.send(AppEvent::Log("Port not found. Select the port manually in Settings and use Connect Directly.".to_string(), "error".to_string()));
                    cleanup();
                    return;
                }
                let _ = tx.send(AppEvent::Log(format!("Modem is in {} mode, switching to {}...", detected, switch_mode), "system".to_string()));
                Self::do_hilink_switch_static(&ip, &modem_user, &modem_pass, &switch_mode, &tx);
                let _ = tx.send(AppEvent::Log("Switch succeeded, waiting for FC - PC UI Interface...".to_string(), "system".to_string()));
                Self::wait_for_fc_pc_ui(&tx, baud);
                cleanup();
                return;
            }

            if current_profile != switch_mode {
                let _ = tx.send(AppEvent::Log(
                    format!("Detected profile: {}, desired: {} — switching modes...", current_profile, switch_mode),
                    "system".to_string(),
                ));
                let ports = serialport::available_ports().unwrap_or_default();
                let friendly = Self::windows_serial_friendly_names();
                let mut reset_port = None;
                for p in &ports {
                    let desc = Self::port_description_static(p, &friendly).to_lowercase();
                    if desc.contains("fc - pc ui") || desc.contains("pc ui") {
                        reset_port = Some(p.port_name.clone());
                        break;
                    }
                }
                if let Some(port) = reset_port {
                    if let Ok(mut serial) = serialport::new(&port, baud)
            .timeout(Duration::from_secs(5))
                        .open()
                    {
                        let _ = serial.write_all(b"AT^RESET\r\n");
                        let _ = serial.flush();
                        let _ = tx.send(AppEvent::Log("AT^RESET sent, waiting for modem to reboot to HiLink...".to_string(), "system".to_string()));

                        for _ in 0..20 {
                            thread::sleep(Duration::from_secs(3));
                            if Self::hilink_alive_static(&ip, &tx) {
                                let _ = tx.send(AppEvent::Log("HiLink is reachable, sending switch command...".to_string(), "system".to_string()));
                                break;
                            }
                        }
                    }
                }
                let mut switched = false;
                for attempt in 1..=3 {
                    let _ = tx.send(AppEvent::Log(format!("Switch to {} attempt {}/3", switch_mode, attempt), "system".to_string()));
                    if Self::do_hilink_switch_static(&ip, &modem_user, &modem_pass, &switch_mode, &tx) {
                        switched = true;
                        break;
                    }
                    if attempt < 3 {
                        thread::sleep(Duration::from_secs(3));
                    }
                }
                if switched {
                    let _ = tx.send(AppEvent::Log("Switch succeeded, waiting for FC - PC UI Interface...".to_string(), "system".to_string()));
                    Self::wait_for_fc_pc_ui(&tx, baud);
                } else {
                    let _ = tx.send(AppEvent::Log("Switch to Debug Mode failed after 3 attempts. Try again or use Switch to Serial Mode from menu.".to_string(), "error".to_string()));
                }
                cleanup();
                return;
            }

            if manual_port.is_empty() {
                let _ = tx.send(AppEvent::Log("No COM port selected in Settings".to_string(), "error".to_string()));
                cleanup();
                return;
            }
            let _ = tx.send(AppEvent::Log(format!("Connecting to {} at {} baud...", manual_port, baud), "system".to_string()));
            let _ = tx.send(AppEvent::ConnectPort(manual_port, baud));
            cleanup();
        });
    }

    fn wait_for_fc_pc_ui(tx: &mpsc::Sender<AppEvent>, baud: u32) {
        let _ = tx.send(AppEvent::Log("Waiting for FC - PC UI Interface to appear...".to_string(), "system".to_string()));
        for _ in 0..30 {
            thread::sleep(Duration::from_secs(2));
            let ports = serialport::available_ports().unwrap_or_default();
            let friendly = Self::windows_serial_friendly_names();
            for p in &ports {
                let desc = Self::port_description_static(p, &friendly).to_lowercase();
                if desc.contains("fc - pc ui") || desc.contains("pc ui") {
                    let _ = tx.send(AppEvent::Log(format!("Found FC - PC UI Interface on {}", p.port_name), "system".to_string()));
                    let _ = tx.send(AppEvent::ConnectPort(p.port_name.clone(), baud));
                    return;
                }
            }
        }
        let _ = tx.send(AppEvent::Log("FC - PC UI Interface not found. Select the port manually in Settings and use Connect Directly.".to_string(), "error".to_string()));
    }

    // ─── HiLink alive ───
    fn hilink_alive_static(ip: &str, tx: &mpsc::Sender<AppEvent>) -> bool {
        let client = match reqwest::blocking::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(2))
            .build()
        {
            Ok(client) => client,
            Err(e) => {
                let _ = tx.send(AppEvent::Log(
                    format!("HiLink HTTP client error: {}", e),
                    "raw".to_string(),
                ));
                return false;
            }
        };
        if let Ok(resp) = client
            .get(format!("http://{}/api/device/information", ip))
            .send()
        {
            if matches!(resp.status().as_u16(), 200 | 401) {
                return true;
            }
        }
        if let Ok(resp) = client.get(format!("http://{}/", ip)).send() {
            if matches!(resp.status().as_u16(), 200 | 302 | 401) {
                return true;
            }
        }
        false
    }

    // ─── Auto-detect modem IP ───
    pub(crate) fn autodetect_modem_ip(&mut self) {
        let candidates = [
            "192.168.8.1",
            "192.168.9.1",
            "192.168.1.1",
            "192.168.0.1",
            "10.0.0.138",
        ];
        let client = reqwest::blocking::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(2))
            .build();
        let Ok(client) = client else {
            self.log("Auto-detect: failed to create HTTP client", "error");
            return;
        };
        for ip in candidates {
            let info_url = format!("http://{}/api/device/information", ip);
            if let Ok(resp) = client.get(&info_url).send() {
                let status_ok = resp.status().is_success();
                let text = resp.text().unwrap_or_default();
                if status_ok
                    && (text.contains("Huawei")
                        || text.contains("DeviceName")
                        || text.contains("<Response>"))
                {
                    self.modem_ip = ip.to_string();
                    self.log(&format!("Auto-detect: modem found at {}", ip), "system");
                    return;
                }
            }
            if let Ok(resp) = client.get(format!("http://{}/", ip)).send() {
                if matches!(resp.status().as_u16(), 200 | 302 | 401) {
                    self.modem_ip = ip.to_string();
                    self.log(&format!("Auto-detect: possible modem at {}", ip), "system");
                    return;
                }
            }
        }
        self.log("Auto-detect: modem not found on common IPs", "error");
    }

    // ─── HiLink switch ───
    fn do_hilink_switch_static(
        ip: &str,
        modem_user: &str,
        modem_pass: &str,
        switch_mode: &str,
        tx: &mpsc::Sender<AppEvent>,
    ) -> bool {
        let client = match reqwest::blocking::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(client) => client,
            Err(e) => {
                let _ = tx.send(AppEvent::Log(
                    format!("HiLink switch unexpected error: {}", e),
                    "raw".to_string(),
                ));
                return false;
            }
        };
        let debug_mode = switch_mode == "Debug Mode";
        let switch_page = if debug_mode {
            "switchDebugMode.html"
        } else {
            "switchProjectMode.html"
        };
        let switch_type = if debug_mode { "1" } else { "0" };
        let _ = tx.send(AppEvent::Log(
            format!(
                "Requesting {} via {} (switchType={})",
                switch_mode, switch_page, switch_type
            ),
            "system".to_string(),
        ));
        let switch_url = format!("http://{}/html/{}", ip, switch_page);
        let csrf = client
            .get(&switch_url)
            .basic_auth(modem_user.to_string(), Some(modem_pass.to_string()))
            .send()
            .ok()
            .and_then(|r| r.text().ok())
            .and_then(|text| RE_CSRF_TOKEN.captures(&text).map(|c| c[1].to_string()));
        let payload = format!(
            "<api version=\"1.0\"><header><function>switchMode</function></header><body><request><switchType>{}</switchType></request></body></api>",
            switch_type
        );
        let mut req = client
            .post(format!("http://{}/CGI", ip))
            .basic_auth(modem_user.to_string(), Some(modem_pass.to_string()))
            .header("Content-Type", "application/xml")
            .body(payload);
        if let Some(token) = csrf {
            req = req.header("__RequestVerificationToken", token);
        }
        match req.send() {
            Ok(_) => {
                let _ = tx.send(AppEvent::Log(
                    "Switch command accepted - modem is rebooting".to_string(),
                    "system".to_string(),
                ));
                true
            }
            Err(e) => {
                let _ = tx.send(AppEvent::Log(
                    "Switch command failed - modem may not be ready".to_string(),
                    "system".to_string(),
                ));
                let _ = tx.send(AppEvent::Log(
                    format!("Switch detail: {}", e),
                    "raw".to_string(),
                ));
                false
            }
        }
    }


    // ─── Switch mode action ───
    pub(crate) fn switch_mode_action(&mut self) {
        self.log("Switch mode action triggered", "system");
        if self.connected {
            let port = self
                .connected_port
                .clone()
                .unwrap_or_else(|| self.get_manual_port_device());
            if port.is_empty() {
                self.log("No connected serial COM port to switch to HiLink", "error");
                return;
            }
            self.status_text = "Switching selected port to HiLink...".to_string();
            self.status_color = Color32::from_rgb(255, 165, 0);
            self.disconnect();
            let baud = self.manual_baud.parse::<u32>().unwrap_or(9600);
            self.spawn_hilink_reset(port, baud);
        } else if self.detected_mode == "Serial"
            || self.serial_profile == "Project Mode"
            || self.serial_profile == "Debug Mode"
        {
            self.refresh_com_ports(true);
            let selected_port = self.get_manual_port_device();
            if selected_port.is_empty() {
                self.log("No selected serial COM port to switch to HiLink", "error");
                return;
            }
            self.status_text = "Switching selected port to HiLink...".to_string();
            self.status_color = Color32::from_rgb(255, 165, 0);
            let baud = self.manual_baud.parse::<u32>().unwrap_or(9600);
            self.spawn_hilink_reset(selected_port, baud);
        } else {
            self.log("Switching to serial mode...", "system");
            self.status_text = "Switching to serial mode...".to_string();
            self.status_color = Color32::from_rgb(255, 165, 0);
            let tx = self.app_event_tx.clone();
            let ip = if self.modem_ip.trim().is_empty() {
                "192.168.8.1".to_string()
            } else {
                self.modem_ip.trim().to_string()
            };
            let modem_user = self.modem_user.clone();
            let modem_pass = self.modem_pass.clone();
            let switch_mode = self.switch_mode.clone();
            thread::spawn(move || {
                let _ = tx.send(AppEvent::Log(
                    format!("Switching to {} via {}...", switch_mode, ip),
                    "system".to_string(),
                ));
                let mut switch_ok = false;
                for attempt in 1..=3 {
                    let _ = tx.send(AppEvent::Log(
                        format!("HiLink switch attempt {}/3...", attempt),
                        "system".to_string(),
                    ));
                    if Self::do_hilink_switch_static(&ip, &modem_user, &modem_pass, &switch_mode, &tx) {
                        switch_ok = true;
                        break;
                    }
                    if attempt < 3 {
                        thread::sleep(Duration::from_secs(3));
                    }
                }
                if switch_ok {
                    let _ = tx.send(AppEvent::Log(
                        "Switch command accepted. Use Connect Serial to connect after modem reboots.".to_string(),
                        "system".to_string(),
                    ));
                } else {
                    let _ = tx.send(AppEvent::Log(
                        "Switch command failed after 3 attempts".to_string(),
                        "error".to_string(),
                    ));
                }
            });
        }
    }

    // ─── Spawn HiLink reset ───
    fn spawn_hilink_reset(&self, port: String, baud: u32) {
        let tx = self.app_event_tx.clone();
        thread::spawn(move || {
            let _ = tx.send(AppEvent::Log(
                format!("Sending AT^RESET to selected port {}", port),
                "system".to_string(),
            ));
            match serialport::new(&port, baud)
                .timeout(Duration::from_secs(1))
                .open()
            {
                Ok(mut serial) => {
                    let _ = serial.write_all(b"AT^RESET\r\n");
                    let _ = serial.flush();
                    let _ = tx.send(AppEvent::Log(
                        "AT^RESET sent to selected serial port".to_string(),
                        "system".to_string(),
                    ));
                    let _ = tx.send(AppEvent::Log(
                        "Modem resetting to HiLink mode...".to_string(),
                        "system".to_string(),
                    ));
                    let _ = tx.send(AppEvent::Mode("HiLink".to_string()));
                    let _ = tx.send(AppEvent::SerialProfile("Unknown".to_string()));
                    let _ = tx.send(AppEvent::Status(
                        "Modem restart sent".to_string(),
                        Color32::GREEN,
                    ));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::Log(
                        format!("Failed to open selected port {}: {}", port, e),
                        "error".to_string(),
                    ));
                    let _ = tx.send(AppEvent::Status("Switch failed".to_string(), Color32::RED));
                }
            }
        });
    }

    // ─── Process UI serial ───
    pub(crate) fn process_ui_serial(&mut self) {
        let mut chunks = Vec::new();
        if let Some(ref rx) = self.ui_rx {
            while let Ok(chunk) = rx.try_recv() {
                chunks.push(chunk);
            }
        }

        for chunk in chunks {
            self.log(&format!("{:?}", chunk), "raw");
            if let Some(readable) = self.readable_raw(&chunk) {
                self.log(&readable, "info");
            }
            self.incoming_buffer.push_str(&chunk);
            while let Some(pos) = self.incoming_buffer.find('\n') {
                let line = self.incoming_buffer[..pos].trim().to_string();
                self.incoming_buffer = self.incoming_buffer[pos + 1..].to_string();
                self.process_serial_line(&line);
            }
        }
    }

    // ─── Process app events ───
    pub(crate) fn process_app_events(&mut self) {
        let mut events = Vec::new();
        while let Ok(event) = self.app_event_rx.try_recv() {
            events.push(event);
        }
        for event in events {
            match event {
                AppEvent::Log(msg, cat) => self.log(&msg, &cat),
                AppEvent::Status(text, color) => {
                    self.status_text = text;
                    self.status_color = color;
                }
                AppEvent::ConnectPort(port, baud) => {
                    self.status_text = "Connecting...".to_string();
                    self.status_color = Color32::from_rgb(255, 165, 0);
                    self.connect_serial(&port, baud);
                }
                AppEvent::Mode(mode) => {
                    if !self.connected {
                        self.detected_mode = mode;
                        if self.status_text.contains("Switching")
                            || self.status_text.contains("restart")
                            || self.status_text == "Disconnected"
                        {
                            match self.detected_mode.as_str() {
                                "Serial" => {
                                    self.status_text = "Serial mode detected (not connected)".to_string();
                                    self.status_color = Color32::LIGHT_BLUE;
                                }
                                "HiLink" => {
                                    self.status_text = "Not connected".to_string();
                                    self.status_color = Color32::RED;
                                }
                                _ => {
                                    self.status_text = "Disconnected".to_string();
                                    self.status_color = Color32::RED;
                                }
                            }
                        }
                    }
                    self.mode_detection_pending = false;
                }
                AppEvent::SerialProfile(profile) => {
                    if !self.connected {
                        self.serial_profile = profile;
                        self.update_serial_profile_label();
                    }
                }
                AppEvent::RndisStatus(status) => {
                    if status == "Available" {
                        self.refresh_com_ports(true);
                    }
                    self.log(&format!("RNDIS status: {}", status), "system");
                    self.rndis_status = status;
                }

                AppEvent::ModemInfo(info) => {
                    let info = *info;
                    self.signal = info.signal;
                    self.operator = info.operator;
                    self.network = info.network;
                    self.net_reg = info.net_reg;
                    self.tac_lac = info.tac_lac;
                    self.cell_id = info.cell_id;
                    self.net_tech = info.net_tech;
                    self.cell_band = info.cell_band;
                    self.dl_earfcn = info.dl_earfcn;
                    self.dl_freq = info.dl_freq;
                    self.dl_bw = info.dl_bw;
                    self.ul_earfcn = info.ul_earfcn;
                    self.ul_freq = info.ul_freq;
                    self.ul_bw = info.ul_bw;
                    self.rssi = info.rssi;
                    self.rsrp = info.rsrp;
                    self.sinr = info.sinr;
                    self.rsrq = info.rsrq;
                }
                AppEvent::ComPorts(ports, manual, profile, label_map) => {
                    self.com_ports = ports.clone();
                    if !manual.is_empty() || self.manual_port.is_empty() {
                        self.manual_port = manual;
                    }
                    if !self.connected {
                        self.serial_profile = profile;
                    }
                    self.port_label_map = label_map;
                    self.update_serial_profile_label();
                }
                AppEvent::HardwareChanged => {
                    self.log("Hardware change detected", "system");
                    self.refresh_com_ports(true);

                    if self.connected {
                        self.refresh_rndis_status_async();
                        if let Some(ref port_name) = self.connected_port {
                            let available = serialport::available_ports()
                                .unwrap_or_default()
                                .iter()
                                .any(|p| p.port_name == *port_name);
                            if !available {
                                self.log(&format!("Modem on {} was removed", port_name), "error");
                                self.disconnect();
                            }
                        }
                    } else {
                        self.detect_modem_mode_async();
                    }
                }
                AppEvent::StatusPendingCleared => {
                    self.mode_detection_pending = false;
                }
                AppEvent::ConnectionFinished => {
                    self.connection_in_progress = false;
                }
            }
        }
    }

    // ─── Modem init ───
    fn init_modem(&mut self) {
        self.send_at("AT+CREG=2", 3);
        self.send_at("AT+CGREG=2", 3);
        self.send_at("AT^HCSQ=1", 3);
        self.setup_cnmi_async();
        self.load_phonebook_local();
        self.get_modem_info_async();
    }

    fn parse_creg_urc(&mut self, line: &str) {
        if let Some(m) = RE_CREG.captures(line) {
            let stat = m[1].to_string();
            self.net_reg = match stat.as_str() {
                "0" => "Not reg".to_string(),
                "1" => "Registered (Home)".to_string(),
                "2" => "Searching...".to_string(),
                "3" => "Denied".to_string(),
                "5" => "Roaming".to_string(),
                _ => format!("Unknown ({})", stat),
            };
            self.tac_lac = m
                .get(2)
                .map(|v| v.as_str().to_string())
                .unwrap_or_else(|| "---".to_string());
            self.cell_id = m
                .get(3)
                .map(|v| v.as_str().to_string())
                .unwrap_or_else(|| "---".to_string());
        }
    }

    fn parse_hcsq_line(&mut self, line: &str) {
        if let Some(m) = RE_HCSQ.captures(line) {
            self.net_tech = m[1].to_string();
            let rssi = m[2].parse::<i32>().unwrap_or(0) - 120;
            let rsrp = m[3].parse::<i32>().unwrap_or(0) - 140;
            let sinr = (m[4].parse::<f32>().unwrap_or(0.0) * 0.2) - 20.0;
            let rsrq = (m[5].parse::<f32>().unwrap_or(0.0) * 0.5) - 19.5;
            self.rssi = format!("{}", rssi);
            self.rsrp = format!("{}", rsrp);
            self.sinr = format!("{:.1}", sinr);
            self.rsrq = format!("{:.1}", rsrq);
        }
    }

    fn parse_hfreqinfo_line(&mut self, line: &str) {
        if let Some(m) = RE_HFREQINFO.captures(line) {
            self.cell_band = format!("B{}", &m[1]);
            self.dl_earfcn = m[2].to_string();
            self.dl_freq = m[3]
                .parse::<f32>()
                .map(|v| format!("{:.1}", v / 10.0))
                .unwrap_or_else(|_| m[3].to_string());
            self.dl_bw = m[4]
                .parse::<f32>()
                .map(|v| format!("{:.1}", v / 1000.0))
                .unwrap_or_else(|_| m[4].to_string());
            self.ul_earfcn = m[5].to_string();
            self.ul_freq = m[6]
                .parse::<f32>()
                .map(|v| format!("{:.1}", v / 10.0))
                .unwrap_or_else(|_| m[6].to_string());
            self.ul_bw = m[7]
                .parse::<f32>()
                .map(|v| format!("{:.1}", v / 1000.0))
                .unwrap_or_else(|_| m[7].to_string());
        }
    }

    // ─── Process serial line ───
    fn process_serial_line(&mut self, line: &str) {
        if line.is_empty() {
            return;
        }
        // Log every line to raw buffer for debugging
        self.log(line, "raw");

        if line.contains("+CDS:") {
            self.log("Delivery report received", "sms");
            self.handle_cds_response(line);
        } else if line.contains("+CDSI:") {
            self.log("Delivery notification stored by modem", "sms");
            self.handle_cdsi(line);
        } else if line.contains("+CMTI:") {
            self.log("New SMS notification (index)", "sms");
            self.handle_cmti(line);
        } else if line.contains("+CMT:") {
            self.log("Incoming SMS (direct PDU)", "sms");
            self.expecting_cmt_pdu = true;
        } else if line.contains("+CMGR:") {
             self.log("CMGR header detected", "system");
             self.expecting_cmgr_pdu = true;
        } else if line.starts_with("^HFREQINFO:") {
            self.parse_hfreqinfo_line(line);
        } else if line.starts_with("^HCSQ:") {
            self.parse_hcsq_line(line);
        } else if line.starts_with("+CREG:") || line.starts_with("+CGREG:") {
            self.parse_creg_urc(line);
        } else if line.contains("+CSQ:") {
             if let Some(m) = RE_CSQ.captures(line) {
                if let Ok(csq) = m[1].parse::<i32>() {
                    if csq == 99 {
                        self.signal = "No signal".to_string();
                    } else {
                        let bars = std::cmp::min(5, csq / 6);
                        self.signal = format!("{}/5 ({})", bars, csq);
                    }
                }
            }
        } else if line.contains("+CUSD:") {
            self.handle_cusd_urc(line);
        } else if line.contains("+COPS:") {
             if let Some(m) = RE_COPS_QUOTED.captures(line) {
                self.operator = map_operator(&m[1]);
            } else if let Some(m) = RE_COPS_UNQUOTED.captures(line) {
                self.operator = map_operator(&m[1]);
            }
        } else if line.contains("^SYSINFOEX:") {
             if let Some(m) = RE_SYSINFOEX.captures(line) {
                self.network = m[1].to_string();
            }
        } else if RE_HEX_LINE.is_match(line) && line.len() > 10
        {
            if self.expecting_cds_pdu {
                self.log("Identified CDS PDU", "system");
                self.expecting_cds_pdu = false;
                self.parse_delivery_report(line);
            } else if self.expecting_cmt_pdu {
                self.log("Identified CMT PDU", "system");
                self.expecting_cmt_pdu = false;
                self.handle_direct_sms(line);
            } else if self.expecting_cmgr_pdu {
                self.log("Identified CMGR PDU", "system");
                self.expecting_cmgr_pdu = false;
                self.handle_direct_sms(line);
            } else {
                self.log("Received unsolicited hex line (no pending SMS read)", "raw");
            }
        }
    }
}
