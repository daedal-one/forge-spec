# Working with specs

This repo uses the **Specs Format v0.5** to capture project intent, requirements, invariants, interfaces, ADRs, tasks, and glossary entries as version-controlled, cross-referenced documents connected to source code and selected project documentation.

## Quick rules

- Specs live in `.specs/` as `.spec.md` files with YAML frontmatter + CommonMark body.
- The singleton root uses `PROJECT:slug`; every other spec uses `TYPE:namespace/slug` (e.g. `REQ:auth/session-expiry`).
- `.specs/_config.toml` declares `baseline = "forge-spec-v0.5.0"`, selects `project = "PROJECT:slug"`, and defaults `intellect_provider = "forge-intellect"`; per-file revisions are derived from Git.
- `implemented: <full-git-oid>` is an authored last-verified checkpoint. Use `spec implementation status` for derived adherence and `spec implementation verify <id>` to record it safely.
- Before changing an older tree, run `spec migrate plan --target agent`; then run `spec migrate apply` and `spec lint`.
- Types: `PROJECT` root description, `REQ` requirement, `INV` invariant, `IFC` interface, `ADR` decision record, `GLO` glossary, `TOPIC` grouping, `SCN` scenario, `TASK` implementation work.
- Documents without an explicit refinement or categorization parent implicitly descend from PROJECT; containment never implies refinement.
- Cross-reference with `[text](spec:REQ:auth/session-expiry)` links. Source refs: `spec:src:path/file.ts:42-78` or `spec:src:path/file.ts#symbol=Type/method`.
- Documentation remains generic Markdown and is enrolled explicitly by collection. Refer to a file with `spec:doc:docs/guide.md` or a heading with `spec:doc:docs/guide.md#heading=Parent/Child`; documentation links never imply refinement or coverage.
- Requirements use typed blocks: `:::{requirement id="name" level="MUST"} ... :::`.
- Clause anchors inside blocks (`- {#c-lifetime} description`) create addressable sub-properties for refinement.
- Children declare `refines: [REQ:parent#c-clause]` in frontmatter; add `aspects:` when refining multiple parents.

## Commits touching specs

Add a `Spec-Ref:` trailer: `Spec-Ref: REQ:auth/session-expiry (implements)`. Kinds: `implements`, `refines`, `tests`, `violates`, `touches` (default).

## CLI cheatsheet

```
spec init                      # initialize a new .specs tree
spec new REQ auth/foo          # scaffold
spec lint                      # validate all rules (pre-commit)
spec implementation status    # provider-derived adherence for the exact workspace
spec implementation verify REQ:auth/foo
spec implementation provider status  # shared worktree-scoped background provider
spec render REQ:auth/foo --target=agent --include-docs  # XML with referenced docs
spec inspect documentation        # enrolled Markdown and headings
spec inspect backlinks 'spec:doc:README.md'
spec inspect relations REQ:auth/foo     # incoming and outgoing relationships
spec inspect coverage REQ:auth/foo      # clause-by-clause coverage
spec impact REQ:auth/foo#c-id  # cascading spec, task, docs, source, and history impact
spec impact 'spec:doc:docs/guide.md#heading=Parent/Child'
spec impact --base origin/main --target agent  # review changed specs before coding
spec inspect graph hierarchy   # DOT output of project hierarchy
spec inspect graph refinement  # DOT output of refinement DAG only
spec migrate plan --target agent      # composed migration context
spec change batch --from changes.json --dry-run
```

## Specification reference

Full spec: `specification.md`. Key sections by line number:

| Topic | Lines | Section |
|-------|-------|---------|
| File layout & ID format | 45-126 | 2.1-2.3 |
| Frontmatter, collections & typed blocks | 127-227 | 3.1-3.3 |
| Entity types & type-specific fields | 229-319 | 4-4.1 |
| Reference syntax & resolution | 321-376 | 5 |
| Project containment, refinement, categorization | 378-452 | 6 |
| Git trailers, checkpoints & history | 454-521 | 7 |
| Render targets (human & agent XML) | 523-612 | 8 |
| Lint rules R001-R031 | 614-658 | 9 |
| CLI, mutation, migration, impact, LSP, projection & adherence | 660-1048 | 10-10.6 |
| Worked file template | 1050-1101 | 11 |
