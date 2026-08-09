---
id: PROJECT:example
type: project
status: accepted
summary: >
  An example service whose specifications demonstrate secure session management
  from project intent through requirements, decisions, interfaces, and code.
owners: [carlo]
---

# Example session service

## Purpose

Demonstrate how forge-spec turns a project's intent into navigable,
cross-referenced specifications and connects those specifications to source.

## Scope

The example covers first-party authenticated session creation, validation,
expiration, revocation, storage, observability, and their implementation
boundaries.

## Non-goals

It is not a production authentication service and does not specify OAuth,
OIDC, user registration, or credential recovery.

## Principles

Security properties remain explicit, higher-level requirements decompose by
addressable clause, and implementation references are evidence rather than a
substitute for intent.
