# Changelog

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
