---
id: TASK:core/architectural-model-runtime
type: task
status: accepted
summary: 'Implement and validate the v0.7 architecture, scenario, view and machine export contract for Forge and Intellect.'
owners: [carlo]
progress: done
addresses: [REQ:core/architectural-model]
labels: []
groups: []
assignee:
eta:
blocked_by: []
---

# Architectural model runtime

## Plan

Implement the [architectural model contract](spec:REQ:core/architectural-model) through shared parser, registry, lint, typed writer, renderers, LSP/editor and canonical projection. Publish model v1 with state/delta v5. Keep C4 optional and TASK orthogonal.

## Acceptance

Validated by the [architecture model suite](spec:src:spec-cli/tests/architecture_model.rs), canonical projection tests and LSP/editor tests. The complete Rust suite passes 153 tests with one pre-existing ignored provider integration test; editor TypeScript compilation and 17 tests pass. The real v0.7 tree lints without errors; coverage warnings remain. Machine export is valid, preserves durable-only intent digests and disables repository fsmonitor hooks and index writes. Historical v0.6 read compatibility emits current v5 state while retaining previous immutable artifact provenance.

