---
status: closed
---
## Changes

1. Change the order of columns:
  - <harness> goes first because has the most predictable (fixed) shape - always 2 chars
  - then date/time
  - then project
  - then title
2. Change the color of date to bright cyan because it is now the same as the
   title
3. Orange is too prominent for the dates in the events view, maybe change it to
   bright cyan, the same as in sessions list
4. drop the `>` indicator for selected row. We already have row highlight and
   can reclaim the space
5. In Session Info, let's split Tokens into `Tokens In` and `Tokens Out`

## Enhancements

- keybinding: <C-y> copies session id to clipboard

