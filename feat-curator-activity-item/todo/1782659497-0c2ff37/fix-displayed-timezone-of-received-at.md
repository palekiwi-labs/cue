---
status: complete
priority: high
title: fix displayed timezone of received_at
---
The displayed date time does not match my system timezone,
fix it so that the displayed time and date matche the host's
timezone.

Resolved in Slice 6d by commits c798df4 and 54ddc6c. Both
datetime formatters convert UTC to host-local timezone via
`dt_utc.with_timezone(&Local)`:
- `format_datetime` (crates/curator/src/ui.rs:1011) for the sessions pane
- `format_event_datetime` (crates/curator/src/ui.rs:1068) for the events pane
