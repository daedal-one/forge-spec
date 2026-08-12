---
id: TASK:core/atomic-mutations
type: task
status: accepted
summary: Validate and persist workspace mutations as recoverable transactions.
owners: [carlo]
progress: done
refines: [REQ:core/typed-mutation#c-atomic]
assignee: carlo
eta:
blocked_by: []
---

# Atomic mutations

## Plan

Apply operations in memory, rebuild and lint the candidate registry, prepare
temporary files only after validation, atomically replace targets with rollback,
then reload the written workspace.

## Acceptance

New errors or a failed multi-document write leave every original unchanged.

Implemented and rollback-tested in the [mutation
engine](spec:src:spec-cli/src/mutation/engine.rs).
