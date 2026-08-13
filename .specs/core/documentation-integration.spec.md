---
id: REQ:core/documentation-integration
type: requirement
status: accepted
level: MUST
summary: 'Configured repository Markdown becomes indexed, addressable project knowledge with durable heading references and backlinks while remaining semantically distinct from specifications.'
owners: [carlo]
refines: []
categorized_under: []
---

# Documentation integration

## Context

Projects already contain durable knowledge in README files, guides, runbooks, and architecture notes. Forge-spec must connect that knowledge to intent and implementation without requiring ordinary Markdown to adopt specification metadata or semantics.

:::{requirement id="documentation-integration" level="MUST"}
- {#c-collections} Documentation discovery MUST be opt-in through explicitly named, repository-relative collections with include and exclude patterns; arbitrary Markdown MUST NOT be enrolled implicitly.
- {#c-references} Forge-spec MUST address an enrolled document by repository-relative path and MAY address a durable hierarchical Markdown heading using percent-encoded segments, as defined by the public [reference model](spec:doc:specification.md#heading=Specs%20Format%20v0.5%20%E2%80%94%20Specification/5.%20References).
- {#c-index} The workspace index MUST expose collection placement, title, summary, headings, outgoing links, and incoming backlinks for enrolled Markdown, including ordinary relative Markdown links.
- {#c-boundary} Documentation relationships MUST remain informational evidence and MUST NOT imply refinement, categorization, requirement coverage, or specification authority.
- {#c-tooling} CLI, canonical projection, impact analysis, human and agent rendering, the language server, and the native editor workbench MUST share one documentation model and resolver.
- {#c-migration} Adoption MUST be additive: migration MAY introduce the configuration vocabulary but MUST NOT enroll documentation without an explicit collection declaration.
:::
