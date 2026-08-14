---
id: TASK:core/canonical-projection-api
type: task
status: accepted
summary: Expose and verify the deterministic read-only specification projection API.
owners: [carlo]
progress: done
addresses:
  - REQ:core/canonical-projection#c-overlay
  - REQ:core/canonical-projection#c-canonical-state
  - REQ:core/canonical-projection#c-invalid-state
  - REQ:core/canonical-projection#c-exact-relations
  - REQ:core/canonical-projection#c-deterministic-diff
  - REQ:core/canonical-projection#c-read-only
labels: [overlay, schema, diagnostics, relations, diff, safety]
assignee: carlo
eta:
blocked_by: []
---

# Canonical projection API

## Plan

Add an independent Rust library module that reads a saved tree plus in-memory
overlays, validates it without external symbol providers, and emits canonical
state and deterministic semantic deltas.

## Acceptance

Disk-equivalent overlays, insertion-order changes, multi-file create/replace/
delete, invalid inputs, exact relationships, source selectors, deterministic
diffs, and filesystem immutability are covered by projection contract tests.

Implemented by the [projection module](spec:src:spec-cli/src/projection.rs) and
[integration tests](spec:src:spec-cli/tests/projection.rs).
