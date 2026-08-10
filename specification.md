# Specs Format v0.3 — Specification

A file format and toolchain for project specifications, designed to be:

- precise enough for coding agents to consume as authoritative context;
- readable for humans during review and onboarding;
- versioned in git with bidirectional cross-references to commits;
- mechanically validated to prevent silent drift.

This document is non-formal. It defines conventions, file layouts, frontmatter
schemas, validation rules, and the CLI surface in enough detail to start
implementing. Edge cases are listed as open issues at the end rather than
forced into a closed grammar.

---

## 1. Scope and non-scope

In scope:

- Capturing project intent, requirements, invariants, interface contracts,
  architecture decisions, tasks, and glossary entries as first-class entities.
- Cross-referencing entities both within the spec graph and to source code.
- Tracking refinement (a high-level requirement decomposed into
  sub-requirements) at clause-level granularity.
- Linking specs to the commits that touch or implement them.
- Producing two render targets from the same source: human (Markdown for
  Pandoc / Typst / static-site rendering) and agent (structured envelope
  optimized for LLM context).

Not in scope:

- Replacing test suites. Specs describe intent; tests verify code.
- Replacing API documentation generators (OpenAPI etc.). Specs reference
  contracts but do not enumerate every field.
- Symbolic / formal verification. The format is designed to be amenable to
  it later; v0.3 produces no proofs.

---

## 2. Storage and identity

### 2.1 File layout

Specs live under `.specs/` at the repository root. Subdirectory structure
mirrors ID namespaces by convention but is not enforced by the toolchain.

```
.specs/
├── _project.spec.md
├── auth/
│   ├── session-management.spec.md
│   ├── session-expiry.spec.md
│   └── credential-rotation.spec.md
├── infra/
│   ├── observability.spec.md
│   └── deployment.spec.md
├── _glossary/
│   └── terms.spec.md
├── _adrs/
│   ├── 0001-storage-engine.spec.md
│   └── 0002-auth-provider.spec.md
├── _config.toml
├── _redirects.toml
└── _history/                 # generated; tracked
    └── REQ_auth_session-expiry.json
```

Files use the `.spec.md` double extension. This both signals the type to
editors and lets ordinary `*.md` tooling skip them when desired.

### 2.2 Identity

The singleton project document declares an ID of the form:

```
PROJECT:<slug>
```

Every other document declares an `id` in frontmatter of the form:

```
<TYPE>:<namespace>/<slug>
```

`TYPE` is one of the registered prefixes (§4). Slugs and `namespace/slug` are
kebab-case. The slash in non-project IDs is a naming convention, not a
structural pointer to a parent.

Examples:

- `PROJECT:forge-spec`
- `REQ:auth/session-expiry`
- `INV:auth/no-stale-tokens`
- `ADR:auth/0002-auth-provider`
- `GLO:terms/idempotency-key`

Sub-entities inside a document are addressed by URL-fragment-style anchors:

- `REQ:auth/session-management#c-lifetime`

### 2.3 Renames

Renames go through `.specs/_redirects.toml`:

```toml
[[redirect]]
from = "REQ:auth/session-timeout"
to   = "REQ:auth/session-expiry"

[[redirect]]
from = "REQ:auth/session-expiry#timeout"
to   = "REQ:auth/session-expiry#c-lifetime"
```

The linter resolves redirects transitively, but emits an info-level finding
(`R-redir`) so long-lived branches are encouraged to clean up after merge.

Cycles in `_redirects.toml` are an error.

---

## 3. Document anatomy

Each spec is YAML frontmatter delimited by `---`, followed by a CommonMark
body that may contain typed fenced divs.

### 3.1 Universal frontmatter fields

| field        | required           | description                                                        |
|--------------|--------------------|--------------------------------------------------------------------|
| `id`         | yes                | full document ID                                                   |
| `type`       | yes                | entity type; must match the prefix in `id`                         |
| `status`     | yes                | `draft`, `accepted`, `deprecated`, or `superseded`                 |
| `summary`    | conditional        | required if any other spec references this one                     |
| `owners`     | yes                | non-empty list of identifiers                                      |
| `pinned_at`  | no                 | git SHA used to resolve `src:` references                          |
| `related`    | no                 | list of related spec IDs (informational, no graph semantics)       |
| `supersedes` | no                 | spec ID this one replaces                                          |
| `superseded_by` | no              | reverse pointer; auto-managed by `spec migrate`                    |

