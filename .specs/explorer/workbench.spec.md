---
id: REQ:explorer/workbench
type: requirement
status: accepted
level: MUST
summary: >
  Forge-spec users can navigate from project intent to specifications and code,
  inspect and edit individual documents, and receive incremental updates
  without a full repository reload.
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
The workbench MUST expose a collapsible tree that begins with the configured
PROJECT description and can be expanded through clauses, refining
specifications, tasks, and related code.
:::

:::{requirement id="viewer" level="MUST"}
Selecting a specification MUST open that exact Markdown document in a rendered
specification viewer showing lifecycle status and type-specific state. Selecting
a typed block or clause MUST open the same native viewer focused on that exact
semantic unit, including direct navigation to specifications that refine it.
:::

:::{requirement id="metadata" level="MUST"}
The viewer MUST expose universal and type-specific frontmatter and MUST compile
supported changes into the Rust-owned typed mutation protocol while retaining
the editor's normal undo, dirty-state, save, and version checks.
:::

:::{requirement id="navigation" level="MUST"}
References to other specifications, clauses, source ranges, and source symbols
MUST be presented with human-readable labels and MUST navigate to their
canonical repository targets without discarding the exact reference identity.
:::

:::{requirement id="incremental" level="MUST"}
The workbench MUST display a persisted cached index immediately and MUST only
reparse stale, created, or explicitly invalidated files during reconciliation.
:::

:::{requirement id="authority" level="MUST"}
The forge-spec Rust implementation MUST remain authoritative for parsing,
validation, graph construction, source-reference resolution, and every
supported mutation so editor clients do not drift from the CLI.
:::
