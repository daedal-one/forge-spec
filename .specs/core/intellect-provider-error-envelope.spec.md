---
id: TASK:core/intellect-provider-error-envelope
type: task
status: accepted
summary: Decode and surface intellect-provider error envelopes instead of misreporting them as malformed success responses.
owners: [carlo]
progress: done
addresses:
  - REQ:core/intellect-provider#c-protocol
  - REQ:core/intellect-provider#c-lifecycle
  - REQ:core/intellect-provider#c-standalone
labels: [adherence, provider, protocol, diagnostics]
assignee: carlo
eta:
blocked_by: []
---

# Intellect-provider error envelopes

## Plan

Decode the provider's versioned success and error envelopes before selecting
the operation-specific success body. Validate the error schema and response
kind, then surface its message as the actionable command failure.

## Acceptance

Valid adherence, health, and shutdown responses retain strict unknown-field
checking. A valid provider error envelope fails closed with the provider's
message, rather than an `unknown field message` success-decoding error. Live
attestation against Forge Intellect exposes the underlying refusal.
