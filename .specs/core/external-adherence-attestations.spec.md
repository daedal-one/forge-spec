---
id: TASK:core/external-adherence-attestations
type: task
status: accepted
summary: Replace tracked implementation checkpoint fields with immutable external adherence attestations.
owners: [carlo]
progress: done
addresses:
  - REQ:core/intellect-provider#c-attestation
  - REQ:core/intellect-provider#c-protocol
  - REQ:core/intellect-provider#c-state
  - REQ:core/intellect-provider#c-surfaces
  - REQ:core/intellect-provider#c-standalone
  - REQ:core/intellect-provider#c-mutation
  - REQ:core/intellect-provider#c-selection
  - REQ:core/intellect-provider#c-portability
  - REQ:core/intellect-provider#c-migration
labels: [adherence, attestation, protocol, git-notes, migration, provenance]
assignee: carlo
eta:
blocked_by: []
---

# External adherence attestations

## Plan

Define canonical intent digests and a versioned attestation protocol, make
verification and revocation append external records without touching tracked
workspace bytes, derive every adherence surface from those records, and migrate
legacy `implemented` fields without manufacturing new verification evidence.

## Acceptance

Verification leaves `HEAD`, the index, and the working tree unchanged while a
subsequent status request resolves the recorded attestation. Tests cover
canonical digest stability, Git-note publication, append-only revocation,
missing or conflicting stores, batch verification, legacy migration, provider
failure, rendering, linting, and exact protocol validation.
