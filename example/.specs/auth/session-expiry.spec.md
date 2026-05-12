---
id: REQ:auth/session-expiry
type: requirement
status: draft
version: 0.1.0
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
[the storage decision](spec:ADR:auth/0001-session-storage). The token
format and rotation flow are documented in the
[session token design note](spec:kb:engineering/auth/session-tokens.md#credential-rotation).

Additional background lives in the
[threat model](spec:kb:engineering/auth/threat-model.md), and an older
note we never wrote up is at
[stale ref](spec:kb:engineering/auth/session-tokens.md#nonexistent-heading).

:::{requirement id="timeout-policy" level="MUST"}
A session token MUST be invalidated when any of the following holds:

1. Wall-clock age >= 30 days from issuance.
2. Idle interval >= 14 days since last authenticated request.
3. The user has rotated credentials after issuance.
:::

:::{invariant id="no-stale-tokens"}
For all token t in the active set: age(t) < 30d and idle(t) < 14d.

Enforcement point: [session.ts:42-78](spec:src:packages/auth/session.ts:42-78).
:::

:::{non-goal id="no-sliding-window"}
Sliding-window refresh on every request is explicitly out of scope.
:::
