---
id: TASK:core/batch-protocol
type: task
status: accepted
summary: Expose versioned multi-operation changes and deterministic dry runs.
owners: [carlo]
progress: done
addresses: [REQ:core/typed-mutation#c-batch]
assignee: carlo
eta:
blocked_by: []
---

# Batch protocol

## Plan

Load `forge-spec-change/v1` JSON from a file or standard input, compile it to
typed operations, and report a deterministic change plan and diagnostics for
`--dry-run`.

## Acceptance

One request can change multiple documents and its dry run performs every check
without changing any bytes.

Implemented by the [batch command](spec:src:spec-cli/src/commands/change.rs) and
shared [mutation engine](spec:src:spec-cli/src/mutation/engine.rs).
