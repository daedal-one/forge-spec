---
id: TASK:core/no-document-deletion
type: task
status: accepted
summary: Keep document deletion outside every supported mutation surface.
owners: [carlo]
progress: done
refines: [REQ:core/typed-mutation#c-no-delete]
assignee: carlo
eta:
blocked_by: []
---

# No document deletion

## Plan

Audit the CLI, batch schema, language-server protocol, migrations, and editor
commands so none can request or imply specification deletion.

## Acceptance

No production operation or command deletes a `.spec.md` document.

Enforced by the closed [operation
catalogue](spec:src:spec-cli/src/mutation/operation.rs) and [CLI
hierarchy](spec:src:spec-cli/src/cli.rs).
