<p align="center">
  <img src="docs/assets/meridian_loom_lockup.svg" alt="Meridian Loom" width="720">
</p>

<p align="center">
  Local-first runtime surface for Meridian operators.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/phase-local--first%20runtime-0c1117?style=flat-square" alt="Local-first runtime">
  <img src="https://img.shields.io/badge/boundary-production--oriented%20local%20only-8b0000?style=flat-square" alt="Production-oriented local only">
  <img src="https://img.shields.io/github/actions/workflow/status/mapleleaflatte03/meridian-loom/rust.yml?branch=main&style=flat-square" alt="Rust CI">
  <img src="https://img.shields.io/badge/license-MIT-1f6feb?style=flat-square" alt="MIT license">
</p>

<p align="center">
  <a href="docs/INSTALL.md">Install</a> ·
  <a href="docs/RUN_LOCAL.md">Run Local</a> ·
  <a href="docs/SERVICE.md">Service</a> ·
  <a href="docs/CONFIG.md">Config</a> ·
  <a href="docs/OPERATIONS.md">Operations</a> ·
  <a href="docs/RELEASE.md">Release</a> ·
  <a href="docs/PRODUCT_TRUTH.md">Product Truth</a>
</p>

# Meridian Loom

Meridian Loom is the local-first runtime surface for Meridian. It is meant to
be installable, inspectable, and operable by a real end-user or operator on a
single Linux host.

What is real today:

- a real `loom` binary
- a real local service lifecycle
- a local HTTP control plane with token auth
- local queue, scheduler, job ledger, audit, and parity artifacts
- Docker, tarball, and source install paths
- operator docs for install, config, run, logs, and release

What is not claimed today:

- hosted runtime replacement
- live OpenClaw retirement
- transport cutover for the live host
- full hosted parity with OpenClaw

OpenClaw still runs the live host. Meridian remains the governance kernel above
runtimes.

## Install order

Prefer these paths in order:

1. Docker for the fastest isolated local runtime
2. prebuilt tarball for end-user/operator machines
3. source build when extending the runtime

See [docs/INSTALL.md](docs/INSTALL.md).

## Quick start

### Docker

```bash
docker build -t meridian-loom:local .
export MERIDIAN_KERNEL_PATH=/tmp/meridian-kernel
export LOOM_SERVICE_TOKEN=loom-local-token
docker run --rm \
  -p 127.0.0.1:18910:18910 \
  -e LOOM_ROOT=/var/lib/loom/runtime/default \
  -e LOOM_SERVICE_TOKEN="$LOOM_SERVICE_TOKEN" \
  -e LOOM_ORG_ID=local_foundry \
  -e MERIDIAN_RUNTIME_AUDIT_FILE=/var/lib/loom/runtime/default/artifacts/audit/loom_runtime_events.jsonl \
  -v "$PWD/runtime:/var/lib/loom" \
  -v "$MERIDIAN_KERNEL_PATH:/kernel:ro" \
  --entrypoint /bin/sh \
  meridian-loom:local \
  -lc 'loom init --mode embedded --root "$LOOM_ROOT" --kernel-path /kernel --org-id "$LOOM_ORG_ID" && exec loom start --foreground --root "$LOOM_ROOT" --kernel-path /kernel --http-address 0.0.0.0:18910 --service-token "$LOOM_SERVICE_TOKEN"'
```

### Local binary

```bash
cargo build --release --workspace --locked
export LOOM_ROOT="$HOME/.local/share/meridian-loom/runtime/default"
export LOOM_SERVICE_TOKEN=loom-local-token
export MERIDIAN_KERNEL_PATH=/tmp/meridian-kernel

target/release/loom init \
  --mode embedded \
  --root "$LOOM_ROOT" \
  --kernel-path "$MERIDIAN_KERNEL_PATH" \
  --org-id local_foundry

target/release/loom doctor --root "$LOOM_ROOT" --format human
target/release/loom start \
  --root "$LOOM_ROOT" \
  --kernel-path "$MERIDIAN_KERNEL_PATH" \
  --http-address 127.0.0.1:18910 \
  --service-token "$LOOM_SERVICE_TOKEN"
```

Submit a request:

