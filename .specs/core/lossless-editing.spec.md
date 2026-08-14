---
id: TASK:core/lossless-editing
type: task
status: accepted
summary: Add an editable document index for byte-preserving semantic rewrites.
owners: [carlo]
progress: done
addresses: [REQ:core/typed-mutation#c-lossless]
assignee: carlo
eta:
blocked_by: []
---

# Lossless editing

## Plan

Index original bytes, BOM, line endings, frontmatter keys, heading paths, typed
blocks, anchored clauses, and references while retaining the semantic parser as
the validation authority.

## Acceptance

Every mutation changes only selected spans and missing or ambiguous headings are
rejected.

Implemented by the span-indexed [editable document
model](spec:src:spec-cli/src/editable.rs).
