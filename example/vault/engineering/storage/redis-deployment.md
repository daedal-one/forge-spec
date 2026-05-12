---
type: Note
tags: [infra, redis]
specs:
  - ADR:auth/0001-session-storage
---

# Redis deployment

Operational notes for the Redis instance backing session storage.

## Topology

Single primary, two read replicas. Replicas are not used for session
reads (consistency); they exist for failover.

## Capacity planning

At peak we hold ~5M active sessions. Each entry is ~400 bytes after
encoding, giving a working set of ~2GB.
