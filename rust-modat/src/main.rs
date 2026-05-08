#![windows_subsystem = "windows"]
mod modem;
mod patterns;
pub mod scheduled;
mod serial;
mod sms;
mod storage;
mod ui;
pub mod ussd;

use base64::{engine::general_purpose, Engine as _};
use eframe::egui;
use egui::Color32;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use serial::SerialPortWrapper;

// â”€â”€â”€ Data structures â”€â”€â”€
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Contact {
    name: String,
    number: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InboxMessage {
    index: usize,
    status: String,
    pdu: String,
    phone: String,
    timestamp: String,
    unread: bool,
    #[serde(default)]
    pre_decoded: Option<String>,
}

#[derive(Debug, Clone)]
struct LogEntry {
    timestamp: String,
    category: String,
    message: String,
}

#[derive(Debug, Clone)]
struct DeliveryRecord {
    mr: u32,
    phone: String,
    msg_type: String,
    status: String,
    sent: String,
    updated: String,
    content: String,
    detail: String,
    tag: String,
}

#[derive(Debug, Clone)]
struct SentMessageInfo {
    msg_type: String,
    phone: String,
    content: String,
}

struct PendingSend {
    phone: String,
    pdu: String,
    length: usize,
    msg_type: String,
    content: String,
}

pub(crate) struct ConcatParts {
    #[allow(dead_code)]
    total: u8,
    parts: Vec<Option<String>>,
    phone: String,
    timestamp: String,
    index: usize,
    status: String,
    unread: bool,
    pdu: String,
}



#[derive(Debug, Clone, Default)]
pub struct FullModemInfo {
    pub signal: String,
    pub operator: String,
    pub network: String,
    pub net_reg: String,
    pub tac_lac: String,
    pub cell_id: String,
    pub net_tech: String,
    pub cell_band: String,
    pub dl_earfcn: String,
    pub dl_freq: String,
    pub dl_bw: String,
    pub ul_earfcn: String,
    pub ul_freq: String,
    pub ul_bw: String,
    pub rssi: String,
    pub rsrp: String,
    pub sinr: String,
    pub rsrq: String,
}

enum AppEvent {
    Log(String, String),
    Status(String, Color32),
    #[allow(dead_code)]
    ConnectPort(String, u32),
    Mode(String),
    SerialProfile(String),
    RndisStatus(String),
    ModemInfo(Box<FullModemInfo>),
    ComPorts(Vec<String>, String, String, HashMap<String, String>), // Ports list, manual_port label, serial_profile, label->device
    HardwareChanged,
    StatusPendingCleared,
    #[allow(dead_code)]
    ConnectionFinished,
}

// â”€â”€â”€ Config structures â”€â”€â”€
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    geometry: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerialConfig {
    manual_port: String,
    baud: String,
    bypass_autodetect: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SmsConfig {
    phone: String,
    sms_class: String,
    dcs: String,
    delivery_report: String,
    log_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NetworkConfig {
    modem_ip: String,
    username: String,
    password: String,
    switch_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppSettingsFile {
    app: AppConfig,
    serial: SerialConfig,
    sms: SmsConfig,
    network: NetworkConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UssdBookmarkEntry {
    pub(crate) name: String,
    pub(crate) code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UssdBookmarkGroup {
    pub(crate) operator: String,
    pub(crate) bookmarks: Vec<UssdBookmarkEntry>,
}

impl Default for AppSettingsFile {
    fn default() -> Self {
        Self {
            app: AppConfig {
                geometry: "850x720".to_string(),
            },
            serial: SerialConfig {
                manual_port: "".to_string(),
                baud: "9600".to_string(),
                bypass_autodetect: "false".to_string(),
            },
            sms: SmsConfig {
                phone: "".to_string(),
                sms_class: "0 (Flash)".to_string(),
                dcs: "0x50 (Class 0 - 7bit) [OK]".to_string(),
                delivery_report: "true".to_string(),
                log_mode: "system".to_string(),
            },
            network: NetworkConfig {
                modem_ip: "192.168.8.1".to_string(),
                username: "admin".to_string(),
                password: "".to_string(),
                switch_mode: "Debug Mode".to_string(),
            },
        }
    }
}

#[cfg(windows)]
fn protect_secret(plain: &str) -> String {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};

    if plain.is_empty() || plain.starts_with("dpapi:") {
        return plain.to_string();
    }

    let mut bytes = plain.as_bytes().to_vec();
        let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_mut_ptr(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    unsafe {
        if CryptProtectData(&input, None, None, None, None, 0, &mut output).is_ok() {
            let protected = std::slice::from_raw_parts(output.pbData, output.cbData as usize);
            let encoded = general_purpose::STANDARD.encode(protected);
            let _ = LocalFree(HLOCAL(output.pbData as *mut _));
            format!("dpapi:{}", encoded)
        } else {
            plain.to_string()
        }
    }
}

#[cfg(windows)]
fn unprotect_secret(value: &str) -> String {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    let Some(encoded) = value.strip_prefix("dpapi:") else {
        return value.to_string();
    };
    let Ok(mut bytes) = general_purpose::STANDARD.decode(encoded) else {
        return String::new();
    };
        let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_mut_ptr(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    unsafe {
        if CryptUnprotectData(&input, None, None, None, None, 0, &mut output).is_ok() {
            let plain = std::slice::from_raw_parts(output.pbData, output.cbData as usize);
            let result = String::from_utf8_lossy(plain).to_string();
            let _ = LocalFree(HLOCAL(output.pbData as *mut _));
            result
        } else {
            String::new()
        }
    }
}

#[cfg(not(windows))]
fn protect_secret(plain: &str) -> String {
    plain.to_string()
}

#[cfg(not(windows))]
fn unprotect_secret(value: &str) -> String {
    value.to_string()
}

// â”€â”€â”€ Main Application State â”€â”€â”€
struct ModAtApp {
    // Connection state
    connected: bool,
    connection_in_progress: bool,
    detected_mode: String,
    serial_profile: String,
    displayed_serial_profile: String,
    rndis_status: String,
    mode_detection_pending: bool,
    last_mode_detection: Instant,
    status_text: String,
    status_color: Color32,

    // Serial state
    serial_port: Option<Arc<Mutex<SerialPortWrapper>>>,
    reader_running: bool,
    expecting_cds_pdu: bool,
    expecting_cmt_pdu: bool,
    expecting_cmgr_pdu: bool,
    expecting_cmgl_pdu: bool,
    pending_cmgr_index: Option<usize>,
    pending_cmgl_index: Option<usize>,
    pending_cmgl_status: String,
    cmgr_read_queue: VecDeque<usize>,
    waiting_cpms_for_sms: bool,
    last_inbox_poll: Instant,
    connected_port: Option<String>,

    // Thread communication
    serial_tx: Option<mpsc::Sender<String>>,
    response_rx: Option<Arc<Mutex<mpsc::Receiver<String>>>>,
    reader_thread: Option<std::thread::JoinHandle<()>>,
    ui_rx: Option<mpsc::Receiver<String>>,
    reader_flag: Option<Arc<Mutex<bool>>>,
    app_event_tx: mpsc::Sender<AppEvent>,
    app_event_rx: mpsc::Receiver<AppEvent>,

    // UI state - SMS tab
    phone_number: String,
    message_text: String,
    sms_class: usize,
    sms_class_options: Vec<String>,
    dcs_value: String,
    dcs_options: Vec<String>,
    delivery_report: bool,
    log_mode: String,
    hide_status_logs: bool,
    char_count: String,

    // UI state - Info
    signal: String,
    operator: String,
    network: String,

    // UI state - Network Info tab
    net_reg: String,
    tac_lac: String,
    cell_id: String,
    net_tech: String,
    cell_band: String,
    dl_earfcn: String,
    dl_freq: String,
    dl_bw: String,
    ul_earfcn: String,
    ul_freq: String,
    ul_bw: String,
    rssi: String,
    rsrp: String,
    sinr: String,
    rsrq: String,

    // UI state - Inbox tab
    inbox_messages: Vec<InboxMessage>,
    inbox_display_items: Vec<(usize, String)>,
    current_inbox_msg: Option<InboxMessage>,
    inbox_view_mode: String,
    inbox_selected: Option<usize>,

    // UI state - Phonebook tab
    phonebook_data: Vec<Contact>,
    phonebook_selected: Option<usize>,

    // UI state - Delivery Reports tab
    dr_records: Vec<DeliveryRecord>,
    dr_selected: Option<usize>,
    dr_detail_text: String,

    // UI state - Settings tab
    modem_ip: String,
    modem_user: String,
    modem_pass: String,
    switch_mode: String,
    manual_port: String,
    manual_baud: String,
    manual_bypass: bool,
    port_label_map: HashMap<String, String>,
    com_ports: Vec<String>,

    // UI state - AT Terminal tab
    at_command: String,
    at_output: String,
    at_history: Vec<String>,
    at_hist_idx: i32,

    // UI state - USSD tab
    ussd_input: String,
    ussd_response: String,
    ussd_raw_response: String,
    ussd_buttons: Vec<String>,
    ussd_active: bool,
    ussd_dcs: String,
    ussd_view_raw: bool,
    ussd_plain_text: bool,
    ussd_console: String,
    ussd_history: Vec<String>,
    ussd_bookmarks: Vec<UssdBookmarkGroup>,
    ussd_bookmarks_file: PathBuf,
    ussd_bookmarks_open: bool,

    // Tab selection
    current_tab: usize,
    tab_names: Vec<String>,

    // Internal state
    sent_messages: HashMap<u32, SentMessageInfo>,
    concat_pending: HashMap<(String, u16), ConcatParts>,
    raw_log_entries: VecDeque<LogEntry>,
    persistent_log_entries: VecDeque<LogEntry>,
    info_log_entries: VecDeque<LogEntry>,
    max_raw_log_entries: usize,
    max_persistent_log_entries: usize,
    max_info_log_entries: usize,
    log_paused: bool,
    log_cache_dirty: bool,
    cached_filtered_log: Vec<LogEntry>,
    log_scrolled_to_bottom: bool,
    log_usage_bytes: usize,
    max_log_bytes: usize,
    incoming_buffer: String,
    serial_busy: Arc<Mutex<bool>>,

    // File paths
    contacts_file: PathBuf,
    settings_file: PathBuf,
    inbox_file: PathBuf,

    // Settings
    cfg: AppSettingsFile,

    // Dialogs
    show_add_contact: bool,
    contact_name_input: String,
    contact_number_input: String,
    warning_message: Option<String>,
    settings_saved: Option<f32>,

    // Scheduled SMS
    scheduled_messages: Vec<scheduled::ScheduledSms>,
    scheduled_file: PathBuf,
    show_add_schedule: bool,
    sched_recipient_list: Vec<String>,
    sched_recipient_input: String,
    sched_show_phonebook_dropdown: bool,
    sched_message: String,
    sched_date: String,
    sched_time: String,
    sched_repeat_input: String,
    sched_repeat_unit: u8,
    sched_end_time: String,
    sched_flash_sms: bool,
    next_schedule_id: u64,
}

impl ModAtApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let base_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        let contacts_file = base_dir.join("contacts.json");
        let settings_file = base_dir.join("settings.ini");
        let inbox_file = base_dir.join("inbox.json");
        let ussd_bookmarks_file = base_dir.join("ussd_bookmarks.json");

        let mut cfg = AppSettingsFile::default();
        let settings_existed = settings_file.exists();
        let mut settings_loaded = false;
        if let Ok(content) = std::fs::read_to_string(&settings_file) {
            if let Ok(parsed) = serde_ini::from_str::<AppSettingsFile>(&content) {
                cfg = parsed;
                settings_loaded = true;
            }
        }
        if !matches!(cfg.sms.log_mode.as_str(), "at" | "system" | "important" | "all" | "raw") {
            cfg.sms.log_mode = "system".to_string();
        }

        let (app_event_tx, app_event_rx) = mpsc::channel::<AppEvent>();
        let serial_busy = Arc::new(Mutex::new(false));

        let mut app = Self {
            connected: false,
            connection_in_progress: false,
            detected_mode: "Unknown".to_string(),
            serial_profile: "Unknown".to_string(),
            displayed_serial_profile: "Unknown".to_string(),
            rndis_status: "Unknown".to_string(),
            mode_detection_pending: false,
            last_mode_detection: Instant::now() - Duration::from_secs(10),
            status_text: "Disconnected".to_string(),
            status_color: Color32::RED,
            serial_port: None,
            reader_running: false,
            expecting_cds_pdu: false,
            expecting_cmt_pdu: false,
            expecting_cmgr_pdu: false,
            expecting_cmgl_pdu: false,
            pending_cmgr_index: None,
            pending_cmgl_index: None,
            pending_cmgl_status: String::new(),
            cmgr_read_queue: VecDeque::new(),
            waiting_cpms_for_sms: false,
            last_inbox_poll: Instant::now(),
            connected_port: None,
            serial_tx: None,
            response_rx: None,
            reader_thread: None,
            ui_rx: None,
            reader_flag: None,
            app_event_tx,
            app_event_rx,
            phone_number: cfg.sms.phone.clone(),
            message_text: String::new(),
            sms_class: 0,
            sms_class_options: vec![
                "0 (Flash)".to_string(),
                "1 (Normal)".to_string(),
                "2 (SIM)".to_string(),
                "3 (Phone)".to_string(),
            ],
            dcs_value: cfg.sms.dcs.clone(),
            dcs_options: vec![
                "0x50 (Class 0 - 7bit) [OK]".to_string(),
                "0x10 (Class 0 - 7bit)".to_string(),
                "0xF0 (Class 0 - 7bit alt)".to_string(),
                "0x11 (Class 1 - 7bit) [OK]".to_string(),
                "0x01 (GSM 7-bit default)".to_string(),
                "0xF1 (Class 1 - 7bit alt)".to_string(),
                "0x12 (Class 2 - 7bit) [OK]".to_string(),
                "0x02 (GSM 7-bit default)".to_string(),
                "0xF2 (Class 2 - 7bit alt)".to_string(),
                "0x13 (Class 3 - 7bit) [OK]".to_string(),
                "0x03 (GSM 7-bit default)".to_string(),
                "0xF3 (Class 3 - 7bit alt)".to_string(),
            ],
            delivery_report: cfg.sms.delivery_report.parse().unwrap_or(true),
            log_mode: cfg.sms.log_mode.clone(),
            hide_status_logs: true,
            char_count: "0 / 160".to_string(),
            signal: "---".to_string(),
            operator: "---".to_string(),
            network: "---".to_string(),
            net_reg: "---".to_string(),
            tac_lac: "---".to_string(),
            cell_id: "---".to_string(),
            net_tech: "---".to_string(),
            cell_band: "---".to_string(),
            dl_earfcn: "---".to_string(),
            dl_freq: "---".to_string(),
            dl_bw: "---".to_string(),
            ul_earfcn: "---".to_string(),
            ul_freq: "---".to_string(),
            ul_bw: "---".to_string(),
            rssi: "---".to_string(),
            rsrp: "---".to_string(),
            sinr: "---".to_string(),
            rsrq: "---".to_string(),
            inbox_messages: Vec::new(),
            inbox_display_items: Vec::new(),
            current_inbox_msg: None,
            inbox_view_mode: "simple".to_string(),
            inbox_selected: None,
            phonebook_data: Vec::new(),
            phonebook_selected: None,
            dr_records: Vec::new(),
            dr_selected: None,
            dr_detail_text: String::new(),
            modem_ip: cfg.network.modem_ip.clone(),
            modem_user: cfg.network.username.clone(),
            modem_pass: unprotect_secret(&cfg.network.password),
            switch_mode: cfg.network.switch_mode.clone(),
            manual_port: cfg.serial.manual_port.clone(),
            manual_baud: cfg.serial.baud.clone(),
            manual_bypass: cfg.serial.bypass_autodetect.parse().unwrap_or(false),
            port_label_map: HashMap::new(),
            com_ports: Vec::new(),
            at_command: String::new(),
            at_output: String::new(),
            at_history: Vec::new(),
            at_hist_idx: -1,
            ussd_input: String::new(),
            ussd_response: String::new(),
            ussd_raw_response: String::new(),
            ussd_buttons: Vec::new(),
            ussd_active: false,
            ussd_dcs: "15".to_string(),
            ussd_view_raw: false,
            ussd_plain_text: false,
            ussd_console: String::new(),
            ussd_history: Vec::new(),
            ussd_bookmarks: Vec::new(),
            ussd_bookmarks_file,
            ussd_bookmarks_open: false,
            current_tab: 0,
            tab_names: vec![
                "SMS".to_string(),
                "Inbox".to_string(),
                "Phonebook".to_string(),
                "Network Info".to_string(),
                "Delivery Reports".to_string(),
                "Settings".to_string(),
                "AT Terminal".to_string(),
                "USSD".to_string(),
                "Scheduled SMS".to_string(),
            ],
            sent_messages: HashMap::new(),
            concat_pending: HashMap::new(),
            raw_log_entries: VecDeque::new(),
            persistent_log_entries: VecDeque::new(),
            info_log_entries: VecDeque::new(),
            max_raw_log_entries: 100,
            max_persistent_log_entries: 20_000,
            max_info_log_entries: 2_000,
            log_paused: false,
            log_cache_dirty: true,
            cached_filtered_log: Vec::new(),
            log_scrolled_to_bottom: true,
            log_usage_bytes: 0,
            max_log_bytes: 50_000_000,
            incoming_buffer: String::new(),
            serial_busy,
            contacts_file,
            settings_file,
            inbox_file,
            cfg,
            show_add_contact: false,
            contact_name_input: String::new(),
            contact_number_input: String::new(),
            warning_message: None,
            settings_saved: None,
            scheduled_messages: Vec::new(),
            scheduled_file: base_dir.join("scheduled.json"),
            show_add_schedule: false,
            sched_recipient_list: Vec::new(),
            sched_recipient_input: String::new(),
            sched_show_phonebook_dropdown: false,
            sched_message: String::new(),
            sched_date: String::new(),
            sched_time: String::new(),
            sched_repeat_input: String::new(),
            sched_repeat_unit: 0,
            sched_end_time: String::new(),
            sched_flash_sms: false,
            next_schedule_id: 1,
        };

        if settings_existed {
            if settings_loaded {
                app.log("Loaded settings from settings.ini", "system");
            } else {
                app.log("Failed to load settings.ini; using defaults", "error");
            }
        } else {
            app.log("settings.ini not found; creating default settings.ini", "system");
            app.save_settings();
        }

        let saved_dcs = app.cfg.sms.dcs.clone();
        app.sms_class = match app.cfg.sms.sms_class.chars().next().unwrap_or('0') {
            '1' => 1,
            '2' => 2,
            '3' => 3,
            _ => 0,
        };
        if saved_dcs.is_empty() || !saved_dcs.starts_with("0x") {
            app.dcs_value = "0x50 (Class 0 - 7bit) [OK]".to_string();
        } else {
            app.sync_class_from_dcs();
        }

        app.load_inbox_file();
        app.load_phonebook_local();
        app.update_char_count();
        app.refresh_com_ports(true);
        app.load_scheduled();
        app.load_ussd_bookmarks();
        app.detect_modem_mode_async();

        // Hardware watcher thread
        let hw_tx = app.app_event_tx.clone();
        modem::spawn_hardware_watcher(hw_tx);

        app
    }
}

impl eframe::App for ModAtApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(17, 24, 39);
        visuals.window_fill = Color32::from_rgb(24, 28, 36);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(31, 41, 55);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(55, 65, 81);
        visuals.widgets.active.bg_fill = Color32::from_rgb(67, 56, 202);
        visuals.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(229, 231, 235);
        ctx.set_visuals(visuals);

        self.update_char_count();
        self.check_scheduled_sms();
        self.process_app_events();
        self.process_ui_serial();
        if self.connected && self.last_inbox_poll.elapsed() >= Duration::from_secs(5) {
            self.last_inbox_poll = Instant::now();
            self.poll_unread_sms();
        }

        if let Some(ref mut t) = self.settings_saved {
            *t -= ctx.input(|i| i.stable_dt);
            if *t <= 0.0 {
                self.settings_saved = None;
            }
        }

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Modem", |ui| {
                    let connection_label = if self.connected {
                        "Disconnect"
                    } else {
                        "Connect Serial"
                    };
                    if ui.button(connection_label).clicked() {
                        if self.connected {
                            self.disconnect();
                            self.detect_modem_mode_async();
                        } else {
                            self.smart_connect(None);
                        }
                        ui.close_menu();
                    }

                    let is_serial_mode = self.connected
                        || self.detected_mode == "Serial"
                        || self.serial_profile == "Project Mode"
                        || self.serial_profile == "Debug Mode";
                    let mode_label = if is_serial_mode {
                        "Switch to Normal Mode"
                    } else {
                        "Switch to Serial Mode"
                    };
                    if ui.button(mode_label).clicked() {
                        self.switch_mode_action();
                        ui.close_menu();
                    }

                    if ui.button("Detect Modem Mode").clicked() {
                        self.detect_modem_mode_async();
                        ui.close_menu();
                    }

                    if ui.button("Refresh Modem Info").clicked() {
                        self.get_modem_info_async();
                        ui.close_menu();
                    }

                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        egui::TopBottomPanel::top("modem_info").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Signal:");
                ui.colored_label(self.get_signal_color(), &self.signal);
                ui.separator();
                ui.label(format!("Operator: {}", self.operator));
                ui.separator();
                ui.label(format!("Network: {}", self.network));
                ui.separator();
                ui.label(format!("RNDIS Status: {}", self.rndis_status));
                ui.separator();
                ui.label(format!("Profile: {}", self.serial_profile_label()));
                ui.separator();
                ui.colored_label(self.status_color, &self.status_text);
            });
        });

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (idx, tab) in self.tab_names.iter().enumerate() {
                    if ui.selectable_label(self.current_tab == idx, tab).clicked() {
                        self.current_tab = idx;
                    }
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_min_width(360.0);
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| match self.current_tab {
                    0 => self.render_sms_tab(ui),
                    1 => self.render_inbox_tab(ui),
                    2 => self.render_phonebook_tab(ui),
                    3 => self.render_network_info_tab(ui),
                    4 => self.render_delivery_reports_tab(ui),
                    5 => self.render_settings_tab(ui),
                    6 => self.render_at_terminal_tab(ui),
                    7 => self.render_ussd_tab(ui),
                    8 => self.render_scheduled_sms_tab(ui),
                    _ => self.render_sms_tab(ui),
                });
        });

        if let Some(message) = self.warning_message.clone() {
            egui::Window::new("Warning")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(message);
                    if ui.button("OK").clicked() {
                        self.warning_message = None;
                    }
                });
        }

        if self.show_add_contact {
            egui::Window::new("Add Contact")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.contact_name_input);
                    ui.label("Number:");
                    ui.text_edit_singleline(&mut self.contact_number_input);
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            let name = self.contact_name_input.clone();
                            let number = self.contact_number_input.clone();
                            if number.trim().is_empty() {
                                self.add_current_phone_as_contact(name);
                            } else {
                                self.add_manual_contact(name, number);
                            }
                            if self.warning_message.is_none() {
                                self.show_add_contact = false;
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_add_contact = false;
                        }
                    });
                });
        }
    }
}

fn main() -> eframe::Result<()> {
    let icon_bytes = include_bytes!("icon.png");
    let icon_data = eframe::icon_data::from_png_bytes(icon_bytes).ok();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([850.0, 720.0])
            .with_title("ModAT-TR v0.2.0")
            .with_icon(icon_data.unwrap_or_default()),
        ..Default::default()
    };
    eframe::run_native(
        "ModAT-TR",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(ModAtApp::new(cc)))
        }),
    )
}
