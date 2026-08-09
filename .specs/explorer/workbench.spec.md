---
id: REQ:explorer/workbench
type: requirement
status: accepted
level: MUST
summary: >
  Forge-spec users can navigate from root specifications to code, inspect and
  edit individual specifications, and receive incremental updates without a
  full repository reload.
owners: [carlo]
refines: []
categorized_under: [TOPIC:explorer/forge-spec]
related:
  - ADR:explorer/vscode-thin-client
---

# Specification workbench

## Context

Forge-spec already owns parsing, linting, refinement, source-reference, symbol,
and Git-trailer semantics. The workbench exposes those semantics without also
introducing a general-purpose notes vault or a second interpretation of the
format.

:::{requirement id="tree" level="MUST"}
The workbench MUST expose a collapsible tree that begins with root
specifications and can be expanded through clauses, refining specifications,
tasks, and related code.
:::

:::{requirement id="viewer" level="MUST"}
Selecting a specification MUST open that exact Markdown document in a rendered
specification viewer showing lifecycle status and type-specific state.
:::

:::{requirement id="metadata" level="MUST"}
The viewer MUST expose universal and type-specific frontmatter and MUST permit
supported metadata fields to be edited through the editor's normal undo,
dirty-state, save, and validation flows.
:::

:::{requirement id="navigation" level="MUST"}
References to other specifications, clauses, source ranges, and source symbols
MUST navigate to their canonical repository targets.
:::

:::{requirement id="incremental" level="MUST"}
The workbench MUST display a persisted cached index immediately and MUST only
reparse stale, created, or explicitly invalidated files during reconciliation.
:::

:::{requirement id="authority" level="MUST"}
The forge-spec Rust implementation MUST remain authoritative for parsing,
validation, graph construction, and source-reference resolution so editor
clients do not drift from the CLI.
:::