File revisions are derived from Git rather than stored in frontmatter. The
revision is the number of commits that touched the file, following renames.
Tools render it as `rN`; a changed or untracked working-tree file is rendered
as `rN+dirty`. This value is informational and may change when history is
rebased.

The format baseline and singleton project root are declared once for the entire
spec tree in `.specs/_config.toml`:

```toml
baseline = "forge-spec-v0.3.0"
project = "PROJECT:forge-spec"
```

### 3.2 Body

CommonMark with two extensions:

1. **Typed fenced divs** in MyST / Pandoc style:

   ```
   :::{requirement id="my-clause" level="MUST"}
   Body of the requirement.
   :::
   ```

2. **Clause anchors** inside list items of a typed block:

   ```
   :::{requirement id="session-management" level="MUST"}
   The system MUST manage sessions per:

   - {#c-lifetime} bounded maximum lifetime
   - {#c-idle} expiration on inactivity
   :::
   ```

   The clause anchor (`c-lifetime`) becomes addressable as
   `<doc-id>#c-lifetime`.

Headings (`#`, `##`) are organizational only and produce no addressable
anchors. All addressable IDs come from typed blocks and clause anchors.

### 3.3 Typed block kinds

| block          | purpose                                                                |
|----------------|------------------------------------------------------------------------|
| `requirement`  | observable property the system must / should / may exhibit             |
| `invariant`    | property that must hold across all valid states                        |
| `interface`    | API surface contract (signature, semantics, stability)                 |
| `clause`       | sub-property inside another block; rarely needed at top level          |
| `assumption`   | something taken as given; surfaces dependencies                        |
| `non-goal`     | explicit negative statement; cancels likely misinterpretations         |
| `example`      | concrete walk-through; not normative                                   |
| `glossary-entry` | term + definition; only inside `GLO:` documents                      |

---

## 4. Entity types

Registered prefixes for full document IDs:

| prefix    | name                            | purpose                                              |
|-----------|---------------------------------|------------------------------------------------------|
| `PROJECT` | project                         | purpose, scope, non-goals, and governing principles |
| `REQ`   | requirement                     | a property the system exhibits                         |
| `INV`   | invariant                       | a property that always holds                           |
| `IFC`   | interface                       | API surface contract                                   |
| `ADR`   | architecture decision record    | rationale for an irreversible-ish choice               |
| `GLO`   | glossary                        | term definitions                                       |
| `TOPIC` | topic                           | informal grouping for navigation                       |
| `SCN`   | scenario                        | example walkthrough                                    |
| `TASK`  | task                            | traceable implementation work                          |

Plus one virtual prefix used only inside reference URLs:

- `src:` — references a path (and optional line range) in the working tree,
  resolved against `pinned_at` if set, else HEAD.

### 4.1 Type-specific frontmatter

**`PROJECT` (project)**

Exactly one `PROJECT:` document exists per spec tree and its ID is selected by
`.specs/_config.toml`. It has no fields beyond universal frontmatter. Its body
describes the project's purpose, scope, non-goals, and durable principles.
Project prose provides context; it is not itself a requirement and is never a
target of `refines`.

**`REQ` (requirement)**

| field              | description                                                  |
|--------------------|--------------------------------------------------------------|
| `level`            | `MUST` / `SHOULD` / `MAY` / `INFO`                           |
| `refines`          | list of `REQ:.../#clause` references                         |
| `aspects`          | strings; required when `refines` has more than one parent    |
| `categorized_under`| list of `TOPIC:` IDs                                         |
| `kind`             | optional, e.g. `functional`, `non-functional`, `component`   |
| `level_monotonic`  | bool; default `true`. Opt out of level-monotonicity check.   |

**`INV` (invariant)**

| field         | description                                                      |
|---------------|------------------------------------------------------------------|
| `enforcement` | list of `src:` refs to enforcement points in code                |
| `applies_to`  | list of `REQ:` IDs the invariant supports                        |

**`IFC` (interface)**

| field         | description                                       |
|---------------|---------------------------------------------------|
| `consumed_by` | list of component identifiers                     |
| `provided_by` | list of component identifiers                     |
| `stability`   | `experimental` / `stable` / `frozen`              |

