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

Rehearse the warrant-bound shadow lane and prepare zk settlement artifacts:

```bash
loom shadow run \
  --backend wasmtime \
  --root "$LOOM_ROOT" \
  --kernel-path "$MERIDIAN_KERNEL_PATH" \
  --agent-id agent_atlas \
  --org-id "$MERIDIAN_ORG_ID" \
  --action-type research \
  --resource system_info \
  --module builtin:system.info \
  --warrant-file ./shadow-warrant.json \
  --format human

loom job settle \
  --zk \
  --root "$LOOM_ROOT" \
  --kernel-path "$MERIDIAN_KERNEL_PATH" \
  --actual-cost-usd 0.05 \
  --format human
```

Run semantic gRPC action transport (same governance/proof contract, different transport):

```bash
loom shadow run \
  --backend grpc_action \
  --root "$LOOM_ROOT" \
  --kernel-path "$MERIDIAN_KERNEL_PATH" \
  --agent-id agent_atlas \
  --org-id "$MERIDIAN_ORG_ID" \
  --action-type shadow_grpc_action \
  --resource external_grpc_action \
  --warrant-file ./shadow-warrant.json \
  --url 127.0.0.1:50051 \
  --grpc-service meridian.runtime.v1.ActionService \
  --grpc-method SubmitAction \
  --grpc-action-kind research.deliver \
  --grpc-action-objective "deliver governed runtime diff" \
  --grpc-context-json '{"lane":"trust_ops"}' \
  --grpc-constraints-json '{"max_latency_ms":12000}' \
  --grpc-memory-json '["mem://pattern/trust-ops-summary-v3"]' \
  --grpc-plaintext \
  --grpc-timeout-seconds 10 \
  --grpc-allow-unknown-fields \
  --format human
```

`grpc_action` notes:

- Requires `grpcurl` in `PATH`, or set `LOOM_SHADOW_GRPCURL_BIN=/path/to/grpcurl`.
- Optional transport flags:
  - `--grpc-authority`
  - repeated `--grpc-import-path`
  - repeated `--grpc-proto`
  - repeated `--grpc-protoset`
  - `--grpc-timeout-seconds` (1..120)
  - `--grpc-allow-unknown-fields`
  - `--grpc-plaintext` / `--grpc-tls`
- RPC formatting is strict: service + method must resolve to exactly one
  `<Service>/<Method>` segment pair.
- `shadow report` now prints typed gRPC diagnostics for the latest
  `grpc_action` run (target/rpc/transport/timeout/proto-protoset counts).
- The same diagnostics are persisted as typed artifacts at
  `artifacts/shadow/grpc_action/latest.json` (+ `stream.jsonl`) under the
  runtime root.
- `parity report` also renders this typed diagnostics block, so operator review
  can stay in one place.

The settlement slice is bounded on purpose:

- `shadow run` requires a verified warrant file
- `job settle --zk` binds to the PoGE witness digest from the latest shadow run
- Court and Treasury are checked before settlement is marked prepared
- chain finality is not claimed until a chain adapter confirms submission

## Common failures

- `401 unauthorized`: token mismatch between service start and HTTP client
- `400 bad_request`: malformed HTTP request or invalid JSON body
- `415 unsupported_media_type`: submit/import request was not JSON
- `service log not found`: service has not been started yet
- `runtime service already appears to be running`: stale or active lock/PID detected
- `kernel path is required`: config or command is missing a kernel binding for that path
