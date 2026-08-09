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
Every specification row MUST display its entity type, lifecycle status, and,
for TASK entities, implementation progress.
:::

:::{requirement id="code" level="MUST"}
Every specification MUST expose its explicit `spec:src:` references as code
children that retain file, line-range, or symbol identity.
:::

:::{requirement id="open" level="MUST"}
Activating a specification or code node MUST open its exact file and MUST reveal
the referenced anchor, range, or symbol whenever one is present.
:::

The VS Code implementation is the native
[tree provider](spec:src:editors/vscode/src/tree.ts).
