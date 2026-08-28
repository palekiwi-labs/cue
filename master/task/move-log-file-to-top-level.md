---
status: complete
title: move log file to top-level
priority: high
---
Currently `cue log` creates a file and log into `spec/log.md`. It is odd that
the log file is placed inside the `spec/` directory intended for specifications.
We should move the branch root directory instead.