**`ADR` (architecture decision record)**

| field           | description                              |
|-----------------|------------------------------------------|
| `decision_date` | ISO 8601 date                            |
| `decided_by`    | list of identifiers                      |

**`GLO` (glossary)**

A `GLO:` document may contain many `glossary-entry` blocks, each with its own
anchor. The document-level `id` is the file's identity; individual terms are
addressed via anchors:

- `GLO:terms/idempotency-key#idempotency-key`

**`TOPIC` (topic)**

No fields beyond universal. Body contains prose describing the topic.

**`TASK` (task)**

| field               | description                                                   |
|---------------------|---------------------------------------------------------------|
| `progress`          | `pending`, `in-progress`, `done`, `blocked`, `deferred`, or `wontdo` |
| `refines`           | list of `REQ:.../#clause` references                          |
| `aspects`           | strings; required when `refines` has more than one parent     |
| `assignee`          | optional responsible identifier                               |
| `eta`               | optional planned completion date                              |
| `blocked_by`        | list of upstream `TASK:` IDs                                  |
| `categorized_under` | list of `TOPIC:` IDs                                           |

---

## 5. References

### 5.1 Syntax

References are CommonMark links with a `spec:` URL scheme:

```
[the session policy](spec:REQ:auth/session-expiry)
[clause: lifetime cap](spec:REQ:auth/session-management#c-lifetime)
[idempotency key](spec:GLO:terms/idempotency-key)
[session.ts:42-78](spec:src:packages/auth/session.ts:42-78)
[SessionStore/expire](spec:src:packages/auth/session.ts#symbol=SessionStore/expire)
```

This survives any standard Markdown renderer (the URL is shown as a link to
an unknown scheme, link text remains intact). The toolchain rewrites these
links during transmutation.

### 5.2 Resolution

- Plain spec IDs → look up document; resolve `_redirects.toml` transitively.
- `#anchor` suffix → look up doc-local anchor (typed-block id or clause id).
- `src:path:lines` → resolve the one-based inclusive line range against
  `pinned_at` if set on the referencing spec, otherwise the working tree.
- `src:path#symbol=segment/segment` → ask the source language server for the
  named document-symbol hierarchy. Each segment uses URL percent-encoding,
  so `/` remains the hierarchy separator even when a symbol name contains it.

### 5.3 Validation

The linter:

- resolves every `spec:` URL across all documents;
- emits `R005` (error) for dangling references;
- emits `R006` (warning) for references to deprecated specs lacking an
  acknowledgment;
- emits `R-redir` (info) when a reference traverses a redirect.

---

## 6. Hierarchy and refinement

Three semantic relations remain independent. Project containment is
synthesized by the toolchain; the others are declared in frontmatter on the
child:

| relation             | shape  | semantics                                                              |
|----------------------|--------|------------------------------------------------------------------------|
| project containment  | rooted | otherwise-unplaced documents belong to the configured project          |
| `refines`            | DAG    | child's content jointly contributes to parent's satisfaction           |
| `categorized_under`  | tree   | navigational grouping only; no claim about content                     |
| `applies_to`         | n-to-n | child concerns these components / interfaces                           |

### 6.1 Project containment

The configured `PROJECT:` document is the sole root of the navigational
hierarchy. A non-project document with no resolvable `refines` or
`categorized_under` parent receives an implicit containment edge to PROJECT.
Documents with explicit parents reach PROJECT transitively through those
parents. No containment field is repeated in document frontmatter.

Containment means “belongs to this project.” It does not mean that an ADR,
interface, glossary, scenario, topic, or requirement satisfies the project
description. The refinement and categorization graphs therefore remain
available independently. `spec graph` renders the synthesized project
hierarchy by default; `--refinement` and `--categorization` render only their
respective semantic relations.

### 6.2 Refinement

A child requirement names which clauses of which parents it refines:

```yaml
refines:
  - REQ:auth/session-management#c-lifetime
  - REQ:auth/session-management#c-idle
aspects: [duration, activity]
```

Rules enforced by the linter:

- Refinement graph is acyclic. (`R007`)
- Every clause referenced by a child must exist on the parent. (`R008`)
- Level-monotonicity: a `MUST` clause cannot be refined exclusively by
  `SHOULD`/`MAY` children. Opt out with `level_monotonic: false` on the
  parent. (`R009`)
