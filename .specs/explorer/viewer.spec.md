---
id: REQ:explorer/spec-viewer
type: requirement
status: accepted
level: MUST
summary: 'Each .spec.md file has a rendered viewer with validated typed metadata editing, hyperlinks, and read-only source inspection.'
owners: [carlo]
refines:
  - REQ:explorer/workbench#viewer
  - REQ:explorer/workbench#metadata
aspects: [rendering, authoring]
categorized_under: [TOPIC:explorer/forge-spec]
related: [REQ:explorer/root-to-code-tree]
---

# Specification viewer

:::{requirement id="render" level="MUST"}
The viewer MUST render CommonMark content and forge-spec typed blocks using the
active editor theme while keeping the underlying text document as its model.
:::

:::{requirement id="metadata-view" level="MUST"}
The viewer MUST show the specification ID, entity type, status, summary, owners,
and applicable type-specific fields such as requirement level or task progress.
:::

:::{requirement id="metadata-edit" level="MUST"}
Supported metadata edits MUST compile into `forge-spec-change/v1` operations,
MUST be validated by the Rust mutation engine, and MUST be returned as
versioned workspace text edits. VS Code MUST apply those edits only while the
document version matches.
:::

:::{requirement id="links" level="MUST"}
The viewer MUST render `spec:` and `spec:src:` references as concise,
human-readable links, including bare references in Markdown and relationships
declared in frontmatter. Authored Markdown link labels MUST be preserved, and
activation MUST route the exact underlying reference through forge-spec
resolution. Specification and clause targets MUST open in the forge-spec native
viewer and reveal the addressed block; `spec:src:` targets MUST open in the
source editor at the addressed file, range, or symbol.
:::

:::{requirement id="requirement-view" level="MUST"}
When opened with a typed-block or clause anchor, the viewer MUST reveal and
visually focus that exact semantic unit, MUST distinguish the focused anchor in
the editor tab, and MUST retain the surrounding specification as its canonical
document context. It MUST list every specification whose canonical `refines`
relationship targets that exact anchor under **Refined by**, and each entry MUST
open the refining specification in its forge-spec native rendering. The list
MUST update with the workspace index.
:::

:::{requirement id="source-mode" level="MUST"}
The viewer MUST provide an explicit read-only source inspection surface
separate from typed authoring controls.
:::

The implementation is split between the
[custom editor](spec:src:editors/vscode/src/viewer.ts), the
[reference-aware Markdown renderer](spec:src:editors/vscode/src/markdown.ts),
the [typed operation compiler](spec:src:editors/vscode/src/frontmatter.ts), and
the Rust-owned [LSP mutation bridge](spec:src:spec-cli/src/lsp.rs).
