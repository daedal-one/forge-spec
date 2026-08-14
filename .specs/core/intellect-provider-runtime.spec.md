---
id: TASK:core/intellect-provider-runtime
type: task
status: accepted
summary: Publish and integrate the v0.5 implementation-attestation and intellect-provider contract.
owners: [carlo]
progress: done
addresses:
  - REQ:core/intellect-provider#c-attestation
  - REQ:core/intellect-provider#c-config
  - REQ:core/intellect-provider#c-protocol
  - REQ:core/intellect-provider#c-lifecycle
  - REQ:core/intellect-provider#c-state
  - REQ:core/intellect-provider#c-surfaces
  - REQ:core/intellect-provider#c-standalone
  - REQ:core/intellect-provider#c-mutation
labels: [attestation, configuration, protocol, lifecycle, evidence, presentation, standalone-operation, authoring]
assignee: carlo
eta:
blocked_by: []
---

# Intellect-provider runtime

## Plan

Add the v0.5 authored metadata and project configuration, publish the adjacent
migration and provider protocol, route adherence-aware CLI surfaces through one
provider client, and implement the default provider in forge-intellect.

## Acceptance

Tests cover configuration defaults and rejection, implementation-checkpoint
mutation, protocol health and state exchange, provider failure without false
current state, lifecycle cleanup, standalone operation without the provider,
tree/render integration, migration idempotency, and an end-to-end
forge-intellect provider exchange.
