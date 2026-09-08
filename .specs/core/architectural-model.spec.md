---
id: REQ:core/architectural-model
type: requirement
status: accepted
level: MUST
summary: 'Canonical renderer-independent subjects, interactions, scenario behavior and separate presentation views preserve durable specification authority.'
owners: [carlo]
refines: []
categorized_under: []
---

# Architectural model

## Context

Forge Spec owns intent. LikeC4 and other renderers consume canonical identities without introducing an independent C4 source of truth. Architectural subjects describe domain entities; requirements remain statements about those subjects.


:::{requirement id="architecture" level="MUST"}
- {#c-subjects} Durable specs MUST declare addressable subjects, interactions and about links independently of refinement, categorization and project containment. Composition MUST be acyclic and may have multiple parents.
- {#c-flow} SCN flow MUST preserve declared participants, ordered repeated interactions, alternatives and parallel groups, resolving exact subject endpoints and durable references. TASK MUST NOT own these declarations.
- {#c-views} Saved presentation recipes MUST remain separate from normative intent digests and MUST diagnose unresolved canonical selectors.
- {#c-roundtrip} Parsing, validation, typed atomic mutations, reference-aware rename, rendering, editor navigation and canonical state/delta MUST retain the same declarations.
- {#c-export} The supported machine export MUST include canonical state, cache digest, durable-only intent digests, diagnostics, capabilities and explicit nullable Git revisions without executing repository code.
- {#c-migration} The v0.6 to v0.7 migration MUST preserve authored prose and MUST NOT infer subjects or scenario order.
:::
