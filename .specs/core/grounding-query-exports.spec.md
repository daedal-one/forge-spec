---
id: TASK:core/grounding-query-exports
type: task
status: accepted
summary: Expose deterministic native clause coverage and change impact as versioned JSON for implementation-grounding consumers.
owners: [carlo]
progress: done
addresses: [REQ:core/change-impact, 'REQ:core/canonical-projection#c-read-only']
---

# Grounding query exports

Add opt-in JSON output to the existing coverage and impact commands. Consumers
must use the native analysis, preserve exact clause anchors and impact paths,
and distinguish structural refinement coverage from implementation verification.
TASK addressing remains separate from durable coverage and impact closure.
Validate JSON parity with the native algorithms, deterministic output, missing
subjects, and read-only behavior before integrating the Hub.

Validation: 154 native tests passed, including JSON boundaries, determinism,
unknown subjects and unchanged source bytes. Hub adapter integration tests also
exercise the real installed commands. The stored specification format remains v0.7.
