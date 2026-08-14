---
id: TASK:core/unified-tree-state
type: task
status: accepted
summary: Collapse lifecycle and adherence into one deterministic durable-specification tree-row state.
owners: [carlo]
progress: done
addresses: [REQ:core/intellect-provider#c-display]
labels: [state-inference, terminal-presentation]
assignee: carlo
eta:
blocked_by: []
---

# Unified tree state

## Plan

1. Preserve lifecycle and provider adherence as independent durable-specification
   source values; render TASK progress only in explicit work-item views.
2. Derive one effective display state with an explicit precedence function.
3. Render every effective state as one bracket-free glyph and short name.
4. Verify the full lifecycle, adherence, and fallback matrix with
   pure unit tests and a live no-color tree inspection.

## Work-item isolation

An accepted TASK displays its authored `pending`, `in-progress`, `blocked`,
`deferred`, `wontdo`, or `done` workflow state only when work items are
explicitly requested. Completing or reopening it never changes any durable
specification's adherence state.
