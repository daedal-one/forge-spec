---
id: TASK:core/documentation-index
type: task
status: accepted
summary: 'Implement configured Markdown collections, heading identity, reference validation, backlinks, projection, rendering, and impact traversal.'
owners: [carlo]
progress: done
addresses: [REQ:core/documentation-integration#c-collections, REQ:core/documentation-integration#c-references, REQ:core/documentation-integration#c-index, REQ:core/documentation-integration#c-boundary]
assignee:
eta:
blocked_by: []
labels:
  - collections
  - references
  - index
  - boundary
---

# Documentation index

## Plan

Implemented the shared documentation model, safe collection resolver, hierarchical heading identity, lint rules R026-R029, backlinks, persistent index, canonical state/delta v2, scoped rendering, impact traversal, inspection commands, and workspace-aware completions. Key surfaces: [documentation index](spec:src:spec-cli/src/documentation.rs), [workspace cache](spec:src:spec-cli/src/workspace.rs), [canonical projection](spec:src:spec-cli/src/projection.rs), and [impact analysis](spec:src:spec-cli/src/impact/mod.rs).

## Acceptance

Deterministic Rust and CLI integration tests cover configured discovery, exclusions, heading resolution, ordinary Markdown links, backlinks, invalid configuration, projection and delta behavior, exact render scoping, and documentation-to-spec impact.
