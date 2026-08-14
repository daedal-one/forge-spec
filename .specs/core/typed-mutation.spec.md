---
id: REQ:core/typed-mutation
type: requirement
status: accepted
level: MUST
summary: >
  Every supported forge-spec authoring surface applies discoverable, typed,
  validated workspace changes through one lossless Rust mutation engine.
owners: [carlo]
refines: []
related:
  - REQ:core/change-impact
  - REQ:core/project-root
  - REQ:explorer/spec-viewer
---

# Typed workspace mutation

:::{requirement id="mutation" level="MUST"}
- {#c-discoverable} The CLI MUST expose authoring, inspection, lifecycle,
  relationship, task, history, and migration behavior through a discoverable
  nested command hierarchy with no hidden compatibility aliases.
- {#c-typed} Every mutation MUST be one member of a closed, versioned operation
  vocabulary; unknown fields, arbitrary property paths, document deletion, and
  type-incompatible operations MUST be rejected before writing.
- {#c-batch} Coding agents MUST be able to submit multiple typed operations in
  one deterministic batch and MUST be able to preview the resulting plan and
  diagnostics without writing.
- {#c-atomic} A mutation MUST validate the complete candidate workspace before
  persistence and MUST commit all affected files together or leave every
  original file unchanged.
- {#c-concurrency} A mutation MUST support content fingerprints and MUST reject
  stale documents before applying selectors or preparing writes.
- {#c-lossless} Bytes outside the selected semantic fields MUST remain
  byte-identical, including BOM, line endings, frontmatter comments and order,
  list style, and surrounding CommonMark prose.
- {#c-no-delete} No supported mutation surface MUST expose an operation that
  deletes a specification document.
- {#c-work-items} TASK mutations MUST expose progress, addressing, labels,
  grouping, blocking, and completion checkpoints as work-item operations and
  MUST reject refinement, categorization, or implementation-adherence
  operations on TASK documents.
- {#c-shared} The CLI, language server, and editor integrations MUST compile
  human actions into the same Rust operation enum and transaction engine.
:::

The authoritative contract is the
[mutation module](spec:src:spec-cli/src/mutation/mod.rs), exposed by the
[CLI command tree](spec:src:spec-cli/src/cli.rs) and
[language server](spec:src:spec-cli/src/lsp.rs).
