# Project Log

## [6335f67] fix: emit absolute paths in cue list output (6335f67)

- **Found:** paths returned by collect_files are already absolute; the only change needed was to stop stripping store_dir in two output sites
- **Found:** to_cue_file root parameter was unused after removing the strip_prefix fallback; removed to clean up the API
- **Found:** proxy_reads tests had explicit assertions against absolute paths that needed inverting
- **Decided:** emit absolute paths in both human and JSON cue list output
- **Decided:** remove unused root param from to_cue_file signature

