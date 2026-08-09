---
id: ADR:explorer/vscode-thin-client
type: adr
status: accepted
summary: >
  Build the primary forge-spec workbench as a VS Code extension backed by the
  existing Rust language server and a Rust-owned persistent index.
owners: [carlo]
decision_date: "2026-08-09"
decided_by: [carlo]
related:
  - REQ:explorer/workbench
  - REQ:explorer/incremental-index
---

# VS Code thin client

## Context

Tolaria provides useful Markdown rendering and reference-link behavior, but its
vault, inbox, Git workflow, and general note-management model are broader than
the specification-navigation job. Reimplementing forge-spec parsing in an
editor extension would create a second semantic authority.

## Decision

The primary interactive workbench will be a VS Code extension. It will use
native tree and text-document integration plus a themed custom specification
viewer. The extension will delegate parsing, validation, graph construction,
and source-reference resolution to a long-lived forge-spec Rust language
service.

The Rust service will own an in-memory index and a SQLite persistence layer. The
extension will place that database in workspace-specific extension storage and
will forward scoped filesystem events to the service.

Tolaria will remain an independent standalone vault and a source of reusable
link/rendering behavior, but it will not be the primary forge-spec workbench.

## Consequences

- Individual specifications inherit editor-native file identity, undo, save,
  diff, accessibility, keyboard navigation, and source opening.
- The service can later support other editors without copying format logic.
- The extension must package or locate a compatible `spec` binary.
- The persistent database is derived state and may be deleted or rebuilt at any
  time.

The client entry point and service boundary live in
[extension.ts](spec:src:editors/vscode/src/extension.ts) and
[service.ts](spec:src:editors/vscode/src/service.ts).
