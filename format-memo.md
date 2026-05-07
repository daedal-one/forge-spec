# Specs Format — Hands-on Memo

One-page reference. For the full spec, see `specs-format-v0.1.md`.

## File layout

```
.specs/<namespace>/<slug>.spec.md
```

## Minimal frontmatter

```yaml
---
id: REQ:auth/session-expiry
type: requirement
status: draft           # draft | accepted | deprecated | superseded
version: 0.1.0
level: MUST             # MUST | SHOULD | MAY | INFO
summary: Sessions expire after bounded wall-clock and idle intervals.
owners: [carlo]
refines: [REQ:auth/session-management#c-lifetime]
aspects: [duration]     # required if refines has multiple parents
---
```

## Reference syntax

CommonMark links with `spec:` URL scheme.

```
[link text](spec:REQ:auth/session-expiry)
[link text](spec:REQ:auth/session-management#c-lifetime)
[link text](spec:GLO:terms/idempotency-key)
[link text](spec:src:packages/auth/session.ts:42-78)
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

| prefix  | meaning                          |
|---------|----------------------------------|
| `REQ`   | requirement                      |
| `INV`   | invariant                        |
| `IFC`   | interface contract               |
| `ADR`   | architecture decision record     |
| `GLO`   | glossary                         |
| `TOPIC` | informal grouping                |
| `SCN`   | scenario                         |
| `src:`  | (virtual) source-tree reference  |

## Common commands

```sh
spec new REQ auth/foo                     # scaffold from template
spec lint                                  # validate (use as pre-commit)
spec render REQ:auth/foo                   # human-target Markdown bundle
spec render REQ:auth/foo --target=agent    # structured envelope for LLMs
spec children REQ:auth/foo                 # direct refining children
spec ancestors REQ:auth/foo                # direct refined-by parents
spec coverage REQ:auth/foo                 # clause-by-clause coverage
spec graph --refinement                    # DOT of the refinement DAG
spec history --update                      # rebuild .specs/_history/
spec migrate                               # apply _redirects.toml
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
| `R-redir`| reference traversed a redirect (info)                              | rewrite to the canonical target           |

## Status escape hatch

`status: draft` downgrades schema errors (R002–R012) to warnings. Use it
only for scratch work; remove before merging to main.

## Pre-commit hook

```sh
spec lint && spec history --update
```
