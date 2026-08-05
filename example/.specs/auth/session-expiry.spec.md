---
id: REQ:auth/session-expiry
type: requirement
status: draft
level: MUST
summary: >
  Session tokens are invalidated after bounded wall-clock and idle intervals,
  and on credential rotation.
owners: [carlo]
refines:
  - REQ:auth/session-management#c-lifetime
  - REQ:auth/session-management#c-idle
aspects: [duration, activity]
related: [INV:auth/no-stale-tokens, IFC:auth/session-api]
categorized_under: [TOPIC:topics/auth]
pinned_at: 7c3a9f1
---

# Session expiry policy

## Context

Sessions persist across browser restarts; the auth subsystem must revoke
them under defined conditions. See
[the storage decision](spec:ADR:auth/0001-session-storage).

:::{requirement id="timeout-policy" level="MUST"}
A session token MUST be invalidated when any of the following holds:

1. Wall-clock age >= 30 days from issuance.
2. Idle interval >= 14 days since last authenticated request.
3. The user has rotated credentials after issuance.
:::

:::{invariant id="no-stale-tokens"}
For all token t in the active set: age(t) < 30d and idle(t) < 14d.

Enforcement point:
[session.ts:8-19](spec:src:example/packages/auth/session.ts:8-19).
:::

:::{non-goal id="no-sliding-window"}
Sliding-window refresh on every request is explicitly out of scope.
:::
