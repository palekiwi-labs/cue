# Artifact Metadata

---

Consider supporting some form of metadata on created artfiacts.

Artifacts differ in terms of their purpose and lifespan, e.g. compare
`spec` and `doc` artifacts. `spec/plan.md` is mostly useful for the duration
of the branch, PR or a feature. Artifacts inside `doc` may provide useful
documentation that could scanned and referred to during future work
or could be used as inputs for creation of agent skills.


## Metadata examples:

- `tags`: a list of tags associated with the content
- `description`: a short summary of the artifact's content that can be extracted and printed
  to evaluate whether the artifact is relevant to the problem at hand
