---
id: PROJECT:forge-spec
type: project
status: accepted
summary: >
  A repository-native format and toolchain for refining project intent into
  navigable, versioned, mechanically validated specifications.
owners: [carlo]
---

# forge-spec

## Purpose

Give humans and coding agents one authoritative, repository-native account of
why a project exists and how that intent is refined into requirements,
decisions, interfaces, tasks, and implementation evidence.

## Scope

forge-spec defines a structured Markdown format, graph semantics, validation,
human and agent rendering, Git-derived history, deterministic migrations,
source-symbol resolution, and native exploration through CLI and editor tools.

## Non-goals

forge-spec does not prove that requirements are correct, replace behavioral
tests, duplicate generated API documentation, or turn an iterative refinement
process into a fixed waterfall plan.

## Principles

Intent remains versioned beside code. Refinement is explicit and clause-aware.
Navigation never weakens semantic distinctions. Tooling shares one parser and
model so humans, agents, CLIs, and editors see the same project structure.
