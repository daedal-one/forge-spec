# Specs Format — Hands-on Memo

One-page reference for Specs Format v0.3. For the full spec, see `specification.md`.

## File layout

```
.specs/<namespace>/<slug>.spec.md
.specs/_project.spec.md
```

Every tree has one `PROJECT:<slug>` document. All other documents implicitly
descend from it when they do not have an explicit refinement or categorization
parent.

## Minimal frontmatter

```yaml
---
id: REQ:auth/session-expiry
type: requirement
status: draft           # draft | accepted | deprecated | superseded
level: MUST             # MUST | SHOULD | MAY | INFO
summary: Sessions expire after bounded wall-clock and idle intervals.
owners: [carlo]
refines: [REQ:auth/session-management#c-lifetime]
aspects: [duration]     # required if refines has multiple parents
---
```

Declare the format once in `.specs/_config.toml`:

```toml
baseline = "forge-spec-v0.3.0"
project = "PROJECT:example"
```

File revisions are computed from Git history and shown as `rN` or `rN+dirty`.

## Reference syntax

CommonMark links with `spec:` URL scheme.

```
[link text](spec:REQ:auth/session-expiry)
[link text](spec:REQ:auth/session-management#c-lifetime)
[link text](spec:GLO:terms/idempotency-key)
[link text](spec:src:packages/auth/session.ts:42-78)
[link text](spec:src:packages/auth/session.ts#symbol=SessionStore/expire)
```

## Typed block

```
:::{requirement id="my-clause" level="MUST"}
Body of the requirement.
:::
```

Block kinds: `requirement`, `invariant`, `interface`, `clause`, `assumption`,
`non-goal`, `example`, `glossary-entry`.

## Clause-level refinement

Parent enumerates clauses with anchors:

```
:::{requirement id="session-management" level="MUST"}
- {#c-lifetime} bounded lifetime
- {#c-idle} idle expiration
- {#c-rotation} token rotation on credential change
:::
```

Children declare refinement:

```yaml
refines:
  - REQ:auth/session-management#c-lifetime
```

## Entity prefixes

| prefix    | meaning                          |
|-----------|----------------------------------|
| `PROJECT` | singleton project description    |
| `REQ`   | requirement                      |
| `INV`   | invariant                        |
| `IFC`   | interface contract               |
| `ADR`   | architecture decision record     |
| `GLO`   | glossary                         |
| `TOPIC` | informal grouping                |
| `SCN`   | scenario                         |
| `TASK`  | implementation task              |
| `src:`  | (virtual) source-tree reference  |

## Common commands

```sh
spec init                                  # initialize a new .specs tree
spec new REQ auth/foo                     # scaffold from template
spec lint                                  # validate (use as pre-commit)
spec render REQ:auth/foo                   # human-target Markdown bundle
spec render REQ:auth/foo --target=agent    # structured envelope for LLMs
spec render project --target=agent         # configured project description
spec children REQ:auth/foo                 # direct refining children
spec ancestors REQ:auth/foo                # direct refined-by parents
spec coverage REQ:auth/foo                 # clause-by-clause coverage
spec impact REQ:auth/foo#c-clause          # transitive spec/code impact
spec impact --base origin/main --target=agent # changed-spec impact XML
spec symbols src/lib.rs --query Resolver   # discover source symbols via LSP
spec resolve 'spec:src:src/lib.rs#symbol=Resolver/resolve'
spec lsp                                   # forge-spec editor language server
spec graph                                 # DOT of the project hierarchy
spec graph --refinement                    # DOT of the refinement DAG only
spec history --update                      # rebuild .specs/_history/
spec migrate --guide --target agent        # composed changelog + agent instructions
spec migrate                               # migrate format + apply _redirects.toml
spec orphans                               # leaf specs with no children
```

## Git trailer

```
Spec-Ref: REQ:auth/foo (implements)
```

Kinds: `refines`, `implements`, `violates`, `tests`, `touches`. Bare ID
without parens is parsed as `(touches)`. `(violates)` requires also
referencing an `ADR:` in the same commit.

## Common linter findings

| code     | meaning                                                            | fix                                       |
|----------|--------------------------------------------------------------------|-------------------------------------------|
| `R005`   | dangling reference                                                 | fix the link or run `spec migrate`        |
| `R007`   | refinement cycle                                                   | break the loop                            |
| `R008`   | refinement points at non-existent clause                           | add the clause or fix the ref             |
| `R009`   | level-monotonicity violation                                       | raise child level or set `level_monotonic: false` on parent |
| `R010`   | clause has no refining child (warning)                             | add a child or remove the clause          |
| `R011`   | referenced spec missing `summary:`                                 | add `summary:` to the target              |
| `R012`   | multi-parent refinement without `aspects:`                         | add `aspects:` justifying the split       |
| `R025`   | missing, duplicate, misconfigured, or refined PROJECT root          | repair config/containment, then review `_project.spec.md` |
| `R-redir`| reference traversed a redirect (info)                              | rewrite to the canonical target           |

## Status escape hatch

`status: draft` downgrades schema errors (R002–R012) to warnings. Use it
only for scratch work; remove before merging to main.

## Pre-commit hook

```sh
spec lint && spec history --update
```
