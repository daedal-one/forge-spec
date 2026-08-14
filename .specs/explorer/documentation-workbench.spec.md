---
id: TASK:explorer/documentation-workbench
type: task
status: accepted
summary: Expose indexed project documentation through Markdown-aware LSP navigation and a dedicated native explorer surface.
owners: [carlo]
progress: done
addresses: [REQ:core/documentation-integration#c-tooling]
assignee:
eta:
blocked_by: []
---

# Documentation workbench

## Plan

Implemented Markdown-aware completion, hover, definition, references, diagnostics, heading document symbols, configured document selection, and a collection/directory/document/heading explorer. Key surfaces: [Rust LSP](spec:src:spec-cli/src/lsp.rs), [workspace protocol](spec:src:editors/vscode/src/protocol.ts), [service wiring](spec:src:editors/vscode/src/service.ts), and [Documentation explorer](spec:src:editors/vscode/src/tree.ts).

## Acceptance

Rust protocol tests verify unsaved Markdown diagnostics, hierarchical heading symbols, hover, and definition. VS Code compilation and 17 tests verify document references, Markdown rendering, and existing workbench behavior. Enrolled files open in the ordinary editable Markdown editor.
