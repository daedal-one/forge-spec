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

- `specification.md` — the full Specs Format v0.3 specification
- `format-memo.md` — one-page cheat sheet
- `AGENTS.md` — compact reference for AI coding agents
- `example/` — a working `.specs/` tree demonstrating the format
- `spec-cli/` — Rust implementation of the `spec` CLI
- `skills/forge-spec/` — installable agent skill for adopting the format

## Install the CLI

On macOS or Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/daedal-one/forge-spec/master/install.sh | sh
```

The installer requires Git, Rust, and Cargo. It makes a temporary shallow
checkout, installs the CLI, and verifies the resulting `spec` command.

From an existing checkout on any platform, including Windows, run:

```sh
cargo install --path spec-cli --locked
```

To build a local checkout instead:

```sh
cd spec-cli
cargo build --release
# binary at target/release/spec
```

## Install the agent skill

Install the `forge-spec` skill into the current project for Codex, Claude Code,
Cursor, OpenCode, or another supported coding agent:

```sh
npx skills add daedal-one/forge-spec --skill forge-spec
```

Add `--global` to make the skill available across projects. The skill inspects
the target project, initializes or migrates its spec tree, authors a focused
starting set of specs, connects them to source, and validates the result.

## Quick start

```sh
spec init                           # create config + draft PROJECT root
spec new REQ auth/session-expiry    # scaffold a spec
spec lint                           # validate .specs/
spec render REQ:auth/session-expiry --target=agent
spec render project --target=agent    # project intent alone
spec render REQ:auth/session-expiry --target=agent --include-source
spec symbols src/session.rs --query expire
spec resolve 'spec:src:src/session.rs#symbol=SessionStore/expire'
spec lsp                            # editor LSP over stdio
spec children REQ:auth/session-management
spec coverage REQ:auth/session-management
spec graph                          # project-rooted hierarchy as DOT
spec graph --refinement             # refinement DAG only
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

Every tree begins with one configured project description:

```markdown
---
id: PROJECT:example
type: project
status: accepted
summary: The purpose and boundaries shared by every descendant specification.
owners: [carlo]
---

# Example

## Purpose
...
```

Requirements and the other entity types then capture increasingly specific
parts of that intent:

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
baseline = "forge-spec-v0.3.0"
project = "PROJECT:example"
```

The project document is ambient context: documents without an explicit
refinement or categorization parent are attached to it implicitly. This does
not turn project prose into a requirement or change the meaning of `refines`.
Human and agent renders include the project description before the requested
specification.

Per-file revisions are derived from Git (`rN`, or `rN+dirty` for working-tree
changes), so spec files do not carry version bookkeeping.

## Migrating format versions

The CLI ships an append-only catalog of adjacent format migrations. It detects
the baseline in `.specs/_config.toml` and composes every required step, so a
repository can move from any supported historical baseline to the current one
in a single invocation.

```sh
spec migrate --guide                  # combined changelog and human instructions
spec migrate --guide --target agent   # structured XML context for an agent
spec migrate                          # apply all mechanical steps and redirects
spec lint                             # validate the migrated tree
```

Use `--from` when an unconfigured legacy tree cannot be inferred and `--to` to
target a specific supported baseline. The baseline is updated only after every
format transformation and reference redirect succeeds.

Source symbols use the repository's language server. Built-in providers are
`rust-analyzer`, `typescript-language-server --stdio`,
`basedpyright-langserver --stdio`, and `sqls`. A project can override providers
in `.specs/_lsp.toml`; custom commands are only executed when the CLI is passed
`--allow-custom-lsp`. Use `spec lint --require-symbols` in CI when provider
availability must be enforced rather than reported as a warning.

See `specification.md` for the full format definition.
