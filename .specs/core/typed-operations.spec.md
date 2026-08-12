---
id: TASK:core/typed-operations
type: task
status: accepted
summary: Define the strict versioned operation catalogue for supported changes.
owners: [carlo]
progress: done
refines: [REQ:core/typed-mutation#c-typed]
assignee: carlo
eta:
blocked_by: []
---

# Typed operations

## Plan

Represent universal, entity-specific, content, relationship, lifecycle, task,
and rename changes as a closed Serde-tagged Rust enum with unknown fields
denied.

## Acceptance

Unknown operations, fields, and incompatible target types fail without touching
the workspace, and no delete operation exists.

Implemented by the closed [operation
catalogue](spec:src:spec-cli/src/mutation/operation.rs).
