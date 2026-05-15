# Project Log

## [9d7bd37] Support --branch flag for mem add

Updated CLI definition, main entry point, and add command logic. Added comprehensive tests for explicit branch selection including short flag support.

- **Found:** Users can now specify a target branch for 'mem add' using --branch or -b.
- **Decided:** Sanitized the branch name (replacing slashes with hyphens) to ensure consistent directory naming regardless of source.

