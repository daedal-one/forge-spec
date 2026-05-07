# forge-spec

A file format and mini-toolchain for project specifications.

Instead of scattering requirements, design decisions, and documentation across markdown
files, code comments, and issue trackers, forge-spec provides a single source of
truth: `.specs/` files in the repo.

Each spec is a standalone document with structured metadata and a clear prose body:
it's markdown++ for your vibe coding needs.

Precise enough for coding agents, readable for humans, versioned in git,
mechanically validated.

## What's here

- `specification.md` — the full Specs Format v0.1 specification
- `format-memo.md` — one-page cheat sheet
- `AGENTS.md` — compact reference for AI coding agents
- `example/` — a working `.specs/` tree demonstrating the format
- `spec-cli/` — Rust implementation of the `spec` CLI

## Building the CLI

```sh
cd spec-cli
cargo build --release
# binary at target/release/spec
```

## Quick start

```sh
spec new REQ auth/session-expiry    # scaffold a spec
spec lint                           # validate .specs/
spec render REQ:auth/session-expiry --target=agent
spec children REQ:auth/session-management
spec coverage REQ:auth/session-management
spec graph --refinement             # DOT output
```

Run `spec --help` for the full command list.

## Spec format at a glance

Specs are `.spec.md` files under `.specs/` with YAML frontmatter and a CommonMark body containing typed blocks:

```markdown
---
id: REQ:auth/session-expiry
type: requirement
status: draft
version: 0.1.0
level: MUST
summary: Sessions expire after bounded intervals.
owners: [carlo]
refines: [REQ:auth/session-management#c-lifetime]
---

:::{requirement id="timeout-policy" level="MUST"}
A session token MUST be invalidated when wall-clock age >= 30 days.
:::
```

Cross-reference other specs with `[text](spec:REQ:auth/foo)`. Link to source with `spec:src:path/file.ts:42-78`.

See `specification.md` for the full format definition.
