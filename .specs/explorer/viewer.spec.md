---
id: REQ:explorer/spec-viewer
type: requirement
status: accepted
level: MUST
summary: >
  Each .spec.md file has a rendered viewer with status, metadata inspection and
  editing, hyperlinks, and a direct path back to raw Markdown.
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
Edits to supported metadata MUST be applied as minimal workspace text edits,
MUST preserve unrelated frontmatter, and MUST fail rather than overwrite a
newer document version.
:::

:::{requirement id="links" level="MUST"}
The viewer MUST make `spec:` and `spec:src:` links interactive and MUST route
them through forge-spec reference resolution before opening the target.
:::

:::{requirement id="source-mode" level="MUST"}
The viewer MUST provide an explicit command to open the same document in the
normal Markdown text editor.
:::

The implementation is split between the
[custom editor](spec:src:editors/vscode/src/viewer.ts) and its
[minimal frontmatter edits](spec:src:editors/vscode/src/frontmatter.ts).
