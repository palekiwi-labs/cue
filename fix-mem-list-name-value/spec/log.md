# Project Log

## [0d33585-dirty] Fix: list command name field includes subdirectories

- **Found:** Found that to_mem_file was only using file_name() for the name field.
- **Decided:** Modified to_mem_file to calculate name relative to category root (spec/bin/ref) or relative to commit-hash directory (trace/tmp).

