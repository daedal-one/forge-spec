---
id: TASK:core/command-hierarchy
type: task
status: accepted
summary: Replace the flat v0.3 parser with the discoverable v0.4 command tree.
owners: [carlo]
progress: done
addresses: [REQ:core/typed-mutation#c-discoverable]
assignee: carlo
eta:
blocked_by: []
---

# Command hierarchy

## Plan

Model namespaces and state arguments as typed Clap enums, keep the binary entry
point thin, and regenerate help and completions from the canonical command
tree.

## Acceptance

Only the v0.4 hierarchy is visible, removed commands fail as unknown, and bare
namespaces print their contextual help.

Implemented by the [typed command tree](spec:src:spec-cli/src/cli.rs), [thin
dispatcher](spec:src:spec-cli/src/commands/dispatch.rs), and [Clap-derived
completions](spec:src:spec-cli/src/commands/completions.rs).