- Coverage: every clause on a non-leaf parent is refined by at least one
  child. Warning, not error. (`R010`)
- `aspects:` is required when `refines` lists more than one parent, or
  more than one clause across different parents. (`R012`)

### 6.3 Categorization

Soft. A document can declare `categorized_under: [TOPIC:auth, TOPIC:security]`.
Topics themselves are documents (`TOPIC:` prefix). The categorization graph
is independent of the refinement graph.

### 6.4 Composition (deferred)

v0.3 does not have a first-class `COMP:` type. If a requirement describes a
component, set `kind: component` on the requirement. Future v2 work may
introduce `COMP:` and migrate `kind: component` requirements to it.

`applies_to:` exists today as a free-form list of component identifiers
(e.g., `[auth-service, gateway]`). It is not validated against any registry
in v0.3.

---

## 7. Git integration

### 7.1 Commit trailers

Commits that touch or implement specs carry typed RFC-822-style trailers:

```
Implement session lifetime cap

Spec-Ref: REQ:auth/session-management#c-lifetime (implements)
Spec-Ref: INV:auth/no-stale-tokens (touches)
```

Trailer kinds:

| kind          | meaning                                                                  |
|---------------|--------------------------------------------------------------------------|
| `refines`     | commit refines or extends the referenced spec                            |
| `implements`  | code change realizes the spec                                            |
| `violates`    | explicit acknowledged deviation; **must** also reference an `ADR:`       |
| `tests`       | test added or modified for the spec                                      |
| `touches`     | generic; default if kind is omitted                                      |

A bare `Spec-Ref: REQ:auth/foo` is parsed as `(touches)`.

### 7.2 History generation

`spec history --update` walks the git log, parses `Spec-Ref:` trailers, and
writes per-spec history files under `.specs/_history/`:

```json
{
  "id": "REQ:auth/session-management",
  "events": [
    {"sha": "7c3a9f1", "kind": "refines",    "date": "2026-04-04", "author": "carlo"},
    {"sha": "a1d2e3b", "kind": "implements", "date": "2026-04-09", "author": "carlo"}
  ]
}
```

History files are committed (not gitignored). This makes history queryable
without re-walking the log and makes diff review meaningful when spec scope
shifts.

### 7.3 `pinned_at`

Each spec may pin a SHA used to resolve `src:` references. If unset,
references resolve against HEAD. Pin when:

- A `draft` spec describes intended behavior on a feature branch;
- A historical spec describes behavior at a specific commit (e.g., a
  superseded ADR).

---

## 8. Transmutation

The `spec render` command produces output bundles tailored to a consumer.

### 8.1 `--target=human` (default, canonical)

Single Markdown file per render. The output is itself plain Markdown,
suitable for further rendering by Pandoc, Typst, or any static-site tool.

- Frontmatter rendered as a header table.
- Cross-references rewritten to relative links targeting the rendered output
  paths.
- Glossary terms referenced in body text auto-link to their definitions on
  first occurrence per document.
- `src:` references rendered as fenced code blocks with file path and line
  numbers shown above the snippet.

### 8.2 `--target=agent`

Same content, structured envelope optimized for LLM consumption:

```xml
<specs project="PROJECT:example">
<project id="PROJECT:example" type="project" status="accepted">
  <summary>Example project intent.</summary>
  <body>...</body>
</project>
<spec id="REQ:auth/session-expiry" type="requirement" status="accepted" level="MUST">
  <summary>Session tokens expire after bounded wall-clock and idle intervals.</summary>
  <body>
    ...
  </body>
  <ancestors>
    <ancestor id="REQ:auth/session-management" level="MUST">
      <summary>...</summary>
    </ancestor>
  </ancestors>
  <descendants>
    <descendant id="REQ:auth/session-rotation" level="SHOULD"/>
  </descendants>
  <referenced-source path="packages/auth/session.ts" lines="42-78">
    <!-- resolved at pinned_at = 7c3a9f1 -->
    ...
  </referenced-source>
</spec>
</specs>
```

Tag boundaries are stable; current-generation LLMs key on them more
reliably than on prose structure. The body inside `<body>` is still the
Markdown source; only the surrounding envelope is structural.

