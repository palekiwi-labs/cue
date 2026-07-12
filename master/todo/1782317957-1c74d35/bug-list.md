---
priority: normal
title: Bug list
status: open
---

- [ ] `.cue/feat-curator-w-acuity/todo/1782317957-1c74d35/sse-limit-history.md`

- [ ] bug with incorrect http server: `.cue/feat-curator-w-acuity/trace/1782317957-1c74d35/bug-server-error-breaks-ui.md`
  This seems a wider issue that applies to any kind of logging and error messages. We do not seem to have any loggin or
  tracing in `curator` yet. Also for an error message like that, we need to consider what UI/UX solution
  to choose, e.g. a popup dialog?

- [x] in the activity feed and diagnostics views, the time is in a different timezone
  than what my host machine uses, e.g. `2026-06-25T08:10:01`

- [x] in the activity feed and diagnostics views, the highlighted row bg color is the same
  as the color for the datetime fg color which results in the datetime being invisible

- [x] in the activity feed a long text is cut off
