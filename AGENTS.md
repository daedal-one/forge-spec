# Working with specs

This repo uses the **Specs Format v0.1** to capture requirements, invariants, interfaces, ADRs, and glossary entries as version-controlled, cross-referenced documents.

## Quick rules

- Specs live in `.specs/` as `.spec.md` files with YAML frontmatter + CommonMark body.
- Every spec has an `id` of the form `TYPE:namespace/slug` (e.g. `REQ:auth/session-expiry`).
- Types: `REQ` requirement, `INV` invariant, `IFC` interface, `ADR` decision record, `GLO` glossary, `TOPIC` grouping, `SCN` scenario.
- Cross-reference with `[text](spec:REQ:auth/session-expiry)` links. Source refs: `spec:src:path/file.ts:42-78`. Knowledge-base refs: `spec:kb:path/to/note.md#heading`.
- Requirements use typed blocks: `:::{requirement id="name" level="MUST"} ... :::`.
- Clause anchors inside blocks (`- {#c-lifetime} description`) create addressable sub-properties for refinement.
- Children declare `refines: [REQ:parent#c-clause]` in frontmatter; add `aspects:` when refining multiple parents.

## Commits touching specs

Add a `Spec-Ref:` trailer: `Spec-Ref: REQ:auth/session-expiry (implements)`. Kinds: `implements`, `refines`, `tests`, `violates`, `touches` (default).

## CLI cheatsheet

```
spec new REQ auth/foo          # scaffold
spec lint                      # validate all rules (pre-commit)
spec render REQ:auth/foo --target=agent   # XML envelope for LLM context
spec children REQ:auth/foo     # who refines this?
spec coverage REQ:auth/foo     # clause-by-clause coverage
spec graph --refinement        # DOT output of refinement DAG
spec kb-refs                   # reverse refs from knowledge base
```

## Specification reference

Full spec: `specification.md`. Key sections by line number:

| Topic | Lines | Section |
|-------|-------|---------|
| File layout & ID format | 44-90 | 2.1-2.2 |
| Frontmatter fields (universal) | 118-131 | 3.1 |
| Typed blocks & clause anchors | 133-173 | 3.2-3.3 |
| Entity types & type-specific fields | 177-242 | 4-4.1 |
| Reference syntax & resolution | 245-278 | 5 |
| Refinement, categorization, coverage | 281-329 | 6 |
| Git trailers & history | 332-384 | 7 |
| Render targets (human & agent XML) | 387-458 | 8 |
| Lint rules R001-R020 | 465-497 | 9 |
| CLI subcommands | 496-520 | 10 |
| Worked file template | 523-572 | 11 |
