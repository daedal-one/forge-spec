---
id: REQ:explorer/incremental-index
type: requirement
status: accepted
level: MUST
summary: >
  A Rust-owned in-memory and SQLite-backed workspace index serves cached specs
  immediately and incrementally reconciles filesystem changes.
owners: [carlo]
refines:
  - REQ:explorer/workbench#incremental
  - REQ:explorer/workbench#authority
aspects: [performance, consistency]
categorized_under: [TOPIC:explorer/forge-spec]
related: [ADR:explorer/vscode-thin-client]
---

# Incremental workspace index

:::{requirement id="persistent" level="MUST"}
The Rust service MUST persist the raw and parsed representation of every
successfully parsed `.spec.md` file in a disposable SQLite database outside the
repository and MUST maintain an in-memory hot index while running.
:::

:::{requirement id="fingerprint" level="MUST"}
A cached file MUST be considered reusable only when its canonical path,
nanosecond modification time, and byte length match; a content hash MAY be used
when filesystem evidence is ambiguous.
:::

:::{requirement id="invalidate" level="MUST"}
The cache MUST be invalidated when its schema version, parser version, declared
spec baseline, or repository identity differs from the active workspace.
:::

:::{requirement id="watch" level="MUST"}
Create, change, and delete notifications for specifications, configuration, and
redirect files MUST update only the affected documents and graph edges after a
short debounce.
:::

:::{requirement id="reconcile" level="MUST"}
Startup, workspace-focus, and Git branch changes MUST trigger a bounded metadata
reconciliation so missed watcher events cannot make the persistent index
authoritative over the filesystem.
:::

:::{requirement id="overlay" level="MUST"}
Unsaved editor buffers MUST overlay the saved index in memory for language
features and validation and MUST NOT be persisted as saved cache state.
:::

:::{requirement id="latency" level="SHOULD"}
On the Codon fixture, a warm tree SHOULD become available within 200 ms, a cold
specification index SHOULD complete within one second, and a single-file change
SHOULD refresh visible state within 250 ms.
:::

The authoritative implementation is the Rust
[workspace index](spec:src:spec-cli/src/workspace.rs), exposed to editors by the
[language server](spec:src:spec-cli/src/lsp.rs).
