# Specs Format v0.5 — Specification

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
- Cross-referencing entities within the spec graph, to source code, and to
  explicitly enrolled project documentation.
- Tracking refinement (a high-level requirement decomposed into
  sub-requirements) at clause-level granularity.
- Recording the exact commit at which each spec was last verified as
  implemented, and deriving its present code-adherence state through a local
  intellect provider.
- Producing two render targets from the same source: human (Markdown for
  Pandoc / Typst / static-site rendering) and agent (structured envelope
  optimized for LLM context).

Not in scope:

- Replacing test suites. Specs describe intent; tests verify code.
- Replacing documentation systems or API documentation generators (OpenAPI
  etc.). Forge-spec indexes selected Markdown and links it to intent, but the
  Markdown remains independently authored documentation.
- Symbolic / formal verification. The format is designed to be amenable to
  it later; v0.5 produces evidence-qualified states, not proofs.

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
| `implemented`| no                 | full Git object ID at which complete adherence was last verified   |
| `related`    | no                 | list of related spec IDs (informational, no graph semantics)       |
| `supersedes` | no                 | spec ID this one replaces                                          |
| `superseded_by` | no              | reverse pointer; auto-managed by `spec lifecycle supersede`        |

File revisions are derived from Git rather than stored in frontmatter. The
revision is the number of commits that touched the file, following renames.
Tools render it as `rN`; a changed or untracked working-tree file is rendered
as `rN+dirty`. This value is informational and may change when history is
rebased.

The format baseline and singleton project root are declared once for the entire
spec tree in `.specs/_config.toml`:

```toml
baseline = "forge-spec-v0.5.0"
project = "PROJECT:forge-spec"
intellect_provider = "forge-intellect"
```

`intellect_provider` defaults to `forge-intellect` when omitted. v0.5
recognizes that provider name only. This is a provider identity, not an
arbitrary shell command; operational environments may control how the named
binary is resolved without placing executable command lines in project data.

Generic Markdown is enrolled through named collections in the same config:

```toml
[[documentation]]
id = "guides"
title = "Engineering guides"
root = "docs"
include = ["**/*.md"]
exclude = ["generated/**", "vendor/**"]
```

Collection roots, includes, and excludes are repository-relative. Each
enrolled file must belong to exactly one collection. `.spec.md` files are
always excluded from documentation collections. A migration never infers
collections from files already present: enrollment is an explicit project
decision, which keeps generated, vendored, private, and incidental Markdown
outside the knowledge graph unless selected deliberately.

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

Plus two virtual prefixes used only inside reference URLs:

- `src:` — references a path (and optional line range) in the working tree,
  resolved against `pinned_at` if set, else HEAD.
