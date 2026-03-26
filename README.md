<p align="center">
  <img src="./docs/assets/meridian_loom_lockup.svg" alt="Meridian Loom" width="720">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-1f6feb?style=flat-square" alt="MIT license">
  <img src="https://img.shields.io/github/actions/workflow/status/mapleleaflatte03/meridian-loom/rust.yml?branch=main&style=flat-square" alt="Build passing">
  <img src="https://img.shields.io/badge/version-0.1.0--alpha.1-0c1117?style=flat-square" alt="Version 0.1.0-alpha.1">
</p>

<p align="center">
  <img src="./docs/assets/loom_runtime_panels.svg" alt="Meridian Loom runtime panels" width="960">
</p>

<p align="center">
  Meridian Loom is the local-first runtime surface for Meridian.
</p>

<p align="center">
  <a href="docs/INSTALL.md">Install</a> ·
  <a href="docs/RUN_LOCAL.md">Run Local</a> ·
  <a href="docs/SERVICE.md">Service</a> ·
  <a href="docs/CONFIG.md">Config</a> ·
  <a href="docs/OPERATIONS.md">Operations</a> ·
  <a href="docs/ARCHITECTURE.md">Architecture</a> ·
  <a href="docs/RELEASE.md">Release</a>
</p>

# Meridian Loom

## What It Is

Meridian Loom is the operator-facing local runtime boundary for Meridian. It is installable, inspectable, and runnable on a single Linux host.

## Three-Part Architecture

Meridian Loom is one part of a three-repo runtime stack:

- [meridian-loom](https://github.com/mapleleaflatte03/meridian-loom) provides the operator-facing local runtime surface.
- [meridian-kernel](https://github.com/mapleleaflatte03/meridian-kernel) provides the governance and policy kernel.
- [meridian-intelligence](https://github.com/mapleleaflatte03/meridian-intelligence) provides the intelligence and route layer.

## What Exists Today

- Local service lifecycle with foreground and background modes.
- Tokenized local HTTP control plane.
- Queue, job ledger, audit, parity, and shadow artifacts on disk.
- Docker, tarball, and source install paths.
- Rehearsal scripts split between operational tests and migration/back-compat tooling.

## What Is Not Claimed

- Hosted runtime replacement.
- Live transport cutover.
- Distributed queue orchestration.
- Retired OpenClaw dependency in the live host.

Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the compact architecture and truth boundary.

## Quick Start

```bash
cargo build --release --workspace --locked

export LOOM_ROOT="$HOME/.local/share/meridian-loom/runtime/default"
export MERIDIAN_KERNEL_PATH=/tmp/meridian-kernel
export LOOM_SERVICE_TOKEN=loom-local-token

target/release/loom init \
  --mode embedded \
  --root "$LOOM_ROOT" \
  --kernel-path "$MERIDIAN_KERNEL_PATH" \
  --org-id local_foundry

target/release/loom start \
  --root "$LOOM_ROOT" \
  --kernel-path "$MERIDIAN_KERNEL_PATH" \
  --http-address 127.0.0.1:18910 \
  --service-token "$LOOM_SERVICE_TOKEN"
```

## More

- Install: [docs/INSTALL.md](docs/INSTALL.md)
- Run Local: [docs/RUN_LOCAL.md](docs/RUN_LOCAL.md)
- Service: [docs/SERVICE.md](docs/SERVICE.md)
- Config: [docs/CONFIG.md](docs/CONFIG.md)
- Operations: [docs/OPERATIONS.md](docs/OPERATIONS.md)
- Architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Release: [docs/RELEASE.md](docs/RELEASE.md)

## Operational Surface

- `loom init`, `loom doctor`, `loom health`, `loom status`
- `loom start`, `loom stop`, `loom restart`, `loom logs`
- `loom queue inspect|consume|ack|run-once|run-until-empty|status`
- `loom job list|inspect`
- `loom parity report`
- `loom shadow report`
- `loom capability list|show|gap show|scaffold|forge|import-workspace-skill|verify|promote|shim`
- `loom service start|status|submit|import-commitments|stop`

## Rehearsals

- Operational rehearsals live in `scripts/tests/`.
- Migration and backward-compatibility rehearsals live in `scripts/migration_tools/`.
- Generated `examples/*-output.txt` transcripts are not checked in.

## Truth Boundary

- Local service, queue, audit, parity, and operator surfaces are real.
- Queue ack, run-once, run-until-empty, and status are real local queue operations.
- The repo is not a hosted replacement claim.
- Legacy OpenClaw-named compatibility surfaces remain only where they still serve import or migration compatibility.
