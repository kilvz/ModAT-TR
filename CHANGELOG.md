# Changelog

## v0.2.1 — 2026-05-08

### Added
- **Concatenated SMS assembly** — multi-part SMS messages are automatically buffered and joined into one inbox entry
- **Alphanumeric sender decoding** — sender names (e.g. "3Info") from providers using alphanumeric addressing are now decoded as 7-bit GSM text instead of garbled BCD digits
- **AT command logging** — all AT commands sent (user or app-initiated) and their replies are logged in the AT terminal view; both readable and raw forms shown
- **"Edit" button for USSD bookmarks** — opens `ussd_bookmarks.json` in Notepad
- **Log memory cap** — total log RAM usage limited to 50MB; oldest entries trimmed when exceeded
- **Persistent important log** — SMS and error log entries are stored in a dedicated high-capacity buffer (20,000 max) that won't be evicted by volume system/polling messages

### Changed
- **Terminal log default** — new installs default to "System" view; invalid legacy values normalized to "system"
- **USSD bookmarks auto-close** — bookmarks panel collapses when a bookmark is clicked
- **USSD menu extraction** — supports multi-digit menu items (10, 88, etc.) and requires whitespace after separator to avoid false matches on dates like "04-OCT"
- **Terminal log layout** — fixed vertical centering; entries are now left-aligned with no text overflow
- **Terminal log performance** — switched to virtual scrolling (`show_rows`); only visible rows rendered
- **AT view** — now shows both readable decoded AT traffic AND raw serial data

### Fixed
- **USSD menu item extraction** — no longer misses options like "88. Kembali" and "0. Lanjut"
- **USSD false matches** — "04-OCT-28" no longer detected as menu item
- **Timezone display** — reverts to modem convention (Huawei uses opposite sign bit) for correct +7 display
- **RNDIS status duplicate log** — "RNDIS status: Available" no longer logged twice when two threads report simultaneously
- **Radio buttons when disconnected** — log view radio buttons now immediately refresh the display
- **Stale UI after disconnect** — `signal`/`operator`/`network` labels now reliably reset to "---" when modem is removed
- **Raw log eviction** — spam entries (signal polling, network, etc.) are evicted first when the raw buffer fills, protecting important raw entries (SMS PDUs, delivery reports)
- **"Hide status" filter** — now consistently filters spam from all log views including RAW

### Build
- Binary size ~6 MB (`opt-level = "z"`, `panic = "abort"`, LTO, strip)

---

## v0.2.0 — 2026-05-08

### Added
- **USSD bookmarks as JSON file** (`ussd_bookmarks.json`) — user can edit, delete, or replace bookmarks without recompiling; Reload button reads changes live
- **Terminal log "Hide status" filter** — checkbox hides repeated network/signal/frequency polling noise (Network, Signal, +CSQ, +COPS, +CREG, +CGREG, ^SYSINFOEX, ^RSSI, ^HCSQ, ^HFREQINFO, ^DSFLOWRPT, OK, and escaped raw chunks)
- **Delivery report modem deletion** — "Clear Delivery Reports (Modem)" button now scans modem storage with `AT+CMGL=4`, identifies only status-report PDUs (MTI 0x02), and deletes them via `AT+CMGD=<index>` without touching inbox SMS
- **Inbox auto-polling** — polls unread SMS with `AT+CMGL=0` every 5 seconds while connected
- **Duplicate PDU detection** — repeated `+CMTI: "ME",0` notifications no longer create duplicate inbox rows
- **MIT LICENSE file**

### Changed
- **USSD bookmarks** — clicking a bookmark now directly sends the USSD code instead of filling the input field
- **USSD reply buttons** — text labels are now clickable directly (e.g. "1. 7GB 35rb") instead of separate numbered buttons + non-clickable labels
- **USSD response area** — now uses dynamic available height with scroll instead of fixed 100px cap
- **Settings save** — replaced big modal "Warning" popup with an inline green "Settings saved!" label that auto-disappears
- **Terminal log default** — new installations default to "Important" view; existing settings.ini with invalid values are normalized
- **Delivery Reports tab** — "Clear Delivery Reports (Modem)" is no longer disabled; it now safely deletes only delivery reports

### Fixed
- **USSD button extraction** — no longer produces false menu items from text like `"Onnet:0 s/d 04-OCT-28"`
- **Inbox SMS receiving** — Huawei modem compatibility restored with `AT^CURC=0`, `AT+CNMI=2,0,0,2,1`, and `AT+CPMS="ME","ME","ME"` modem init
- **Delivery report receiving** — CNMI `ds` parameter changed from `0` to `1` so modem forwards `+CDS:` delivery reports
- **CMGL response parsing** — loosened `+CMGL:` regex to handle Huawei-style variable-format responses (`+CMGL: 0,0,,26` and similar)
- **SMS inbox reading** — `+CMTI:` / `+CDSI:` handlers now set CPMS to the modem-reported storage before sending CMGR/CDS reads
- **Stale PDU expectation flags** — removed cross-frame flag corruption that could misroute hex lines
- **Initialization race** — AT+CPMS sent before CNMI setup to ensure correct storage selection

### Build
- Binary size reduced from ~10 MB to ~6 MB (`opt-level = "z"`, `panic = "abort"`, LTO, strip)
