---
status: open
---
Reconsider the purpos of diagnostics view: currently it displays
all pre-tool call and post-tool call events. Almost all of those
tool call events are successes so it does not tell us too much
that is immediately important. Moreover, displayed information
is extremely limited - just the name of the tool and status,
it does not say what command and arguments where requested
for `bash` tools.

Consider displaying in a filtered view by default where only
errors (actual diagnostics) are printed with enough context
that makes the diagnostics useful and actionable.
