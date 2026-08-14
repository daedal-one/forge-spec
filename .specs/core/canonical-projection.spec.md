---
id: REQ:core/canonical-projection
type: requirement
status: accepted
level: MUST
summary: Library consumers can project a saved specification tree plus multi-file in-memory changes into canonical semantic state and deterministic deltas without modifying the workspace.
owners: [carlo]
refines: []
related: [REQ:explorer/incremental-index, REQ:core/change-impact]
---

# Canonical specification projection

:::{requirement id="projection" level="MUST"}
- {#c-overlay} The library API MUST accept a base `.specs/` directory plus
  repository-relative in-memory create, replace, and delete entries for
  specification, configuration, and redirect files. It MUST reject paths that
  escape the supplied root.
- {#c-canonical-state} The result MUST use a versioned public schema and
  deterministic ordering for configuration, durable specifications, typed
  blocks, clause anchors, redirects, explicit relationships, source selectors,
  diagnostics, and a separate work-item collection. It MUST contain no absolute
  host paths.
- {#c-invalid-state} Invalid specification, configuration, or redirect input
  MUST remain represented as sorted diagnostics with an invalid state;
  projection MUST NOT silently omit the input or depend on language-server
  availability.
- {#c-exact-relations} Projection MUST retain exact clause-qualified refinement
  targets, aspects, categorization, explicit references, source selectors,
  synthesized project containment, task addressing, and task blocking as
  distinct relationship kinds. Task addressing MUST NOT imply specification
  refinement or source evidence.
- {#c-deterministic-diff} The library MUST compute a deterministic semantic
  delta between two canonical states, covering added, removed, and changed
  durable specifications, work items, and relationships.
- {#c-read-only} Projection MUST perform no workspace writes. Equivalent saved
  and overlaid bytes MUST produce byte-identical canonical state.
:::

The independent library surface is implemented by the [canonical projection
module](spec:src:spec-cli/src/projection.rs) and validated by the [projection
contract tests](spec:src:spec-cli/tests/projection.rs).
