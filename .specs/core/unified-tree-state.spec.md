---
id: TASK:core/unified-tree-state
type: task
status: accepted
summary: Collapse lifecycle, task progress, and adherence into one deterministic tree-row state.
owners: [carlo]
progress: done
refines: [REQ:core/intellect-provider#c-display]
aspects: [state-inference, terminal-presentation]
assignee: carlo
eta:
blocked_by: []
---

# Unified tree state

## Plan

1. Preserve lifecycle, TASK progress, and provider adherence as independent
   source values.
2. Derive one effective display state with an explicit precedence function.
3. Render every effective state as one bracket-free glyph and short name.
4. Verify the full lifecycle, progress, adherence, and fallback matrix with
   pure unit tests and a live no-color tree inspection.

## Effective TASK transitions

An accepted TASK remains in its authored workflow state while it is `pending`,
`in-progress`, `blocked`, `deferred`, or `wontdo`. Marking it `done` moves the
display into the adherence gate: `unverified`, `current`, `stale`, `partial`,
`violated`, `unknown`, or `unresolved`. A task without a code-adherence
predicate displays `done`. Reopening the task returns it to its authored
workflow state; only complete provider evidence can display `current`.
