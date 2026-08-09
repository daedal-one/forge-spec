---
id: REQ:explorer/root-to-code-tree
type: requirement
status: accepted
level: MUST
summary: >
  A native editor tree presents the configured project root and its hierarchy
  through clauses, refining specs, tasks, and explicit code relationships.
owners: [carlo]
refines:
  - REQ:explorer/workbench#tree
  - REQ:explorer/workbench#navigation
aspects: [structure, navigation]
categorized_under: [TOPIC:explorer/forge-spec]
related: [REQ:explorer/spec-viewer]
---

# Root-to-code tree

:::{requirement id="roots" level="MUST"}
The tree MUST place the configured PROJECT document at its sole root and MUST
place specifications without a resolvable refinement or categorization parent
as implicit children of that project.
:::

:::{requirement id="dag" level="MUST"}
The tree MUST preserve clause-qualified refinement relationships, MUST support
the same specification appearing below multiple parents, and MUST prevent
cycles from causing recursive expansion.
:::

:::{requirement id="state" level="MUST"}
Every specification row MUST use the document's Markdown title as its concise
label, MUST convey entity type through its icon without repeating the type
prefix in the label, and MUST keep the complete ID and summary available in its
tooltip. Typed block and clause rows MUST likewise use their icon and tooltip
for kind information instead of repeating the kind beside the anchor label.
Lifecycle status and TASK progress MUST remain visible as secondary state.
:::

:::{requirement id="code" level="MUST"}
Every specification MUST expose its explicit `spec:src:` references as code
children that retain file, line-range, or symbol identity.
:::

:::{requirement id="open" level="MUST"}
Activating a specification, typed block, clause, or code node MUST open its
exact native target. Typed blocks and clauses MUST open the owning specification
in a focused forge-spec view of that anchor, while source references MUST reveal
the addressed file, range, or symbol.
:::

The VS Code implementation is the native
[tree provider](spec:src:editors/vscode/src/tree.ts).
