# Operations

This is the compact runbook for a local operator.

## Environment checks

Doctor:

```bash
loom doctor --root "$LOOM_ROOT" --format human
```

Use `doctor` for:

- config existence
- path layout
- worker paths
- kernel binding
- permissions and obvious missing files

Health:

```bash
loom health --root "$LOOM_ROOT" --format human
curl -sS -H "Authorization: Bearer ${LOOM_SERVICE_TOKEN}" http://127.0.0.1:18910/health
```

Use `health` for:

- workspace readiness
- service running/degraded state

Status:

```bash
loom status --root "$LOOM_ROOT"
curl -sS -H "Authorization: Bearer ${LOOM_SERVICE_TOKEN}" http://127.0.0.1:18910/status
curl -sS -H "Authorization: Bearer ${LOOM_SERVICE_TOKEN}" http://127.0.0.1:18910/metrics
```

Use `status` for:

- service process state
- transport in use
- queue depth
- last request and last job
- runtime paths

## Logs

```bash
loom logs --root "$LOOM_ROOT" --lines 100
loom logs --root "$LOOM_ROOT" --follow
```

Primary files:

- `<root>/logs/service.log`
- `<root>/logs/service_events.jsonl`

Retention knobs:

- `log_max_bytes`
- `log_max_files`

Service logs never print the raw HTTP bearer token. Config/status surfaces only
show the token environment variable name.

## Jobs and artifacts

Inspect jobs:

```bash
loom job list --root "$LOOM_ROOT" --format human
loom job inspect --root "$LOOM_ROOT" --job-id <job_id> --format human
curl -sS -H "Authorization: Bearer ${LOOM_SERVICE_TOKEN}" http://127.0.0.1:18910/jobs/<job_id>
```

Inspect proof surfaces:

```bash
loom parity report --root "$LOOM_ROOT"
loom shadow report --root "$LOOM_ROOT"
```

## Common failures

- `401 unauthorized`: token mismatch between service start and HTTP client
- `400 bad_request`: malformed HTTP request or invalid JSON body
- `415 unsupported_media_type`: submit/import request was not JSON
- `service log not found`: service has not been started yet
- `runtime service already appears to be running`: stale or active lock/PID detected
- `kernel path is required`: config or command is missing a kernel binding for that path
