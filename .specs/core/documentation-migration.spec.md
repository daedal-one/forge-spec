---
id: TASK:core/documentation-migration
type: task
status: accepted
summary: 'Publish the additive v0.4 documentation vocabulary, migration guidance, CLI completions, and adoption documentation.'
owners: [carlo]
progress: done
addresses: [REQ:core/documentation-integration#c-migration]
assignee:
eta:
blocked_by: []
---

# Documentation migration

## Plan

Published the additive [v0.3 to v0.4 migration](spec:src:spec-cli/migrations/forge-spec-v0.3.0-to-v0.4.0.yaml), executable v0.5 metadata, typed collection enrollment, completions, specification reference, format memo, agent guidance, editor documentation, and onboarding examples.

## Acceptance

The adjacent migration is deterministic and idempotent, preserves existing configuration, and creates no collection implicitly. This repository migrated to v0.4 and explicitly enrolled two bounded collections; all enrolled Markdown passes the current linter.
