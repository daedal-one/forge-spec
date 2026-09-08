# One semantic model, several views

Forge Spec v0.7 keeps requirements, architectural subjects, scenario behavior
and transient work distinct. LikeC4 can present this model using domain kinds
or a C4 profile. Generated diagrams carry canonical references; their local
renderer IDs are never specification identities.

A durable specification may declare a `model` facet. A subject describes a
thing in the domain; its owning specification remains a document about intent.
Subjects and interactions use local anchors, resolved as `OWNER#id`. Anchor
names are shared with typed blocks, clauses and scenario steps, so collisions
are errors. Kinds are extensible, nonempty strings; no C4 vocabulary is required.

```yaml
model:
  subjects:
    - id: customer
      kind: actor
      title: Customer
    - id: product
      kind: product
      title: Product
    - id: api
      kind: service
      title: API
      part_of: ['PROJECT:example#product']
  interactions:
    - id: request
      kind: request
      title: Submit request
      from: 'PROJECT:example#customer'
      to: 'PROJECT:example#api'
      governed_by: ['REQ:api/contract#c-response']
```

Other durable documents connect to these subjects with
`model: {about: ['PROJECT:example#api']}`. Composition is an acyclic graph,
independent of refinement and categorization. Multiple composition parents are
allowed; a renderer must retain their meaning rather than choosing one as an
implicit normative hierarchy. Interaction endpoints and `about` targets resolve
to declared subjects; governing references resolve to durable specs or anchors.
Legacy `provided_by`, `consumed_by` and `applies_to` strings stay unregistered
participant declarations until explicitly modeled. TASK cannot own a model.

Only SCN accepts `flow`. Participants are unique canonical subject references;
steps retain authored order. Every step and alternative branch has a unique
local anchor. `interaction` steps may refer to a declared interaction and must
then use its exact endpoints; `refs` addresses durable specs and anchors.

```yaml
flow:
  participants: ['PROJECT:example#customer', 'PROJECT:example#api']
  steps:
    - kind: interaction
      id: submit
      from: 'PROJECT:example#customer'
      to: 'PROJECT:example#api'
      title: Submit request
      interaction: 'PROJECT:example#request'
      refs: ['REQ:api/contract#c-response']
    - kind: alternatives
      id: response
      branches:
        - id: success
          title: Success
          steps:
            - kind: interaction
              id: reply
              from: 'PROJECT:example#api'
              to: 'PROJECT:example#customer'
              title: Reply
        - id: retry
          title: Retry
          steps:
            - kind: parallel
              id: recovery
              steps:
                - kind: interaction
                  id: resubmit
                  from: 'PROJECT:example#customer'
                  to: 'PROJECT:example#api'
                  title: Resubmit request
                  interaction: 'PROJECT:example#request'
```

`parallel.steps` and every branch are nonempty; alternatives require at least
two named branches. Nesting is bounded to 32 levels. Unsupported kinds and
unknown fields fail parsing rather than disappear. Steps describe intended
behavior; they carry no execution state. Legacy prose-only SCN remains valid
and migration never infers steps from it.

Presentation belongs in `.specs/_views.toml`, outside normative spec digests:

```toml
schema = "forge-spec-views/v1"

[[views]]
id = "architecture"
title = "Architecture"
mode = "architecture"
focus = "PROJECT:example#api"
depth = 2
include = []
exclude = []
profile = "c4"
```

Modes are `map`, `architecture`, `scenarios`, and `work`. Focus, include and
exclude are exact canonical spec or anchor references. Depth is at most 32;
profile is `generic` or `c4` when provided. Missing selectors are diagnostics.
Presentation changes alter the canonical state and its cache digest but do not
change any durable specification's normative digest or stale its attestations.

Use typed batch operations through `spec change batch --from changes.json`:
`model.replace`, `model.clear`, `scenario.flow.replace`, `scenario.flow.clear`,
`view.replace`, and `view.remove`. Replace operations accept the typed value;
model and flow operations also name `spec`. `view.remove` names `id`.
`spec.rename` updates model, scenario and saved-view references atomically.
The parser, lint, projections, human/XML rendering, LSP anchor completion,
definitions and document symbols share these declarations.

`spec inspect model --json` emits `forge-spec-model/v1`:

- `baseline`, `projector_version`, and versioned `state` describe compatibility.
- `state` uses `forge-spec-state-v5`; `model` includes canonical subjects,
  interactions and about edges; `scenarios` preserves local ordered step IDs.
  The canonical step reference is `scenario.id#step.id`.
- `digest` hashes the full canonical state for presentation caching.
- `intent_digests` maps durable spec IDs to canonical normative hashes. TASK
  never appears in this map.
- `revisions` maps durable and work-item IDs to `path`, per-file `revision`, and
  the selected snapshot HEAD `commit`. Without Git, both values are explicitly
  null. This commit denotes the snapshot, not the last commit touching a file.
- `capabilities` names supported semantic surfaces and read-only compatibility.

The export does not execute repository hooks, source providers, layout code or
LikeC4 configuration. It supports v0.6 read-only input through shared `projection::project_compatible`; it emits
current v5 state and does not reproduce historical v4 artifact hashes. Other
writers and strict `projection::project` use v0.7 after `spec migrate plan --target agent` and
`spec migrate apply`. Invalid input remains visible as `valid: false` plus
sorted diagnostics. Consumers must inspect validity and schema compatibility.

`forge-spec-delta-v5` retains prior collections and adds nullable `model`,
`scenarios`, and `views` changes, each containing `before` and `after` values.
Model/flow declarations also participate in changed durable specifications.