### 8.3 Bundle scoping

```sh
spec render REQ:auth/session-expiry --depth=2 --target=agent
spec render project                    --target=agent
spec render --query 'REQ:auth/*'    --target=human
spec render --since=HEAD~10         --target=agent
```

Default scope:

- The configured project description, in full and first.
- The focal spec(s), in full.
- Ancestors in full up to `--depth` (default one edge).
- Descendants summarized up to `--depth` (default one edge).
- Siblings as IDs only.
- Glossary terms used in any included body, in full.

Flags:

- `--ancestors=full|summary|id-only|none`
- `--descendants=full|summary|id-only|none`
- `--depth=N`
- `--include-source` / `--no-source`

### 8.4 Determinism

Rendering is deterministic: given the same set of input files and SHA, the
output is byte-stable. This makes the agent bundle cacheable and produces
clean diffs when the bundle is checked into a derived directory.

---

## 9. Validation rules

The linter runs a fixed set of checks. Each check has a stable code for
suppression via `# noqa: <code>` comments in frontmatter or as a per-file
config in `.specs/_lint.toml`.

| code        | severity | check                                                                |
|-------------|----------|----------------------------------------------------------------------|
| `R001`      | error    | ID matches `PROJECT:<slug>` or `<TYPE>:namespace/slug`               |
| `R002`      | error    | Type matches ID prefix                                               |
| `R003`      | error    | All universal frontmatter fields present                             |
| `R004`      | error    | Type-specific frontmatter fields present                             |
| `R005`      | error    | Referenced specs exist                                               |
| `R006`      | warning  | Reference does not point at deprecated spec                          |
| `R007`      | error    | Refinement graph acyclic                                             |
| `R008`      | error    | Refinement clauses exist on parent                                   |
| `R009`      | error    | Level-monotonicity (unless opted out)                                |
| `R010`      | warning  | Every clause has at least one refining child                         |
| `R011`      | error    | `summary:` present on referenced specs                               |
| `R012`      | error    | `aspects:` present when refinement is multi-parent                   |
| `R013`      | error    | Commit trailer references resolve                                    |
| `R014`      | error    | No two documents share an ID                                         |
| `R015`      | error    | No two anchors share a (doc, anchor) pair                            |
| `R016`      | warning  | Multi-entity file warning past threshold (configurable, default 10)  |
| `R017`      | warning  | RFC 2119 keyword discipline (a `requirement` block with no MUST/SHOULD/MAY) |
| `R018`      | warning  | TASK has neither a refinement parent nor an upstream blocker        |
| `R019`      | warning  | Deferred/wontdo TASK lacks a summary explaining why                 |
| `R020`      | error    | Source path exists and remains inside the repository                 |
| `R021`      | error    | Referenced source symbol exists                                      |
| `R022`      | warning  | Source language server unavailable (`--require-symbols` makes it an error) |
| `R023`      | error    | Source line range is valid                                           |
| `R024`      | warning  | `.specs/_config.toml` declares the supported format baseline        |
| `R025`      | error    | Exactly one configured PROJECT with a summary exists and is not refined |
| `R-redir`   | info     | Reference traverses a redirect                                       |

`status: draft` downgrades `R002`–`R012` from error to warning. This is the
escape hatch for scratch specs; it is not meant to be permanent.

---

## 10. Tooling

A single CLI: `spec`. Single static binary, no runtime dependency on the
host project.

Subcommands:

| command                             | purpose                                                       |
|-------------------------------------|---------------------------------------------------------------|
| `spec init`                         | initialize config and a draft singleton PROJECT document       |
| `spec new <type> <slug>`            | scaffold a new spec from a per-type template                  |
| `spec lint [--require-symbols]`     | validate specs and source references                          |
| `spec render <id-or-query> [flags]` | produce render bundles                                        |
| `spec graph [--hierarchy\|--refinement\|--categorization]` | emit DOT for the requested graph       |
| `spec history [--update\|<id>]`     | regenerate or query commit history per spec                   |
| `spec children <id>`                | list direct refining children                                 |
| `spec ancestors <id>`               | list direct refined-by parents                                |
| `spec coverage <id>`                | clause-by-clause refinement-coverage report                   |
| `spec impact <id-or-anchor> [--target agent]` | prospective transitive impact report                |
| `spec impact --base B [--head H] [--target agent]` | impact of parsed Git/working-tree changes     |
| `spec orphans`                      | list referenceless leaf specs                                 |
| `spec migrate [--from B] [--to B]` | compose and apply format migrations, then rewrite redirects  |
| `spec migrate --guide [--target agent]` | emit the composed changelog and migration instructions  |
| `spec symbols <path> [--query Q]`   | list language-server symbols for a source file                |
| `spec resolve <reference>`          | resolve a spec or source reference                            |
| `spec lsp`                          | run the forge-spec language server over stdio                 |

