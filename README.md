<p align="center">
  <img src="docs/assets/meridian_loom_lockup.svg" alt="Meridian Loom" width="720">
</p>

<p align="center">
  <img src="docs/assets/loom_runtime_panels.svg" alt="Meridian Loom runtime panels" width="960">
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

Meridian Loom is the operator-facing local runtime boundary for Meridian. It is installable, inspectable, and runnable on a single Linux host.

The current repo truth is intentionally narrow:

- a real local service lifecycle exists
- foreground and background service modes exist
- the local HTTP control plane is tokenized
- queue, job ledger, audit, parity, and shadow artifacts are real
- Docker, tarball, and source install paths exist
- rehearsal scripts are split between operational tests and migration/back-compat tooling

What is not claimed:

- hosted runtime replacement
- live transport cutover
- distributed queue orchestration
- retired OpenClaw dependency in the live host

Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the compact architecture and truth boundary.

## Quick links

- Install: [docs/INSTALL.md](docs/INSTALL.md)
- Run Local: [docs/RUN_LOCAL.md](docs/RUN_LOCAL.md)
- Service: [docs/SERVICE.md](docs/SERVICE.md)
- Config: [docs/CONFIG.md](docs/CONFIG.md)
- Operations: [docs/OPERATIONS.md](docs/OPERATIONS.md)
- Architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Release: [docs/RELEASE.md](docs/RELEASE.md)

## Operational surface

- `loom init`, `loom doctor`, `loom health`, `loom status`
- `loom start`, `loom stop`, `loom restart`, `loom logs`
- `loom queue inspect|consume|ack|run-once|run-until-empty|status`
- `loom job list|inspect`
- `loom parity report`
- `loom shadow report`
- `loom capability list|show|gap show|scaffold|forge|import-workspace-skill|verify|promote|shim`
- `loom service start|status|submit|import-commitments|stop`

## Rehearsals

- Operational rehearsals live in `scripts/tests/`
- Migration and backward-compatibility rehearsals live in `scripts/migration_tools/`
- Generated `examples/*-output.txt` transcripts are not checked in

## Truth boundary

- local service, queue, audit, parity, and operator surfaces are real
- queue ack, run-once, run-until-empty, and status are real local queue operations
- the repo is not a hosted replacement claim
- legacy OpenClaw-named compatibility surfaces remain only where they still serve import or migration compatibility
