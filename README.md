# ModAT-TR v 0.2.0

A high-performance modem dashboard built with **Rust** + **egui** for SMS management, real-time network diagnostics, USSD queries, scheduled messaging, and AT terminal control.

---

## Features

### SMS Management
- Send SMS in PDU mode with configurable Class (0-Flash/1-Normal/2-SIM/3-Phone) and DCS
- Inbox with read, reply, forward, delete — auto-decodes PDU, saves locally
- Inbox auto-sync: polls unread SMS with `AT+CMGL=0` every 5 seconds while connected
- Manual "Load Inbox" button syncs all modem-stored SMS via `AT+CMGL=4`
- Duplicate PDU detection to prevent repeated notifications from spamming inbox
- Delivery reports with status tracking and raw PDU detail view
- "Clear Delivery Reports (Modem)" scans modem storage and deletes only status-report PDUs (leaves inbox SMS alone)
- Invisible ping support
- Contact name resolution in logs and delivery reports

### USSD Tab
- Send USSD codes with configurable DCS (GSM 7-bit, UCS2, packed 7-bit, etc.)
- GSM 03.38 encoding + 7-bit packing for proper Huawei modem compatibility
- Click a bookmark or reply text label to directly send the USSD code or reply
- Reply button extraction only matches menu items at the start of lines or after whitespace
- Activity indicator (Active/Idle), RAW/readable toggle, console log
- **Bookmarks**: user-editable `ussd_bookmarks.json` with Reload button — no restart needed
- Pre-loaded default bookmarks for Indonesian operators (Telkomsel, Indosat, XL, Tri, Smartfren)
- **History**: remembers last 5 unique USSD codes sent

### Scheduled SMS
- Schedule SMS with date, time, and repeat interval (minutes/hours/days)
- Optional end date for recurring messages
- Flash SMS (Class 0) support
- Tag-based recipient input with phonebook picker
- Background timer with auto-reschedule for repeating entries

### AT Terminal
- Tab autocomplete from ~150 commands across 4 categories (Universal, Huawei, Qualcomm, MediaTek)
- Always-visible matching command panel with descriptions
- Up/Down history navigation
- Response output with scroll

### Terminal Log
- Five filter views: **AT** (decoded serial), **System** (app events + SMS), **Important** (errors + SMS only), **All**, **RAW**
- "Hide status" checkbox filters out repeated network polling noise (Signal, Network, +CSQ, +COPS, +CREG, +CGREG, ^SYSINFOEX, ^RSSI, ^HCSQ, ^HFREQINFO, ^DSFLOWRPT, OK, and their escaped raw chunks)
- Pause/Resume button to freeze log for analysis
- Raw serial lines auto-translated to readable format
- VecDeque-backed buffers for O(1) trimming (24/7 safe)

### Network Info
- Signal strength, operator, network registration
- Cell ID, TAC/LAC, band, EARFCN, frequency, bandwidth
- RSSI, RSRP, SINR, RSRQ with color-coded quality indicators
- Signal quality guide

### Phonebook
- Local `contacts.json` persistence
- Add/delete contacts, use in recipients

### Settings
- COM port selection with auto-detection
- Baud rate (9600-115200)
- Bypass auto-detect for direct connection
- Switch mode (Debug/Project)
- SMS DCS, class, delivery report, and log mode configuration
- Settings save shows inline green confirmation label
- DPAPI-encrypted password storage (Windows)

### Modem Control
- Connect Serial / Connect Directly
- Smart connect with RNDIS detection and profile matching
- Switch to Normal Mode (HiLink) / Switch to Serial Mode
- Detect Modem Mode, Refresh Modem Info

---

## Build

### Requirements
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- Cargo (included)

### Compile & Run
```bash
cd rust-modat
cargo run
```

### Release Build
```bash
cd rust-modat
cargo build --release
```
Output: `target/release/modat-t.exe` (~6 MB, optimized with `opt-level="z"`, LTO, stripped)

---

## Project Structure

```
rust-modat/
├── src/
│   ├── main.rs          # App entry, structs, config, update loop
│   ├── modem.rs         # Serial, connection, detection, CNMI, events
│   ├── sms.rs           # PDU encode/decode, SMS send, inbox, delivery, polling
│   ├── storage.rs       # Persistence, logging, contacts, settings
│   ├── ussd.rs          # USSD send/reply/cancel/parse + GSM encoding
│   ├── scheduled.rs     # Scheduled SMS struct + timer + load/save
│   ├── patterns.rs      # Cached regex definitions
│   ├── serial.rs        # SerialPortWrapper
│   └── ui/
│       ├── mod.rs
│       ├── helpers.rs
│       ├── sms_tab.rs
│       ├── inbox_tab.rs
│       ├── phonebook_tab.rs
│       ├── network_tab.rs
│       ├── delivery_tab.rs
│       ├── settings_tab.rs
│       ├── at_terminal_tab.rs
│       ├── ussd_tab.rs
│       └── scheduled_tab.rs
├── Cargo.toml
└── LICENSE
```

---

## Configuration Files
- `settings.ini` — connection, SMS, and network preferences
- `contacts.json` — phonebook entries
- `inbox.json` — saved SMS messages
- `scheduled.json` — scheduled SMS entries
- `ussd_bookmarks.json` — user-editable USSD bookmark groups

---

## License
MIT — see `LICENSE` file.

## Disclaimer
This software interacts directly with modem firmware. Modifying advanced parameters via AT commands may cause unexpected behavior. Use at your own risk.
