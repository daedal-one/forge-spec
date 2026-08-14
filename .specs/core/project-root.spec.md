---
id: REQ:core/project-root
type: requirement
status: accepted
level: MUST
summary: >
  Every specification tree has one configured project description that supplies
  ambient intent and a sole navigation root without weakening refinement semantics.
owners: [carlo]
refines: []
related: [REQ:explorer/root-to-code-tree]
---

# Project root

:::{requirement id="singleton" level="MUST"}
A current spec tree MUST contain exactly one `PROJECT:<slug>` document and its
`_config.toml` MUST select that document.
:::

:::{requirement id="intent" level="MUST"}
The PROJECT document MUST describe project purpose, scope, non-goals, and
durable principles as context rather than as a refinable requirement.
:::

:::{requirement id="containment" level="MUST"}
Every non-project durable specification without a resolvable refinement or
categorization parent MUST implicitly descend from PROJECT in the navigational
hierarchy; explicit refinement and categorization semantics MUST remain
unchanged. TASK work items MUST remain outside that hierarchy.
:::

:::{requirement id="rendering" level="MUST"}
Human and agent render bundles MUST include the configured project description
in full before the focal specification, independently of ancestor depth flags.
:::

:::{requirement id="migration" level="MUST"}
Migration from v0.2 MUST scaffold a deterministic draft project description,
MUST preserve an existing valid PROJECT document, MUST never overwrite a
colliding file, and MUST write the target baseline only after verification.
:::

The authoritative implementation spans the
[project scaffolder](spec:src:spec-cli/src/project.rs),
[configuration and singleton validation](spec:src:spec-cli/src/lint/structural.rs),
[project hierarchy](spec:src:spec-cli/src/graph/build.rs), and
[render scoping](spec:src:spec-cli/src/render/scope.rs).
