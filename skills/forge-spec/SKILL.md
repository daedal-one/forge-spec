---
name: forge-spec
description: Introduce and maintain forge-spec as version-controlled project specifications. Use when asked to adopt forge-spec in a project, initialize or migrate a .specs tree, author or update requirements, invariants, interfaces, ADRs, glossary entries, scenarios, topics, or spec tasks, connect specs to source code, or validate spec-driven implementation work.
---

# Forge Spec

Use the `spec` CLI to create a small, coherent specification tree that is readable by humans, actionable by coding agents, and validated before handoff.

## Prepare the project

1. Work from the project root and preserve unrelated working-tree changes.
2. Run `spec --version`. When setup is part of the request and the CLI is unavailable, install it:

   ```sh
   curl --proto '=https' --tlsv1.2 -LsSf \
     https://raw.githubusercontent.com/daedal-one/forge-spec/master/install.sh | sh
   ```

   On systems without a POSIX shell, make a shallow clone without submodules
   and run `cargo install --path spec-cli --locked` inside the checkout. Do not
   use `cargo install --git`: Cargo attempts to fetch repository submodules.

3. Inspect the project before writing specifications. Read its agent instructions, primary documentation, public interfaces, important implementation paths, and relevant tests. Treat current behavior as evidence, not automatically as intended behavior.
4. Locate `.specs/` or `specs/`:
   - If neither exists, run `spec init` from the project root, then replace the generated PROJECT placeholders with evidence-backed purpose, scope, non-goals, principles, summary, and owners.
   - If both exist, stop and resolve the intended tree or pass `--specs-dir` explicitly.
   - If a tree lacks `_config.toml` or declares an older baseline, run `spec migrate --guide --target agent`, review the composed guidance, run `spec migrate`, then run `spec lint`. Never stamp the current baseline onto legacy content manually.

## Design the smallest useful tree

Create only specifications that improve an engineering decision or verification boundary. Avoid mirroring every source file.

- Maintain exactly one configured `PROJECT:<slug>` description as the tree's purpose and boundary. Everything else implicitly belongs beneath it, but containment does not mean refinement.
- Use `TOPIC` to group a durable domain.
- Use `REQ` for observable behavior and constraints.
- Use `INV` for a property that must always hold.
- Use `IFC` for a boundary consumed or provided by another component.
- Use `ADR` for a consequential decision and its tradeoffs.
- Use `GLO` when a term could be interpreted differently.
- Use `SCN` for an important end-to-end journey or failure path.
- Use `TASK` for traceable implementation work refining a requirement.

Leave new documents as `draft` until their content has been reviewed or is clearly established by project evidence.

## Author and connect specs

1. Scaffold with `spec new <TYPE> <namespace/slug>` and replace every placeholder.
2. Use stable IDs such as `REQ:auth/session-expiry` and link specs with CommonMark `spec:` links.
3. Put normative behavior in typed blocks:

   ```markdown
   :::{requirement id="session-lifetime" level="MUST"}
   A session MUST expire after its configured maximum lifetime.
   :::
   ```

4. Add clause anchors such as `- {#c-idle} idle expiration` when children need to refine separate properties. Point children at the exact clause with `refines: [REQ:auth/session-management#c-idle]`; add `aspects:` for multiple parents.
5. Connect claims to implementation using `spec:src:path/file.ts#symbol=Type/method` when possible. Run `spec symbols <path> --query <name>` to discover resolvable symbols. Use line ranges only when symbol identity is unavailable and the range is stable.
6. Record genuine cross-spec links. Do not invent dependencies, ownership, decisions, or guarantees that project evidence does not support.

## Validate the result

Run at least:

```sh
spec lint
spec render <relevant-id> --target=agent --include-source
```

Use `spec coverage <parent-id>` for clause refinement, `spec graph` for the project hierarchy, `spec graph --refinement` for refinement alone, and `spec lint --require-symbols` when language-server availability is an enforced requirement. Fix errors before handoff and state warnings or unavailable source-symbol validation plainly.

When implementing from specs, render the relevant scope before editing code and update specs when the agreed contract changes. For commits touching a spec or its implementation, add a trailer such as:

```text
Spec-Ref: REQ:auth/session-expiry (implements)
```

Use `implements`, `refines`, `tests`, `violates`, or `touches` accurately.