- `doc:` — references an explicitly enrolled repository Markdown path and,
  optionally, one hierarchical heading.

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
[deployment guide](spec:doc:docs/deployment.md)
[rollback procedure](spec:doc:docs/deployment.md#heading=Operations/Rollback)
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
- `doc:path` → resolve a repository-relative Markdown file that belongs to one
  configured documentation collection.
- `doc:path#heading=segment/segment` → resolve an exact hierarchical heading
  path. Heading segments use URL percent-encoding. Hierarchy makes repeated
  titles such as `Setup` unambiguous without relying on renderer-specific
  fragment algorithms.

Enrolled Markdown remains ordinary CommonMark. The index derives its title
from the first heading (falling back to the filename), its summary from the
first prose paragraph, and its outline from headings. It records both
`spec:` links and ordinary relative links between enrolled Markdown files,
normalizing ordinary heading fragments to hierarchical `spec:doc:` targets.
Backlinks are available in both directions between documentation,
specifications, and source references.

### 5.3 Validation

The linter:

- resolves every `spec:` URL across specs and enrolled documentation;
- validates ordinary relative Markdown links between enrolled documents;
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

Documentation links are intentionally not a fourth specification relation.
An enrolled Markdown file can document a spec, and a spec can cite a document,
but neither direction implies project containment, refinement,
categorization, clause coverage, normative authority, or requirement
satisfaction. Tooling exposes documentation as a parallel navigational tree
and as associated context on a spec.

### 6.1 Project containment

The configured `PROJECT:` document is the sole root of the navigational
hierarchy. A non-project document with no resolvable `refines` or
`categorized_under` parent receives an implicit containment edge to PROJECT.
Documents with explicit parents reach PROJECT transitively through those
parents. No containment field is repeated in document frontmatter.

Containment means “belongs to this project.” It does not mean that an ADR,
interface, glossary, scenario, topic, or requirement satisfies the project
description. The refinement and categorization graphs therefore remain
available independently. `spec inspect graph hierarchy` renders the synthesized project
hierarchy; the `refinement` and `categorization` views render only their
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

v0.5 does not have a first-class `COMP:` type. If a requirement describes a
component, set `kind: component` on the requirement. Future v2 work may
introduce `COMP:` and migrate `kind: component` requirements to it.

`applies_to:` exists today as a free-form list of component identifiers
(e.g., `[auth-service, gateway]`). It is not validated against any registry
in v0.5.

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

`spec history rebuild` walks the git log, parses `Spec-Ref:` trailers, and
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

### 7.4 `implemented`

`implemented` is an authored attestation containing one complete Git object
ID. It means the specification's complete normative content was checked
against code at that commit. It is not a cached `current` flag and it is never
advanced by migration, history indexing, or inference. The current adherence
state is derived by the configured intellect provider for the exact selected
HEAD and working-tree identity (§10.6).

Because recording the field itself changes a specification file, provider
comparison ignores only the `implemented:` scalar when deciding whether the
specification's normative content changed. All other frontmatter and body
changes invalidate the earlier verification boundary.

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
- With `--include-docs`, directly referenced documentation files or exact
  heading sections are appended with collection and resolution status.

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
<documentation>
  <document reference="spec:doc:docs/deployment.md#heading=Operations/Rollback"
            collection="guides" status="verified"><![CDATA[...]]></document>
</documentation>
</specs>
```

Tag boundaries are stable; current-generation LLMs key on them more
reliably than on prose structure. The body inside `<body>` is still the
Markdown source; only the surrounding envelope is structural.

### 8.3 Bundle scoping

```sh
spec render REQ:auth/session-expiry --depth=2 --target=agent
spec render project                    --target=agent
spec render 'REQ:auth/*'            --target=human
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
- `--include-source`
- `--include-docs`

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
| `R026`      | error    | Documentation collections are safe, named, non-overlapping, and resolvable |
| `R027`      | error    | `spec:doc:` path targets are enrolled                               |
| `R028`      | error    | `spec:doc:` heading paths resolve uniquely                          |
| `R029`      | error    | Ordinary relative Markdown links resolve within enrolled documentation |
| `R030`      | error    | `implemented` is a full 40- or 64-character Git object ID            |
| `R031`      | error    | Configured intellect provider is supported by the active baseline     |
| `R-redir`   | info     | Reference traverses a redirect                                       |

`status: draft` downgrades `R002`–`R012` from error to warning. This is the
escape hatch for scratch specs; it is not meant to be permanent.

---

## 10. Tooling

The core CLI is a single `spec` binary with no runtime dependency on the host
project. Adherence-aware commands additionally require the named local
intellect provider described in §10.6.

Subcommands:

| command                             | purpose                                                       |
|-------------------------------------|---------------------------------------------------------------|
| `spec init`                         | initialize config and a draft singleton PROJECT document       |
| `spec new <type> <slug>`            | scaffold a new spec from a per-type template                  |
| `spec lint [--require-symbols]`     | validate specs, documentation, and source references           |
| `spec render <id-or-query> [flags]` | produce render bundles                                        |
| `spec inspect tree`                 | print the project-rooted specification tree                   |
| `spec inspect graph [view]`         | emit hierarchy, refinement, or categorization DOT             |
| `spec inspect relations <id>`       | report incoming and outgoing relationships                    |
| `spec inspect coverage <id>`        | clause-by-clause refinement-coverage report                   |
| `spec inspect orphans`              | list specs without refinement relationships                   |
| `spec inspect documentation`        | list enrolled documents and hierarchical headings             |
| `spec inspect backlinks <reference>`| show incoming links from specs or documentation                |
| `spec inspect resolve <reference>`  | resolve a spec, documentation, or source reference             |
| `spec inspect symbols <path>`       | list language-server symbols for a source file                |
| `spec impact <id-or-anchor> [--target agent]` | prospective transitive impact report                |
| `spec impact --base B [--head H] [--target agent]` | impact of parsed Git/working-tree changes     |
| `spec change ...`                   | compile human changes into typed workspace operations         |
| `spec change batch --from F`        | apply or dry-run a versioned multi-operation request          |
| `spec change documentation collection-add ...` | enroll one named Markdown collection             |
| `spec rename <id> <new-id>`         | rename a spec, incoming references, config, and redirect      |
| `spec lifecycle ...`                | draft, accept, deprecate, or atomically supersede             |
| `spec relation ...`                 | refine, categorize, or relate specifications                  |
| `spec task ...`                     | list and update typed TASK state, ownership, and schedule     |
| `spec implementation provider start\|status\|stop` | manage the shared worktree provider              |
| `spec implementation status [id]`  | pull current adherence state from the intellect provider       |
| `spec implementation verify <id>`  | verify adherence and record the exact successful checkpoint    |
| `spec implementation clear <id>`   | remove the authored implementation checkpoint                  |
| `spec history show\|rebuild`        | query or atomically regenerate derived Git history            |
| `spec migrate plan\|apply`          | safely inspect or apply composed format migrations            |
| `spec lsp`                          | run the forge-spec language server over stdio                 |
| `spec completions <shell>`          | generate nested, workspace-aware completions from Clap        |

There are no compatibility aliases or raw `edit`, `patch`, `set`, `delete`, or
`remove-document` commands. Recommended pre-commit hook chain:
`spec lint && spec history rebuild`.

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

Migration artifacts compose in format release order. Given a tree at
`forge-spec-v0.1.0`, CLI v0.7 plans and applies
`v0.1 -> v0.2 -> v0.3 -> v0.4 -> v0.5` in one
invocation rather than requiring a direct migration for every version pair.
The CLI release does not create a format migration when stored document syntax
is unchanged. Historical artifacts remain shipped with future CLIs.

`spec migrate plan` renders the combined changelog and instructions as
Markdown. `spec migrate plan --target agent` renders the same plan as a
structured XML envelope. The source baseline is normally read from
`.specs/_config.toml`; `--from` handles an unconfigured or explicitly selected
source, and `--to` defaults to the newest baseline supported by the CLI.

`spec migrate apply` executes each mechanical transformation and its verifier
in order, then applies `_redirects.toml`, and writes the target baseline last.
Every transformation must be deterministic and idempotent so an interrupted
migration can be rerun safely. A CLI must reject unknown baselines, downgrades,
cycles, ambiguous routes, and a `--from` value that conflicts with a declared
project baseline.

Missing `_config.toml` is inferred as v0.1 when legacy per-file version fields
are present, as the current v0.5 shape when a valid PROJECT document exists,
and as v0.2
otherwise. The v0.2→v0.3 migration creates a deterministic draft project
document, reuses existing owners, and records the source baseline before the
target baseline so interrupted migrations remain recoverable. Migration
guidance requires the generated purpose, scope, non-goals, principles,
summary, and owners to be reviewed; mechanical migration does not invent
project intent.

The v0.3→v0.4 migration adds documentation collections and references as an
optional capability. Its mechanical step updates the baseline only. It never
scans the repository to invent collections; maintainers add reviewed roots and
patterns explicitly with `spec change documentation collection-add`.

The v0.4→v0.5 migration adds
`intellect_provider = "forge-intellect"`, advances the baseline, and leaves
`implemented` absent on every existing specification. It never invents code
adherence. Maintainers establish checkpoints individually with
`spec implementation verify` after provider-backed review.

### 10.2 Change-impact analysis

`spec impact` is a read-only bridge from changed intent to implementation
review. It has two mutually exclusive modes:

```sh
spec impact REQ:auth/session-management#c-lifetime
spec impact 'spec:doc:docs/session-operations.md#heading=Expiry/Recovery'
spec impact --base origin/main --head working-tree --target agent
```

Subject mode starts from an exact current specification, typed-block anchor,
clause anchor, enrolled documentation file, or documentation heading. Git mode
compares the parsed specifications and enrolled documentation at `--base` with a
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

A documentation change follows explicit spec-to-document backlinks before
cascading through refinements; it does not create a refinement edge of its own.
Reports also list documentation referenced by affected specs and enrolled
documents that link back to them. Documentation is context and traceability,
not implementation or test coverage evidence.

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
paths, documentation, sources, history, tasks, gaps, notes, and handoff instructions. The
command never creates TASK documents, changes task state, or edits code.

### 10.3 Language-server integration

`spec lsp` is a Language Server Protocol server for `.spec.md` files and
Markdown enrolled by documentation collections. It publishes lint diagnostics
for unsaved buffers and provides completion, hover, definition, references,
and document symbols using the shared workspace index. Enrolled Markdown gets
ordinary editable Markdown behavior plus heading symbols and `spec:doc:`
navigation; unconfigured Markdown remains inert. Source-symbol completion
delegates to the downstream language server selected from the referenced file
extension.

The custom `forgeSpec/applyChanges` request accepts the same
`forge-spec-change/v1` operation objects as `spec change batch`. Rust checks the
open document version, computes a validated workspace edit, and returns a
versioned `forge-spec-workspace-edit/v1` response. An editor applies that edit
only while every addressed open document still matches its expected version;
the editor does not maintain a second metadata rewrite implementation.

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

### 10.4 Typed workspace mutation

CLI v0.7 implements the v0.5 document format. Every supported document writer
and documentation-collection mutation uses one Rust transaction engine. The
public batch envelope is:

```json
{
  "schema": "forge-spec-change/v1",
  "if_match": {"REQ:auth/session": "git-blob:<fingerprint>"},
  "operations": [
    {
      "op": "content.clause.replace",
      "spec": "REQ:auth/session",
      "block": "session",
      "clause": "c-lifetime",
      "markdown": "The session MUST expire after 30 minutes."
    }
  ]
}
```

The operation name selects a closed Serde-tagged Rust enum. Unknown names,
extra fields, arbitrary property paths, type-incompatible targets, stale
fingerprints, and document deletion are rejected. Human commands compile to
the same enum.

The editable-document index records original bytes, BOM, line endings,
frontmatter key spans, CommonMark heading paths, typed blocks, clause anchors,
and references. Missing or ambiguous headings fail. Applying a batch mutates
candidate documents in memory, rebuilds the complete registry, checks
structural/reference/refinement/content rules, and rejects newly introduced
errors before preparing files. Warnings remain visible but do not block.

Persistence prepares same-directory temporary files only after validation,
commits every affected path together with rollback backups, reloads the written
workspace, and validates it again before discarding backups. `--dry-run`
performs the same planning and validation without writing. Rename is a typed
workspace transaction that preserves entity type, prevents ID/path collision,
updates incoming frontmatter and body references plus project configuration,
and records a redirect. Supersession updates both lifecycle pointers in one
transaction and rejects conflicts or cycles. No `--force` bypass exists.

`documentation.collection.add` is a configuration operation rather than a
spec mutation. It preserves existing TOML content, rejects duplicate IDs and
unsafe or overlapping collections, rebuilds the candidate documentation
index, and commits only when the complete workspace introduces no lint errors.

### 10.5 Canonical overlay projection

The Rust library exposes a read-only projection surface for consumers that need
specification semantics at an unsaved workspace state. Its inputs are one saved
`.specs/` directory and a map of repository-relative overlay entries. An entry
can create or replace bytes, or delete the corresponding saved input. Supported
inputs are `.spec.md`, `_config.toml`, `_redirects.toml`, and generic Markdown
matched by the overlaid configuration's documentation collections; absolute
paths and parent-directory traversal are rejected before projection.

`forge-spec-state-v3` deterministically orders and serializes semantic
configuration, specifications, typed blocks and clause anchors, redirects,
explicit relationships, synthesized PROJECT containment, documentation
collections, document bodies and headings, cross-surface documentation links,
source selectors, and diagnostics. Paths in the schema are relative and use
`/`; absolute host paths are not part of the contract. Invalid UTF-8, YAML,
configuration, redirects, enrollment, links, or semantic rules produce a state
with `valid: false` and sorted diagnostics rather than disappearing from the
projection. Source selectors remain explicit and unverified: canonical output
never depends on language-server availability.

`forge-spec-delta-v3` compares two canonical states and reports added, removed,
and changed specifications and documentation plus documentation-link,
redirect, relationship, source-reference, diagnostic, validity, and
configuration changes. Projecting or diffing performs no workspace writes.
Identical saved and overlaid bytes must produce identical canonical state
bytes.

### 10.6 Intellect providers and code adherence

Forge-spec owns authored intent and the durable `implemented` checkpoint.
Current adherence is derived observation: forge-spec asks an intellect
provider to assess those authored inputs against one exact repository state,
then presents the result without writing it back into lifecycle or TASK
fields. Forge-intellect is the only provider identity defined by v0.5.

Adherence-aware commands discover or atomically start one lightweight
`forge-intellect provider serve` process scoped to the Git worktree. Concurrent
commands reuse its authenticated loopback endpoint; they do not spawn one
provider each. The server removes its registration and terminates after five
idle minutes by default. `spec implementation provider start`, `status`, and
`stop` expose explicit lifecycle control, and `start --idle-timeout-seconds N`
selects another positive timeout.

Registration, startup lock, PID, endpoint, bearer token, timeout, and log live
beside the worktree-specific Git administrative directory, outside tracked
workspace bytes. Startup uses exclusive file creation and a second health
check under the lock, so concurrent callers converge on one process. Stale
registration never counts as healthy and is replaced only while holding that
lock. Shutdown is sent through the authenticated protocol rather than by
killing an unverified PID. The direct `forge-intellect provider --stdio` mode
remains available for protocol diagnostics, but ordinary forge-spec commands
use the shared server.

Every client performs a health handshake and requests one coherent snapshot.
The protocol schema is `forge-spec-intellect/v1`. Each request contains:

- the canonical workspace root, full HEAD object ID, and a deterministic
  identity for all tracked and untracked working-tree bytes;
- every selected specification's ID, type, lifecycle state, relative path,
  authored `implemented` checkpoint, and unique referenced source paths; and
- an optional candidate checkpoint used only by
  `spec implementation verify`.

The provider independently recomputes the workspace identity and rejects a
request if HEAD, working-tree bytes, root, protocol, or specification set is
incoherent. The response repeats that exact workspace state and identifies the
provider name and version. Every per-spec result retains the authored or
candidate checkpoint, one state, an evidence-completeness flag, ordered
reasons, and explicit evidence boundaries. Responses with missing, duplicate,
or unexpected spec IDs are invalid.

The derived state vocabulary is:

| state | meaning |
|-------|---------|
| `unverified` | no implementation checkpoint has been authored |
| `current` | complete provider evidence supports adherence at the exact requested state |
| `stale` | normative spec content or bounded source evidence changed after verification |
| `partial` | some, but not all, required evidence resolved |
| `violated` | explicit evidence records a known deviation, including a later `(violates)` trailer |
| `unknown` | the provider cannot establish a safe evidence boundary |
| `unresolved` | the checkpoint, spec, source, or selected history cannot be resolved |
| `not-applicable` | the entity has no code-adherence predicate, such as a pure topic or glossary entry |

Lifecycle, TASK progress, authored checkpoint, and derived adherence remain
separate values. Render bundles and agent XML retain the full snapshot,
including provider, workspace, completeness, reasons, and evidence. Human tree
output derives exactly one compact effective state per row instead of printing
those independent values as adjacent badges.

The display FSA uses this precedence:

| priority | condition | effective display state |
|----------|-----------|-------------------------|
| 1 | lifecycle is `draft`, `deprecated`, or `superseded` | that lifecycle state |
| 2 | accepted TASK progress is not `done` | `pending`, `in-progress`, `blocked`, `deferred`, or `wontdo` |
| 3 | accepted entity has applicable adherence | `unverified`, `current`, `stale`, `partial`, `violated`, `unknown`, or `unresolved` |
| 4 | adherence is `not-applicable` | `accepted`, or `done` for a completed TASK |
| 5 | a required provider result is absent | `unknown` |

Thus an accepted requirement moves from `? unverified` to `✓ current` after
verification and to `↻ stale` after relevant drift. An accepted TASK stays
`○ pending`, `◐ in-progress`, or another authored workflow state until it is
marked done; `done` enters the adherence gate rather than implying verified
implementation. A completed unverified TASK is therefore `? unverified`, and
only complete evidence makes it `✓ current`. Reopening it returns the display
to the authored workflow state. Every state uses one bracket-free glyph plus a
short color-coded name.

`spec inspect tree`, `spec render`, `spec explore`, and
`spec implementation status` pull one provider snapshot per invocation from
the shared worktree server, starting it if necessary.
Provider absence, timeout, malformed output, incomplete evidence, or abnormal
termination is rendered explicitly as `unknown` with a warning; it is never
converted to `current`. `spec implementation verify` is stricter: it records
the resolved full commit through the shared typed mutation engine only when
the provider returns `current` with complete evidence for that candidate.
Verification failure writes nothing. `spec implementation clear`, lint,
migration, structural inspection, and unrelated mutations remain usable
without starting the provider.

Forge-spec therefore has no build, installation, or mandatory runtime
dependency on forge-intellect. Read-only adherence surfaces are optional
enrichment and remain successful with explicit incomplete `unknown` state when
the provider is unavailable. Only the authoring operation that records a new
verification checkpoint requires a successful provider assessment and fails
closed without one. Forge-intellect is installed independently as the global
`forge-intellect` CLI from its locked `apps/cli` package; installing the
provider does not require its optional CodeGraph sidecar or retained daemon.

The v0.5 forge-intellect implementation does not treat a fresh candidate as
self-verifying merely because no time has elapsed since it. Unless the same
checkpoint is already authored, the candidate commit must carry an exact
`Spec-Ref: <id> (implements)` trailer before freshness and bounded source
evidence can produce `current`. Richer semantic evidence may extend this
provider contract later without weakening the conservative fallback.

The public mutation vocabulary includes
`implementation.checkpoint.set`, `implementation.checkpoint.clear`, and
`intellect.provider.set`. The v0.5 provider name is fixed to
`forge-intellect`; an unsupported value fails lint and cannot be introduced by
the typed command surface.

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

## 12. Open issues for v0.5 → v1

- **Component first-class type (`COMP:`).** Deferred. Trigger condition for
  introducing it: more than ~5 requirements with `kind: component` and a
  need to talk about components without an associated requirement.

- **Test integration.** A `tests:` field on requirements pointing at test
  IDs or paths. Useful but requires a parallel test-ID convention; deferred
  until that exists.

- **Cross-repo references.** Useful for monorepo / multi-repo organizations.
  The `spec:` URL scheme is extensible (`spec:other-repo/REQ:foo`); resolution
  needs a registry. Out of scope for v0.5.

- **Symbolic clause-coverage proof.** The current coverage check is
  syntactic: every clause has at least one refining child. A semantic check
  — children jointly imply parent — is not feasible in v0.5.

- **Concurrent edits and merge.** No special handling. Standard git merge
  with line-level conflict resolution. Frontmatter is small and conflict-
  prone; consider per-field YAML linting in `spec lint`.

- **Internationalization.** Spec body can be in any language. Linter rules
  that depend on RFC 2119 keywords (`R017`) are English-only. Multi-language
  projects should disable `R017` and rely on `level:` in typed-block
  attributes.
