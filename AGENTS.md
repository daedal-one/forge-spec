# Working with specs

This repo uses the **Specs Format v0.3** to capture project intent, requirements, invariants, interfaces, ADRs, tasks, and glossary entries as version-controlled, cross-referenced documents.

## Quick rules

- Specs live in `.specs/` as `.spec.md` files with YAML frontmatter + CommonMark body.
- The singleton root uses `PROJECT:slug`; every other spec uses `TYPE:namespace/slug` (e.g. `REQ:auth/session-expiry`).
- `.specs/_config.toml` declares `baseline = "forge-spec-v0.3.0"` and selects `project = "PROJECT:slug"`; per-file revisions are derived from Git.
- Before changing an older tree, run `spec migrate --guide --target agent`; then run `spec migrate` and `spec lint`.
- Types: `PROJECT` root description, `REQ` requirement, `INV` invariant, `IFC` interface, `ADR` decision record, `GLO` glossary, `TOPIC` grouping, `SCN` scenario, `TASK` implementation work.
- Documents without an explicit refinement or categorization parent implicitly descend from PROJECT; containment never implies refinement.
- Cross-reference with `[text](spec:REQ:auth/session-expiry)` links. Source refs: `spec:src:path/file.ts:42-78` or `spec:src:path/file.ts#symbol=Type/method`.
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
spec render REQ:auth/foo --target=agent   # XML envelope for LLM context
spec children REQ:auth/foo     # who refines this?
spec coverage REQ:auth/foo     # clause-by-clause coverage
spec graph                     # DOT output of project hierarchy
spec graph --refinement        # DOT output of refinement DAG only
spec migrate --guide --target agent   # composed migration context
```

## Specification reference

Full spec: `specification.md`. Key sections by line number:

| Topic | Lines | Section |
|-------|-------|---------|
| File layout & ID format | 43-122 | 2.1-2.3 |
| Frontmatter fields & typed blocks | 128-199 | 3.1-3.3 |
| Entity types & type-specific fields | 200-289 | 4-4.1 |
| Reference syntax & resolution | 290-329 | 5 |
| Project containment, refinement, categorization | 330-398 | 6 |
| Git trailers & history | 399-453 | 7 |
| Render targets (human & agent XML) | 454-538 | 8 |
| Lint rules R001-R025 | 539-578 | 9 |
| CLI initialization, migration, and LSP integration | 579-682 | 10-10.2 |
| Worked file template | 683-735 | 11 |
