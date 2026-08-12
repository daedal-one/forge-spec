---
id: TASK:core/optimistic-concurrency
type: task
status: accepted
summary: Guard typed mutations with deterministic content fingerprints.
owners: [carlo]
progress: done
refines: [REQ:core/typed-mutation#c-concurrency]
assignee: carlo
eta:
blocked_by: []
---

# Optimistic concurrency

## Plan

Calculate stable document fingerprints and check every supplied `if_match`
entry before resolving selectors or applying operations.

## Acceptance

A stale fingerprint rejects the entire transaction without writing.

Implemented by fingerprint and pre-commit checks in the [mutation
engine](spec:src:spec-cli/src/mutation/engine.rs).
