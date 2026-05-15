# Project Log

## [a1aa3b8] Added 'doc' category and 'list --type' filter

Implemented support for a new 'doc' category in 'mem add' and added a '--type' (-t) filter to 'mem list' to allow category-specific listings.

- **Found:** 'mem add --type doc' correctly places files in the 'doc/' subdirectory.
- **Found:** 'mem list --type <category>' filters files by the second path component.
- **Found:** Explicitly requesting a type in 'list' overrides the default 'tmp/ref' exclusion.
- **Decided:** Used the existing 'MemType' enum for the new 'list --type' argument for consistency.
- **Decided:** Updated 'src/commands/list.rs' to handle the mapping between 'MemType' variants and directory names.

