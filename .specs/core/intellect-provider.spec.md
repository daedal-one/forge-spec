---
id: REQ:core/intellect-provider
type: requirement
status: accepted
level: MUST
summary: >
  Authored implementation checkpoints remain in specifications while a
  configured intellect provider supplies exact, evidence-backed code adherence
  for the selected workspace state.
owners: [carlo]
refines: []
related:
  - REQ:core/canonical-projection
  - REQ:core/change-impact
  - REQ:core/typed-mutation
---

# Intellect provider and code adherence

:::{requirement id="adherence" level="MUST"}
- {#c-attestation} Every durable specification MAY declare one full Git object ID in
  `implemented`, meaning that its complete normative content was last verified
  as implemented at that checkpoint. The field is authoritative attestation,
  not derived current-state evidence. TASK work items MUST NOT carry
  implementation-adherence checkpoints.
- {#c-config} Project configuration MUST select one named intellect provider,
  defaulting to `forge-intellect`; v0.6 recognizes no other implementation.
- {#c-protocol} Forge-spec MUST exchange a versioned, deterministic adherence
  request and response with the provider, keyed by specification ID and one
  exact workspace state.
- {#c-lifecycle} Adherence-aware commands MUST discover or atomically start one
  healthy worktree-scoped background provider, complete a health handshake,
  and request one coherent state without stopping the shared process. The
  provider MUST self-terminate after a bounded idle interval. Provider absence,
  timeout, malformed output, or incomplete evidence MUST remain explicit and
  MUST NOT be reported as current adherence.
- {#c-state} Derived adherence MUST distinguish `unverified`, `current`,
  `stale`, `partial`, `violated`, `unknown`, `unresolved`, and
  `not-applicable`, with provider identity, completeness, exact state identity,
  and reasons retained separately from authored lifecycle. TASK work items are
  outside the adherence protocol.
- {#c-display} A human tree row MUST render exactly one compact effective state
  as a symbol and name. A non-accepted lifecycle controls first; otherwise
  applicable adherence replaces `accepted`. `not-applicable` falls back to
  `accepted`, and a missing provider result fails closed to `unknown`. Work-item
  views MUST render TASK progress directly without requesting adherence.
- {#c-surfaces} Tree, render, interactive exploration, and implementation
  status or verification commands MUST consume the same provider snapshot;
  lint, migration, structural inspection, and unrelated mutations MUST remain
  independently usable without an intellect provider.
- {#c-standalone} Forge-spec MUST have no build, installation, or mandatory
  runtime dependency on an intellect provider. Read-only adherence surfaces
  MUST remain usable with explicit `unknown` state when the provider is absent;
  only recording a new verification checkpoint MUST fail closed.
- {#c-service} `spec implementation provider start|status|stop` MUST manage an
  authenticated loopback provider in the background. Its endpoint, process,
  and log metadata MUST live outside tracked workspace bytes, be isolated per
  Git worktree, reject mismatched workspace roots, and tolerate stale control
  metadata without reporting the service as healthy.
- {#c-mutation} Setting or clearing an implementation checkpoint and changing
  the configured provider MUST use the shared typed mutation engine.
- {#c-selection} Provider requests and implementation status, verification, and
  checkpoint commands MUST exclude TASK work items.
:::
