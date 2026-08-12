# Forge Spec for VS Code

Forge Spec adds a focused specifications workbench to VS Code:

- a collapsible tree from root topics and requirements through refinements,
  clauses, tasks, explicit `spec:src:` links, and associated documentation;
- a parallel Documentation tree organized by configured collection, directory,
  file, and hierarchical heading, with backlinks shown on specifications;
- a themed CommonMark viewer for each `.spec.md` file;
- status, summary, owners, requirement level, and task progress editing through
  Rust's typed mutation engine and the normal VS Code undo, dirty, and save flow;
- a read-only source inspection surface, kept separate from typed authoring;
- navigation for specification, documentation-file, documentation-heading,
  source-range, and source-symbol links;
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
reference resolution, documentation indexing, and mutation planning. Viewer controls send
`forge-spec-change/v1` operations through `forgeSpec/applyChanges`; Rust returns
versioned workspace edits, and VS Code applies them only while the document
version still matches. File watchers cover `.specs` plus Markdown, but the Rust
service indexes only files matched by configured documentation collections.
Enrolled Markdown keeps the normal editable Markdown editor and receives
diagnostics, heading symbols, completion, hover, definition, and references.
Window focus and Git branch changes trigger a metadata reconciliation that
only reparses stale spec or documentation files.
