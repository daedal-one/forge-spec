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

- `specification.md` — the full Specs Format v0.2 specification
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
spec render REQ:auth/session-expiry --target=agent --include-source
spec symbols src/session.rs --query expire
spec resolve 'spec:src:src/session.rs#symbol=SessionStore/expire'
spec lsp                            # editor LSP over stdio
spec children REQ:auth/session-management
spec coverage REQ:auth/session-management
spec graph --refinement             # DOT output
spec tree                           # printed tree of all specs
spec explore                        # interactive TUI browser
```

Run `spec --help` for the full command list.

## Shell completion

`spec completions <shell>` prints a completion script for `bash`, `zsh`, or `fish`.
It completes subcommands, flags, and — for id-taking subcommands — actual spec IDs
from the current `.specs/` directory (powered by a hidden `spec __complete ids`).

```sh
# fish
spec completions fish > ~/.config/fish/completions/spec.fish

# bash
spec completions bash > /usr/local/etc/bash_completion.d/spec
# or per-session: source <(spec completions bash)

# zsh (ensure the target dir is on $fpath)
spec completions zsh > ~/.zsh/completions/_spec
```

## Spec format at a glance

Specs are `.spec.md` files under `.specs/` with YAML frontmatter and a CommonMark body containing typed blocks:

```markdown
---
id: REQ:auth/session-expiry
type: requirement
status: draft
level: MUST
summary: Sessions expire after bounded intervals.
owners: [carlo]
refines: [REQ:auth/session-management#c-lifetime]
---

:::{requirement id="timeout-policy" level="MUST"}
A session token MUST be invalidated when wall-clock age >= 30 days.
:::
```

Cross-reference other specs with `[text](spec:REQ:auth/foo)`. Link to source by
line with `spec:src:path/file.ts:42-78` or by language-server symbol with
`spec:src:path/file.ts#symbol=Type/method`.

The spec tree declares its format once in `.specs/_config.toml`:

```toml
baseline = "forge-spec-v0.2.0"
```

Per-file revisions are derived from Git (`rN`, or `rN+dirty` for working-tree
changes), so spec files do not carry version bookkeeping.

Source symbols use the repository's language server. Built-in providers are
`rust-analyzer`, `typescript-language-server --stdio`,
`basedpyright-langserver --stdio`, and `sqls`. A project can override providers
in `.specs/_lsp.toml`; custom commands are only executed when the CLI is passed
`--allow-custom-lsp`. Use `spec lint --require-symbols` in CI when provider
availability must be enforced rather than reported as a warning.

See `specification.md` for the full format definition.
