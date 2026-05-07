---
id: IFC:auth/session-api
type: interface
status: accepted
version: 1.0.0
summary: >
  HTTP API surface for session creation, validation, and revocation.
owners: [carlo]
consumed_by: [gateway, web-client]
provided_by: [auth-service]
stability: stable
categorized_under: [TOPIC:topics/auth]
---

# Session API

:::{interface id="session-endpoints" level="MUST"}
The session API MUST expose the following endpoints:

- `POST /sessions` -- create a new session (login)
- `GET  /sessions/:id` -- validate an existing session
- `DELETE /sessions/:id` -- revoke a session (logout)
:::

:::{assumption id="tls-required"}
All session API traffic is assumed to be encrypted via TLS. The API
does not implement its own transport-layer encryption.
:::

:::{non-goal id="oauth-flows"}
OAuth and OIDC flows are handled by a separate identity provider.
This interface covers only first-party session management.
:::
