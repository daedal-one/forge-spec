---
id: GLO:terms/auth
type: glossary
status: accepted
summary: Authentication and session terminology.
owners: [carlo]
---

# Authentication glossary

:::{glossary-entry id="session-token"}
**Session token** -- An opaque, cryptographically random string issued
by the auth service upon successful authentication. It serves as the
bearer credential for subsequent API requests.
:::

:::{glossary-entry id="idle-interval"}
**Idle interval** -- The elapsed wall-clock time since the last
authenticated request associated with a given session token.
:::

:::{glossary-entry id="credential-rotation"}
**Credential rotation** -- The act of a user changing their password
or other authentication secret, which invalidates all previously
issued session tokens.
:::

:::{glossary-entry id="idempotency-key"}
**Idempotency key** -- A client-supplied unique identifier attached
to a mutating request to ensure that retries do not produce
duplicate side effects.
:::
