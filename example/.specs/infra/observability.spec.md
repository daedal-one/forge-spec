---
id: REQ:infra/observability
type: requirement
status: accepted
version: 1.0.0
level: SHOULD
summary: >
  All services emit structured logs, metrics, and traces in a
  consistent format.
owners: [carlo]
kind: non-functional
---

# Observability

:::{requirement id="structured-logging" level="SHOULD"}
Services SHOULD emit structured JSON logs to stdout with at minimum:

- `timestamp` (ISO 8601)
- `level` (debug, info, warn, error)
- `service` (service identifier)
- `trace_id` (distributed trace correlation)
- `message`
:::

:::{requirement id="health-endpoint" level="MUST"}
Every service MUST expose a `GET /healthz` endpoint returning
`200 OK` when the service is ready to accept traffic.
:::
