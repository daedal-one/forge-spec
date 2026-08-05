---
id: ADR:auth/0001-session-storage
type: adr
status: accepted
summary: >
  Sessions are stored in Redis with TTL-based expiration rather than
  in the primary database.
owners: [carlo]
decision_date: "2026-01-15"
decided_by: [carlo, elena]
---

# ADR 0001: Session storage engine

## Context

Session lookups happen on every authenticated request. The storage
backend must support high read throughput and automatic expiration.

## Decision

:::{requirement id="redis-storage" level="MUST"}
Session state MUST be stored in Redis. Each session key carries a TTL
equal to the maximum session lifetime (30 days).
:::

## Consequences

- Redis becomes an infrastructure dependency for the auth service.
- Session data is ephemeral; a Redis restart causes mass logout.
- No need for a scheduled reaper job for lifetime expiration (TTL
  handles it), though idle-expiration still requires active checks.

:::{example id="key-format"}
Session keys follow the pattern `session:{user_id}:{token_hash}`.

```
SET session:u42:abc123def456 '{"issued":"2026-01-20T10:00:00Z","last_seen":"2026-01-20T12:00:00Z"}' EX 2592000
```
:::
