---
type: Meeting
date: 2026-02-10
attendees: [carlo, elena]
specs:
  - REQ:auth/session-expiry
  - REQ:auth/session-management
  - ADR:auth/0001-session-storage
  - INV:auth/no-stale-tokens
---

# Auth review — Feb 2026

## Agenda

- Review [session expiry policy](spec:REQ:auth/session-expiry)
- Confirm [storage decision](spec:ADR:auth/0001-session-storage) holds
- Discuss observability gaps

## Decisions

- Keep the 30-day cap for now; revisit in Q3
- Add per-tenant override capability (new requirement to draft)

## Action items

- [ ] Draft tenant-override REQ
- [ ] Update [session API interface](spec:IFC:auth/session-api) with revoke endpoint
- [ ] Schedule load test for Redis at 10M sessions
