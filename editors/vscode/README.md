# Forge Spec for VS Code

Forge Spec adds a focused specifications workbench to VS Code:

- a collapsible tree from root topics and requirements through refinements,
  clauses, tasks, and explicit `spec:src:` links;
- a themed CommonMark viewer for each `.spec.md` file;
- status, summary, owners, requirement level, and task progress editing through
  the normal VS Code undo, dirty, and save flow;
- navigation for specification, source-range, and source-symbol links;
- a Rust-owned in-memory index with a per-workspace SQLite cache.

## Run from this checkout

1. Build the CLI with `cargo build --manifest-path spec-cli/Cargo.toml`.
2. In `editors/vscode`, run `npm install` and `npm run compile`.
3. Open `editors/vscode` in VS Code and run the `Run Extension` launch target,
   or start an Extension Development Host with this directory as its
   `--extensionDevelopmentPath`.
4. Open a repository containing `.specs/_config.toml`.

Set `forgeSpec.serverPath` when the compatible `spec` executable is not on
`PATH`. During local development, `FORGE_SPEC_SERVER_PATH` overrides the
setting.

The persistent database lives in VS Code's workspace-specific extension
storage. It contains only derived data and may be deleted safely. Disable it
with `forgeSpec.cache.enabled`.

## Architecture

The extension deliberately remains a thin client. The Rust language service is
authoritative for parsing, validation, graph construction, cache invalidation,
and reference resolution. File watcher events are scoped to `.specs`, while
window focus and Git branch changes trigger a metadata reconciliation that only
reparses stale files.
