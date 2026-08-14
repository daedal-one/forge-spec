<p align="center">
  <img src="docs/logo.svg" alt="forge-spec logo" width="480">
</p>

# forge-spec

A file format and mini-toolchain for project specifications.

Forge-spec keeps project intent authoritative in `.specs/`, then connects that
intent to source symbols and deliberately selected Markdown without forcing
ordinary documentation into a specification schema.

Each spec is a standalone document with structured metadata and a clear prose body:
it's markdown++ for your vibe coding needs.

Precise enough for coding agents, readable for humans, versioned in git,
mechanically validated.

## What's here

- `specification.md` — the full Specs Format v0.6 specification
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
spec inspect symbols src/session.rs --query expire
spec inspect resolve 'spec:src:src/session.rs#symbol=SessionStore/expire'
spec inspect documentation              # enrolled Markdown + headings
spec inspect backlinks 'spec:doc:docs/operations.md'
spec lsp                            # editor LSP over stdio
spec inspect relations REQ:auth/session-management
spec inspect coverage REQ:auth/session-management
spec impact REQ:auth/session-management#c-lifetime
spec impact --base origin/main --head working-tree --target agent
spec inspect graph hierarchy        # project-rooted hierarchy as DOT
spec inspect graph refinement       # refinement DAG only
spec inspect tree                   # durable specification hierarchy
spec inspect tree --include-tasks   # append a separate WORK ITEMS section
spec inspect graph work             # task addressing/blocking graph
spec implementation status         # provider-derived code adherence
spec implementation verify REQ:auth/session-expiry
spec implementation provider status # shared background provider
spec change summary replace REQ:auth/session-expiry 'Sessions expire.'
spec task start TASK:auth/update-session
spec explore                        # interactive TUI browser
```

Run `spec --help` for the full command list.

## Review change impact

`spec impact` produces a read-only, evidence-based review before implementation.
Select an exact specification or clause to examine prospective impact:

```sh
spec impact REQ:auth/session-management#c-lifetime
spec impact REQ:auth/session-management#c-lifetime --target agent
```

Or compare parsed specification snapshots. The default head is the working
tree, so staged, unstaged, added, and removed spec semantics are reviewed
together:

```sh
spec impact --base origin/main
spec impact --base origin/main --head HEAD --target agent
```

The command follows clause-qualified requirement refinement paths transitively,
reports addressing TASK work items separately, collects explicit `spec:src:` and documentation surfaces, and recovers
implementation and test paths from `Spec-Ref:` history. Git comparisons union
the base and head graphs so deleted specs and removed relationships are not
silently lost. Formatting-only edits are reported without inventing a semantic
cascade.

Documentation files and headings are also valid impact subjects. A changed
document follows explicit spec backlinks into the refinement closure, while
remaining context rather than refinement or coverage evidence.

Human output is a review report. `--target agent` emits the same evidence as a
deterministic `<forge-spec-impact schema-version="2">` XML envelope. Both
outputs identify related work, missing implementation and test evidence, and state the
boundary explicitly: source links and Git history are traceability evidence,
not proof of every runtime code dependency. TASK source links are never promoted
to implementation evidence for addressed specifications. `spec impact` never
creates work items or changes work state; review the report, add missing TASK
work items deliberately, connect them with `spec task address`, then use
`spec task start <task-id>` when work actually begins.

## Shell completion

`spec completions <shell>` prints a completion script for `bash`, `zsh`, or `fish`.
The command levels are generated from the Clap tree; dynamic candidates include
filtered spec IDs, anchors, block and clause IDs, headings, relation targets,
task IDs, and progress states from the current workspace.

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
`spec:src:path/file.ts#symbol=Type/method`. Link to enrolled documentation with
`spec:doc:docs/operations.md` or an exact hierarchical heading such as
`spec:doc:docs/operations.md#heading=Deployment/Rollback`.

The spec tree declares its format once in `.specs/_config.toml`:

```toml
baseline = "forge-spec-v0.6.0"
project = "PROJECT:example"
intellect_provider = "forge-intellect"
```

The project document is ambient context: durable specifications without an
explicit refinement or categorization parent are attached to it implicitly. This does
not turn project prose into a requirement or change the meaning of `refines`.
Human and agent renders include the project description before the requested
specification. TASK work items remain outside this hierarchy and instead point
to one or more durable targets through `addresses`.

Per-file revisions are derived from Git (`rN`, or `rN+dirty` for working-tree
changes), so spec files do not carry version bookkeeping.

## Track implementation adherence

Every durable specification may carry an authored implementation checkpoint:

```yaml
implemented: 0123456789abcdef0123456789abcdef01234567
```

This means complete adherence was last verified at that exact commit; it is
not a manually maintained current/stale flag. Adherence-aware commands
atomically ensure one lightweight provider is running for the Git worktree,
validate the exact HEAD and working-tree state, and pull one evidence-qualified
snapshot. Concurrent commands reuse the same process, which exits after five
idle minutes by default:

```sh
spec implementation status
spec implementation status REQ:auth/session-expiry --json
spec implementation verify REQ:auth/session-expiry
spec implementation provider start
spec implementation provider status
spec implementation provider stop
spec inspect tree
spec render REQ:auth/session-expiry --target agent
```

