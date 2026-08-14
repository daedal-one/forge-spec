---
id: REQ:core/change-impact
type: requirement
status: accepted
level: MUST
summary: >
  Humans and coding agents can measure a selected or changed specification's
  explainable refinement, related-work, source, and historical implementation impact
  before editing code.
owners: [carlo]
refines: []
related: [REQ:core/project-root]
---

# Change-impact analysis

:::{requirement id="selection" level="MUST"}
Impact analysis MUST accept either one exact specification or anchor, or a Git
base/head comparison whose head MAY be the working tree. Git comparison MUST
compare parsed specification snapshots and MUST distinguish semantic changes
from formatting-only edits.
:::

:::{requirement id="cascade" level="MUST"}
The durable report MUST traverse refining requirements transitively, MUST
preserve the clause-qualified path that explains every affected specification,
MUST include nested clauses when a typed block changes, and MUST union the old
and new refinement graphs so removed relationships remain reviewable. A PROJECT
change MUST affect every durable specification because project intent is
ambient context; ordinary containment, categorization, and task addressing MUST
NOT be treated as refinement. Addressing TASK work items MUST be reported
separately and MUST NOT extend the durable impact closure.
:::

:::{requirement id="evidence" level="MUST"}
Every affected durable specification's explicit `spec:src:` references MUST be reported
separately from historically inferred implementation and test files recovered
from typed `Spec-Ref:` commit trailers. Evidence gaps MUST remain visible; the
tool MUST NOT present either evidence class as a code-dependency proof. Source
references or legacy trailers attached to TASK work items MUST NOT be promoted
to implementation evidence for addressed specifications.
:::

:::{requirement id="report" level="MUST"}
Human output MUST summarize changed inputs, affected specifications, traversal
depth, implementation surfaces, related work-item state, and evidence gaps. Agent output
MUST expose the same information in a deterministic
`forge-spec-impact` XML envelope with a versioned schema and explicit traversal
paths.
:::

:::{requirement id="read-only" level="MUST"}
Impact analysis MUST be read-only. It MAY recommend rendering affected specs,
creating missing TASK work items, starting existing work, linting, and running
project tests, but MUST NOT create work items, alter work state, or edit code.
:::

The authoritative implementation is the
[impact engine](spec:src:spec-cli/src/impact/mod.rs), exposed through the
[impact command](spec:src:spec-cli/src/commands/impact.rs) and
[CLI contract](spec:src:spec-cli/src/cli.rs), with an end-to-end
[CLI impact test](spec:src:spec-cli/tests/impact_cli.rs).
