---
priority: normal
title: acumen
status: open
---
the "cue" framework produces a lot of content that eventually
piles up and is increasingly difficult for the agent to query
and retrieve information effectively.

Perhaps we can consider introducing  a RAG (retrieval augmented
generation) component to the "cue" ecosystem?

My proposal: `acumen` - a RAG pipeline for `cue`.

I don't really have much to say about `acumen` would work as RAG
because I have never built or used RAG before. I only have conceptual
understanding of how RAG works and its functionality.

However, thanks to `acuity` we are able to collect statistics
and important signals that could be vital to automated
and timely re-indexing of `acumen`. Moreover, we could extend
`curator` to track the state of `acumen` and/or trigger
any of the pipiline's operations manually.
