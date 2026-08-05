# Working with specs

This repo uses the **Specs Format v0.2** to capture requirements, invariants, interfaces, ADRs, and glossary entries as version-controlled, cross-referenced documents.

## Quick rules

- Specs live in `.specs/` as `.spec.md` files with YAML frontmatter + CommonMark body.
- Every spec has an `id` of the form `TYPE:namespace/slug` (e.g. `REQ:auth/session-expiry`).
- `.specs/_config.toml` declares `baseline = "forge-spec-v0.2.0"` once; per-file revisions are derived from Git.
- Types: `REQ` requirement, `INV` invariant, `IFC` interface, `ADR` decision record, `GLO` glossary, `TOPIC` grouping, `SCN` scenario.
- Cross-reference with `[text](spec:REQ:auth/session-expiry)` links. Source refs: `spec:src:path/file.ts:42-78` or `spec:src:path/file.ts#symbol=Type/method`.
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
```

## Specification reference

Full spec: `specification.md`. Key sections by line number:

| Topic | Lines | Section |
|-------|-------|---------|
| File layout & ID format | 43-112 | 2.1-2.3 |
| Frontmatter fields & typed blocks | 118-188 | 3.1-3.3 |
| Entity types & type-specific fields | 189-256 | 4-4.1 |
| Reference syntax & resolution | 257-296 | 5 |
| Refinement, categorization, coverage | 297-347 | 6 |
| Git trailers & history | 348-402 | 7 |
| Render targets (human & agent XML) | 403-479 | 8 |
| Lint rules R001-R024 | 480-518 | 9 |
| CLI and LSP integration | 519-573 | 10-10.1 |
| Worked file template | 574-626 | 11 |