Recommended pre-commit hook chain: `spec lint && spec history --update`.

`spec init` creates `.specs/_config.toml` at the current baseline and a draft
`.specs/_project.spec.md`, deriving its `PROJECT:<slug>` ID from the repository
directory. It reuses an existing `.specs/` or `specs/` directory and is
idempotent for an already-current tree. It must not overwrite a different
declared baseline or add a current baseline to an unconfigured tree that
already contains spec documents; those trees must use the migration flow.

### 10.1 Baseline migration

The CLI embeds an append-only catalog of migration artifacts. Each artifact
describes exactly one adjacent baseline transition and contains:

- source and target baselines;
- stable, categorized changelog entries;
- ordered CLI and agent instructions with explicit verification conditions;
- final validation commands; and
- an idempotent mechanical transformation implemented by the CLI.

Migration artifacts compose in release order. Given a tree at
`forge-spec-v0.1.0` and a CLI supporting `forge-spec-v0.4.0`, the CLI plans and
applies `v0.1 -> v0.2 -> v0.3 -> v0.4` in one invocation rather than requiring
a direct migration for every version pair. Historical artifacts remain shipped
with future CLIs.

`spec migrate --guide` renders the combined changelog and instructions as
Markdown. `spec migrate --guide --target agent` renders the same plan as a
structured XML envelope. The source baseline is normally read from
`.specs/_config.toml`; `--from` handles an unconfigured or explicitly selected
source, and `--to` defaults to the newest baseline supported by the CLI.

Applying a migration executes each mechanical transformation and its verifier
in order, then applies `_redirects.toml`, and writes the target baseline last.
Every transformation must be deterministic and idempotent so an interrupted
migration can be rerun safely. A CLI must reject unknown baselines, downgrades,
cycles, ambiguous routes, and a `--from` value that conflicts with a declared
project baseline.

Missing `_config.toml` is inferred as v0.1 when legacy per-file version fields
are present, as v0.3 when a valid PROJECT document exists, and as v0.2
otherwise. The v0.2→v0.3 migration creates a deterministic draft project
document, reuses existing owners, and records the source baseline before the
target baseline so interrupted migrations remain recoverable. Migration
guidance requires the generated purpose, scope, non-goals, principles,
summary, and owners to be reviewed; mechanical migration does not invent
project intent.

### 10.2 Change-impact analysis

`spec impact` is a read-only bridge from changed intent to implementation
review. It has two mutually exclusive modes:

```sh
spec impact REQ:auth/session-management#c-lifetime
spec impact --base origin/main --head working-tree --target agent
```

Subject mode starts from an exact current specification, typed-block anchor, or
clause anchor. Git mode compares the parsed specifications at `--base` with a
revision supplied by `--head`, or with the index plus working tree when head is
omitted or is `working-tree`. Added and removed documents are semantic changes;
edits whose parsed frontmatter, prose, blocks, and clauses are unchanged are
reported as formatting-only and do not trigger a cascade.

For each semantic input, the command traverses refining REQ and TASK documents
transitively. An anchored input follows only refinements of that semantic unit;
a typed-block input also includes its nested clause anchors. Document-level
inputs follow every refinement of that document. A PROJECT input affects every
document because project intent is ambient context. Git mode traverses both the
base and head graphs and unions the results, preserving removed specifications
and relationships for review. Every affected document includes its minimum
depth and one deterministic, clause-qualified explanation path.

Implementation evidence has two explicit confidence classes:

- authored `spec:src:` file, line, and symbol references from every affected
  document in either snapshot; and
- source and test paths changed by historical commits carrying a matching
  typed `Spec-Ref:` trailer.

