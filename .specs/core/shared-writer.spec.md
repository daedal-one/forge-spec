---
id: TASK:core/shared-writer
type: task
status: accepted
summary: Route CLI and editor authoring through the shared Rust mutation engine.
owners: [carlo]
progress: done
addresses: [REQ:core/typed-mutation#c-shared]
assignee: carlo
eta:
blocked_by: []
---

# Shared writer

## Plan

Move task, lifecycle, relation, rename, scaffolding, migration, and VS Code
metadata writes onto the Rust transaction boundary and remove the TypeScript
frontmatter writer.

## Acceptance

Every supported `.spec.md` writer uses the mutation engine and editor changes carry
a matching document version.

Implemented across the [transaction
engine](spec:src:spec-cli/src/mutation/engine.rs), [LSP
bridge](spec:src:spec-cli/src/lsp.rs), and VS Code [service
adapter](spec:src:editors/vscode/src/service.ts).
