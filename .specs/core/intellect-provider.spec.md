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
- {#c-attestation} Every specification MAY declare one full Git object ID in
  `implemented`, meaning that its complete normative content was last verified
  as implemented at that checkpoint. The field is authoritative attestation,
  not derived current-state evidence.
- {#c-config} Project configuration MUST select one named intellect provider,
  defaulting to `forge-intellect`; v0.5 recognizes no other implementation.
- {#c-protocol} Forge-spec MUST exchange a versioned, deterministic adherence
  request and response with the provider, keyed by specification ID and one
  exact workspace state.
- {#c-lifecycle} Adherence-aware commands MUST start the configured provider,
  complete a health handshake, request one coherent state, and terminate the
  provider cleanly. Provider absence, timeout, malformed output, or incomplete
  evidence MUST remain explicit and MUST NOT be reported as current adherence.
- {#c-state} Derived adherence MUST distinguish `unverified`, `current`,
  `stale`, `partial`, `violated`, `unknown`, `unresolved`, and
  `not-applicable`, with provider identity, completeness, exact state identity,
  and reasons retained separately from authored lifecycle and TASK progress.
- {#c-surfaces} Tree, render, interactive exploration, and implementation
  status or verification commands MUST consume the same provider snapshot;
  lint, migration, structural inspection, and unrelated mutations MUST remain
  independently usable without an intellect provider.
- {#c-standalone} Forge-spec MUST have no build, installation, or mandatory
  runtime dependency on an intellect provider. Read-only adherence surfaces
  MUST remain usable with explicit `unknown` state when the provider is absent;
  only recording a new verification checkpoint MUST fail closed.
- {#c-mutation} Setting or clearing an implementation checkpoint and changing
  the configured provider MUST use the shared typed mutation engine.
:::
