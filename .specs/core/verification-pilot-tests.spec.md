---
id: TASK:core/verification-pilot-tests
type: task
status: accepted
summary: Make canonical projection path safety and ordering directly testable by the Intellect verification pilot.
owners: [carlo]
progress: done
addresses: ['REQ:core/canonical-projection#c-overlay', 'REQ:core/canonical-projection#c-canonical-state', 'REQ:core/canonical-projection#c-read-only']
---

# Projection verification controls

Add executable controls for nested path traversal, unchanged saved bytes after a
rejected overlay, and canonical equality across different file creation orders.
Retain these in the ordinary projection suite so a compiling behavioral mutant
cannot pass by using an unrelated test filter. Intellect owns the assessment and
external attestation; no verification checkpoint belongs in durable Spec bytes.