The report also includes affected TASK progress and gaps where a leaf has no
source or historical implementation evidence, the closure has no attached
TASK, or no explicit or historical test evidence exists. These are review
signals, not proof of the complete runtime dependency graph.

The default human target is Markdown. `--target agent` emits the same facts in
a deterministic XML document rooted at
`<forge-spec-impact schema-version="1">`, including inputs, affected specs,
paths, sources, history, tasks, gaps, notes, and handoff instructions. The
command never creates TASK documents, changes task state, or edits code.

### 10.3 Language-server integration

`spec lsp` is a Language Server Protocol server for `.spec.md` files. It
publishes lint diagnostics for unsaved buffers and provides completion, hover,
definition, references, and document symbols. Source-symbol completion delegates
to the downstream language server selected from the referenced file extension.

Built-in providers are Rust (`rust-analyzer`), TypeScript/JavaScript
(`typescript-language-server --stdio`), Python
(`basedpyright-langserver --stdio`), and SQL (`sqls`). Provider configuration is
stored in `.specs/_lsp.toml`; changing a command or defining another provider is
trusted only with the explicit `--allow-custom-lsp` CLI flag:

```toml
[servers.rust]
extensions = ["rs"]
command = "rust-analyzer"
args = []
root_markers = ["Cargo.toml"]
```

Source paths must be repository-relative and remain within the repository after
canonicalization. Editor clients never opt into custom commands at their
service boundary.

Implementation language is open. A Rust binary using `tree-sitter-markdown`
and `pulldown-cmark` is the recommended choice based on parsing throughput
and the desire to ship a single static binary.

---

## 11. Worked file template

A typical requirement file:

```markdown
---
id: REQ:auth/session-expiry
type: requirement
status: draft
level: MUST
summary: >
  Session tokens are invalidated after bounded wall-clock and idle intervals,
  and on credential rotation.
owners: [carlo]
refines:
  - REQ:auth/session-management#c-lifetime
  - REQ:auth/session-management#c-idle
aspects: [duration, activity]
related: [INV:auth/no-stale-tokens, IFC:auth/session-api]
pinned_at: 7c3a9f1
---

# Session expiry policy

## Context

Sessions persist across browser restarts; the auth subsystem must revoke
them under defined conditions. See [the storage decision](spec:ADR:auth/0004-session-storage).

:::{requirement id="timeout-policy" level="MUST"}
A session token MUST be invalidated when any of the following holds:

1. Wall-clock age ≥ 30 days from issuance.
2. Idle interval ≥ 14 days since last authenticated request.
3. The user has rotated credentials after issuance.
:::

:::{invariant id="no-stale-tokens"}
∀ token t in the active set: age(t) < 30d ∧ idle(t) < 14d.

Enforcement point: [session.ts:42-78](spec:src:packages/auth/session.ts:42-78).
Symbolic enforcement point:
[SessionStore/expire](spec:src:packages/auth/session.ts#symbol=SessionStore/expire).
:::

## Non-goals

- Sliding-window refresh on every request — see
  [no sliding window](spec:ADR:auth/0007-no-sliding).
```

---

## 12. Open issues for v0.3 → v1

- **Component first-class type (`COMP:`).** Deferred. Trigger condition for
  introducing it: more than ~5 requirements with `kind: component` and a
  need to talk about components without an associated requirement.

- **Test integration.** A `tests:` field on requirements pointing at test
  IDs or paths. Useful but requires a parallel test-ID convention; deferred
  until that exists.

- **Cross-repo references.** Useful for monorepo / multi-repo organizations.
  The `spec:` URL scheme is extensible (`spec:other-repo/REQ:foo`); resolution
  needs a registry. Out of scope for v0.3.

- **Symbolic clause-coverage proof.** The current coverage check is
  syntactic: every clause has at least one refining child. A semantic check
  — children jointly imply parent — is not feasible in v0.3.

- **Concurrent edits and merge.** No special handling. Standard git merge
  with line-level conflict resolution. Frontmatter is small and conflict-
  prone; consider per-field YAML linting in `spec lint`.

- **Internationalization.** Spec body can be in any language. Linter rules
  that depend on RFC 2119 keywords (`R017`) are English-only. Multi-language
  projects should disable `R017` and rely on `level:` in typed-block
  attributes.
