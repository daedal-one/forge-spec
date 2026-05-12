---
type: Note
tags: [auth, security]
specs:
  - REQ:auth/session-expiry
  - INV:auth/no-stale-tokens
  - IFC:auth/session-api
---

# Auth threat model

Working notes on the threat surface for the session subsystem.

## Token theft

A stolen token grants the attacker access until expiry. The
30-day cap from [session expiry](spec:REQ:auth/session-expiry)
bounds the blast radius, but credential rotation
([the design note](spec:kb:engineering/auth/session-tokens.md#credential-rotation))
shortens it on detected compromise.

## Stale tokens

The [no-stale-tokens invariant](spec:INV:auth/no-stale-tokens)
is enforced at the API boundary; see
[`session.ts`](spec:src:packages/auth/session.ts:42-78) for
the check.
