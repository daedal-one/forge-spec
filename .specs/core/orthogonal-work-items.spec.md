---
id: REQ:core/orthogonal-work-items
type: requirement
status: accepted
level: MUST
summary: Implementation work remains transient and auditable without becoming part of the durable specification graph or claiming behavioral adherence.
owners: [carlo]
refines: []
related:
  - REQ:core/canonical-projection
  - REQ:core/change-impact
  - REQ:core/intellect-provider
  - REQ:core/project-root
  - REQ:core/typed-mutation
---

# Orthogonal implementation work

:::{requirement id="work-items" level="MUST"}
- {#c-orthogonal} TASK documents MUST represent progress-centric implementation
  work and MUST NOT participate in project containment, refinement,
  categorization, clause coverage, or implementation adherence.
- {#c-addresses} A TASK MUST be able to address zero or more durable
  specification documents or clause anchors through a directional
  `addresses` relation. Addressing a specification MUST NOT refine it, cover
  it, satisfy it, or prove that its intended behavior is implemented.
- {#c-transient} Work items MUST remain version-controlled and auditable while
  active or completed, but MUST be semantically transient: creating, starting,
  completing, or archiving one MUST NOT change the specification graph. No
  supported mutation may delete a work-item document.
- {#c-surfaces} Specification-oriented tree, hierarchy, refinement,
  categorization, coverage, render, and adherence surfaces MUST omit TASK
  documents by default. An explicit work-item view MAY show them separately
  and MUST NOT nest them into the specification hierarchy.
- {#c-impact} Change-impact and relation inspection MUST report addressed work
  separately from the specification closure. Work-item changes MUST NOT imply
  that intended behavior changed.
- {#c-adherence} Authored implementation checkpoints and provider-derived
  adherence MUST apply only to durable specification entities. TASK completion
  evidence MUST remain workflow metadata and MUST NOT become implementation
  evidence for an addressed specification.
- {#c-projection} Canonical projection MUST publish durable specifications and
  work items as distinct collections, with typed task-address and task-blocker
  relationships that downstream graph consumers cannot confuse with
  refinement or implementation evidence.
- {#c-migration} Migration from the previous task model MUST be deterministic,
  idempotent, and lossless: task `refines` becomes `addresses`, task `aspects`
  becomes `labels`, task categorization becomes work-item grouping, and a task
  `implemented` checkpoint becomes non-adherence completion metadata.
:::

The authoritative format contract is defined by the [Specs Format
specification](spec:doc:specification.md). The durable implementation evidence
boundary spans [frontmatter parsing](spec:src:spec-cli/src/parse/frontmatter.rs),
[graph construction](spec:src:spec-cli/src/graph/build.rs), [structural
validation](spec:src:spec-cli/src/lint/structural.rs), [typed task
mutations](spec:src:spec-cli/src/commands/task.rs), [tree
presentation](spec:src:spec-cli/src/commands/tree.rs), [impact
analysis](spec:src:spec-cli/src/impact/mod.rs), [adherence request
selection](spec:src:spec-cli/src/intellect.rs), [canonical
projection](spec:src:spec-cli/src/projection.rs), and the [v0.6
migration](spec:src:spec-cli/src/migration/mod.rs).