```bash
curl -sS \
  -H "Authorization: Bearer ${LOOM_SERVICE_TOKEN}" \
  -H "Content-Type: application/json" \
  -X POST \
  --data '{
    "request_id":"demo-submit",
    "agent_id":"agent_allow",
    "org_id":"local_foundry",
    "action_type":"research",
    "resource":"web_search",
    "estimated_cost_usd":0.05,
    "kernel_path":"/tmp/meridian-kernel"
  }' \
  http://127.0.0.1:18910/submit
```

Inspect and stop:

```bash
target/release/loom status --root "$LOOM_ROOT"
target/release/loom logs --root "$LOOM_ROOT" --lines 50
target/release/loom stop --root "$LOOM_ROOT"
```

## Runtime layout

Each runtime root is self-contained:

- `<root>/loom.toml`
- `<root>/state/`
- `<root>/run/service/`
- `<root>/run/ingress/`
- `<root>/logs/`
- `<root>/capabilities/`
- `<root>/artifacts/audit/`
- `<root>/artifacts/parity/`
- `<root>/artifacts/runtime/`
- `<root>/artifacts/shadow/`

This is the production-oriented local layout. Legacy `.loom/` roots are still
read for backward compatibility when an older workspace is present.

## Command surface

First-class operator commands:

- `loom version`
- `loom init`
- `loom doctor`
- `loom health`
- `loom status`
- `loom start`
- `loom stop`
- `loom restart`
- `loom logs`
- `loom capability list|show|gap show|scaffold|forge|import-workspace-skill|verify|promote|shim`
- `loom service start|status|submit|import-commitments|stop`
- `loom job list|inspect`
- `loom parity report`
- `loom shadow report`

Run `loom help` for the full surface.

## Acceptance path

The repo includes an end-to-end local acceptance harness:

```bash
./scripts/acceptance_local_service.sh \
  --root "$HOME/.local/share/meridian-loom/runtime/acceptance" \
  --kernel-path /tmp/meridian-kernel
```

That harness proves:

- init into the production-oriented layout
- start as a local service
- status and health checks
- tokenized HTTP submit
- job processing and inspection
- logs and artifacts at stable paths
- restart and idempotent stop

The repo also includes container verification without depending on a compose
plugin:

```bash
./scripts/acceptance_container_service.sh --kernel-path /tmp/meridian-kernel
./scripts/verify_release_local.sh --kernel-path /tmp/meridian-kernel
```

For capability-backed execution on a fixture kernel:

```bash
./scripts/rehearse_capability_runtime.sh > examples/capability-runtime-output.txt
```

For the capability-backed service path end-to-end:

```bash
./scripts/acceptance_capability_service.sh
```

For imported clawfamily workspace skills on a fixture kernel:

```bash
./scripts/rehearse_claw_skill_import.sh > examples/claw-skill-import-output.txt
```

For the full imported-skill lifecycle inside Loom:

```bash
./scripts/rehearse_claw_skill_lifecycle.sh > examples/claw-skill-lifecycle-output.txt
```

For the imported clawfamily skill through Loom service submit:

```bash
./scripts/rehearse_claw_skill_service.sh > examples/claw-skill-service-output.txt
```

For multi-shape clawfamily compatibility through Loom service submit:

```bash
./scripts/rehearse_claw_skill_multi_import.sh > examples/claw-skill-multi-import-output.txt
```

For a Loom-forged candidate capability that verifies and promotes itself through
the same runtime path:

```bash
./scripts/rehearse_capability_forge_lifecycle.sh > examples/capability-forge-lifecycle-output.txt
```

For a single local server-path rehearsal driven entirely by Loom with an
imported clawfamily skill owned by Loom's service boundary:

```bash
./scripts/rehearse_server_replacement.sh > examples/server-replacement-output.txt
```

When the kernel bundle is mounted read-only in Docker, set
`MERIDIAN_RUNTIME_AUDIT_FILE` into the writable Loom runtime root as shown
above. That keeps runtime audit emission usable without pretending the mounted
kernel tree itself is writable.

## Packaging

Release and install helpers:

- `./scripts/package_release.sh`
- `./scripts/release_local.sh`
- `./scripts/install_local.sh`
- `deploy/systemd/loom.service`
- `deploy/systemd/loom-user.service`

See [docs/RELEASE.md](docs/RELEASE.md).

## Product truth

Loom is now production-oriented in local form. It is still not the hosted
runtime that retires OpenClaw. That distinction is intentional and enforced in
the docs and operator output.

See [docs/PRODUCT_TRUTH.md](docs/PRODUCT_TRUTH.md).