TASK work items are excluded from provider requests and implementation commands;
their progress and optional `completion_checkpoint` are workflow metadata only.
The default tree hides them, while `--include-tasks` appends a separate work-item
section. v0.6 defaults to and supports only the `forge-intellect` provider, but forge-spec has no build,
installation, or mandatory runtime dependency on it. Lint, migration,
mutation, impact, inspection, LSP, and other non-adherence workflows run
without the provider. Read-only adherence surfaces such as tree, render,
explore, and status also remain usable when it is absent: they warn and show
`unknown`, never `current`. Only verification fails closed, writing nothing
unless the provider reports complete current adherence.

Install the provider separately when adherence is wanted:

```sh
curl -fsSL https://raw.githubusercontent.com/daedal-one/forge-intellect/main/install.sh | sh
forge-intellect --version
```

The provider binds only to loopback and publishes its authenticated endpoint,
PID, timeout, and log beside the worktree's Git administrative data—not in
tracked project bytes. `spec implementation provider start
--idle-timeout-seconds N` overrides the idle lifetime for an explicit start.

For a fresh checkpoint, the v0.6 forge-intellect provider also requires the
candidate commit to carry `Spec-Ref: <id> (implements)`. This prevents
`verify` from circularly declaring an arbitrary current commit implemented
only because nothing has changed since it.

## Connect project documentation

Generic Markdown remains ordinary Markdown. Projects opt specific files into
named collections in `.specs/_config.toml`:

```toml
[[documentation]]
id = "guides"
title = "Engineering guides"
root = "docs"
include = ["**/*.md"]
exclude = ["generated/**", "vendor/**"]
```

The shared index extracts titles, summaries, hierarchical headings, ordinary
relative Markdown links, `spec:` links, and backlinks. Files must belong to
exactly one collection; forge-spec never auto-enrolls existing Markdown.

```sh
spec change documentation collection-add guides \
  --title 'Engineering guides' --root docs --include '**/*.md' \
  --exclude 'generated/**' 'vendor/**'
spec inspect documentation --collection guides
spec inspect resolve 'spec:doc:docs/operations.md#heading=Deployment/Rollback'
spec render REQ:deploy/release --target agent --include-docs
```

The LSP and VS Code explorer use this same model. Enrolled files get heading
symbols, completion, hover, definition, references, validation, and normal
editable Markdown behavior; unconfigured Markdown stays outside the index.

## Migrating format versions

The CLI ships an append-only catalog of adjacent format migrations. It detects
the baseline in `.specs/_config.toml` and composes every required step, so a
repository can move from any supported historical baseline to the current one
in a single invocation.

```sh
spec migrate plan                     # combined changelog and human instructions
spec migrate plan --target agent      # structured XML context for an agent
spec migrate apply                    # apply all mechanical steps and redirects
spec lint                             # validate the migrated tree
```

Use `--from` when an unconfigured legacy tree cannot be inferred and `--to` to
target a specific supported baseline. The baseline is updated only after every
format transformation and reference redirect succeeds.

## Typed changes in CLI v0.7

The executable is `spec 0.7.0`; the stored document format is
`forge-spec-v0.6.0`. Supported writers compile human commands and editor
actions into the same closed Rust operation enum. A versioned batch can group
changes across the workspace:

```json
{
  "schema": "forge-spec-change/v1",
  "if_match": {
    "REQ:auth/session-expiry": "git-blob:<content-fingerprint>"
  },
  "operations": [
    {
      "op": "summary.replace",
      "spec": "REQ:auth/session-expiry",
      "value": "Sessions expire after inactivity."
    }
  ]
}
```

Preview with `spec change batch --from changes.json --dry-run`; omit
`--dry-run` only after reviewing the deterministic plan. Unknown operations or
fields, stale fingerprints, type-incompatible changes, and newly introduced
lint errors fail before any file is written. Multi-document writes are
all-or-nothing, untouched bytes remain identical, and no document-deletion
operation exists.

Source symbols use the repository's language server. Built-in providers are
`rust-analyzer`, `typescript-language-server --stdio`,
`basedpyright-langserver --stdio`, and `sqls`. A project can override providers
in `.specs/_lsp.toml`; custom commands are only executed when the CLI is passed
`--allow-custom-lsp`. Use `spec lint --require-symbols` in CI when provider
availability must be enforced rather than reported as a warning.

## Canonical library projection

Rust consumers can project a saved `.specs/` tree plus a multi-file in-memory
overlay through `spec_cli::projection`. Overlay entries create, replace, or
delete spec, configuration, redirect, and configured Markdown inputs without
writing them to disk. The result is a deterministic `forge-spec-state-v4`
schema containing normalized durable specifications, a separate work-item
collection, clauses, relationships, documentation, cross-surface links, source
selectors, and diagnostics; invalid intermediate input remains visible as an
invalid state. `SpecState::diff` produces a deterministic
`forge-spec-delta-v4` semantic delta.

The projection surface deliberately performs no language-server lookup and
contains no ledger, session, or temporal-graph policy. Downstream systems may
hash or retain the canonical bytes while forge-spec stays independently useful
as the authority for specification semantics.

See `specification.md` for the full format definition.
