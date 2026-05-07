---
id: INV:auth/no-stale-tokens
type: invariant
status: accepted
version: 1.0.0
summary: >
  No session token in the active set may exceed its wall-clock or idle
  time bounds.
owners: [carlo]
enforcement:
  - src:packages/auth/session.ts:42-78
  - src:packages/auth/reaper.ts:10-30
applies_to: [REQ:auth/session-management]
---

# No stale tokens

:::{invariant id="token-bounds"}
For all token t in the active token set:

- age(t) < MAX_LIFETIME (30 days)
- idle(t) < MAX_IDLE (14 days)

This invariant MUST hold at all times, including during deployment
rollouts and clock-skew windows.
:::

Violations of this invariant are logged as critical alerts and trigger
automatic token revocation via the
[session reaper](spec:src:packages/auth/reaper.ts:10-30).
