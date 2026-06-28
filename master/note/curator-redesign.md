---
status: open
title: Curator Redesign
---
Having used our MVP version of `curator`, I have learned a bit more
about my needs regarding this applicatoin and would like to discuss
them with you so that we can design improvements.

## 1. New view: projects

We currently have three views: kanban, activity feed, diagnostics.

I would like to append a new view at the front: `projects`, give
it a keybinding `1` and shift the other view keybinginds by 1.

### Purpose

The purpose of this view is to display a list of all rergistered cue
projects and aperhaps llow the user to select "active" projects that
will apply to the other views: kanban, activity and diagnostics.

I user should also be allowed to select/deselect all projects.

When the user selects one or more projects, only the tasks and events
for this project will be shown in the other views.

I would also like to display some extra information next to the project
such as the timestamp of the latest event coming from that project which
will give the user information about the project recency and also allow
them to sort the projects by recency so that most recent projects
appear on top.

## 2. Multi-project support

This result from point `1` above - we need to be able to collect and display
data from all registered projects not just the current project.

## 3. Better display of activity view, better data collection

Current activity view is a mess which is partly due to the UI design
and partly due to what data we are collecting.

`cue-plugins` now collects data from the harness only and completely
ignores the data that it can obtain by calling shell commands, e.g.
- current working directory, i.e. the project path
- the name of the agent harness, e.g. `opencode`, `pi`, etc.
- the name of the host (this is more problematic becuase it runs in `cast` containers
  but we can find workarounds, such as adding a feature in `cast` to inject `CAST_HOSTNAME` env var
  into the container)

As a user, when I open the activity feed view, I would like to be able to see clear
organization of the data so that I easily see:

1. what project is this data coming from?

Every event's payload should contain a project path, e.g. `/home/pl/code/palekiwi-labs/cue`
so that we can immediately identify the project that it belongs to. We can improve the way
we display it, e.g. by the key or label that this project registered in our cue projects store.

2. What session is the event part of?

look at quasi-screenshot - copy of what `cuator` displayed to me in the activity view
available in: `/home/pl/code/palekiwi-labs/cue/.cue/master/tmp/1782635029-739e2b7/curator-screenshot.txt`

We can see that events are grouped by project (opencode, cue.nvim, nix-config) which appears correct
but why do all of them have the same identifier - `(ses_0f2c)`?
What are we capturing as here? Is it a "session" identifier?

I would like to propose thi UI design as a start for our discussion, keeping in mind
that we should eventually suport proper collapsing (folding/unfolding) of the event tree:
`/home/pl/code/palekiwi-labs/cue/.cue/master/ref/1782635029-739e2b7/curator-activity-feed-example.txt`

This is just an example to begin the discussion
