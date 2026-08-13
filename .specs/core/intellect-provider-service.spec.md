---
id: TASK:core/intellect-provider-service
type: task
status: accepted
summary: Add deterministic provider installation and worktree-scoped background lifecycle management.
owners: [carlo]
progress: done
refines:
  - REQ:core/intellect-provider#c-lifecycle
  - REQ:core/intellect-provider#c-standalone
  - REQ:core/intellect-provider#c-service
aspects: [provider-selection, standalone-operation, background-lifecycle]
assignee: carlo
eta:
blocked_by: []
---

# Background intellect-provider service

## Plan

Add provider lifecycle commands beneath `spec implementation`, atomically
ensure and reuse one healthy worktree-scoped background endpoint for adherence
requests, retain stdio only as a direct provider protocol/debug surface, and
validate concurrent startup, idle expiry, stale endpoint, shutdown, and absent
provider behavior.

## Acceptance

`start` launches at most one configured globally resolvable provider and waits
for a successful authenticated health check. Concurrent adherence commands
reuse that process. `status` distinguishes running, stopped, and stale
metadata. `stop` shuts down only the provider registered for the exact
worktree, while inactivity shuts it down automatically. Ordinary forge-spec
commands and read-only adherence degradation remain usable without any
installed provider.
