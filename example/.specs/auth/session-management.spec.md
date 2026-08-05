---
id: REQ:auth/session-management
type: requirement
status: accepted
level: MUST
summary: >
  The system manages authenticated sessions with bounded lifetimes,
  idle expiration, and credential-rotation revocation.
owners: [carlo]
categorized_under: [TOPIC:topics/auth]
---

# Session management

## Overview

The authentication subsystem maintains server-side session state for every
authenticated user. Sessions are the sole mechanism for carrying identity
across requests after initial authentication.

:::{requirement id="session-management" level="MUST"}
The system MUST manage sessions according to:

- {#c-lifetime} bounded maximum lifetime from issuance
- {#c-idle} expiration after a period of inactivity
- {#c-rotation} revocation when the user rotates credentials
:::

## Rationale

Unbounded sessions are a security risk. The three clauses above cover the
principal attack surfaces: stale tokens, abandoned sessions, and compromised
credentials.

See also [session API contract](spec:IFC:auth/session-api) and
[no-stale-tokens invariant](spec:INV:auth/no-stale-tokens).
