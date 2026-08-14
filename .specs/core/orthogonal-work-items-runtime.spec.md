---
id: TASK:core/orthogonal-work-items-runtime
type: task
status: accepted
summary: Implement and dogfood the v0.6 separation between durable specifications and transient implementation work.
owners: [carlo]
progress: done
addresses:
  - REQ:core/orthogonal-work-items#c-orthogonal
  - REQ:core/orthogonal-work-items#c-addresses
  - REQ:core/orthogonal-work-items#c-transient
  - REQ:core/orthogonal-work-items#c-surfaces
  - REQ:core/orthogonal-work-items#c-impact
  - REQ:core/orthogonal-work-items#c-adherence
  - REQ:core/orthogonal-work-items#c-projection
  - REQ:core/orthogonal-work-items#c-migration
labels: [semantics, relations, lifecycle, presentation, impact, adherence, projection, migration]
assignee: carlo
eta:
blocked_by: []
---

# Orthogonal work-item runtime

## Plan

Publish the v0.6 format and migration, replace task refinement with explicit
addressing across the parser, mutation engine, graph, CLI, renderers, impact
analysis, adherence protocol, and canonical projection, then migrate Forge Spec
and Forge Intellect as the first real consumers.

## Acceptance

Creating, starting, completing, or archiving a task leaves specification
hierarchy, refinement, categorization, clause coverage, and adherence
unchanged. Default specification trees omit tasks; explicit work views preserve
their addressing and blocker relationships; Forge Intellect ingests the two
collections without treating task evidence as implementation evidence.
