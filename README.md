<p align="center">
  <img src="./docs/assets/meridian_loom_lockup.svg" alt="Meridian Loom" width="720">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-1f6feb?style=flat-square" alt="MIT license">
  <img src="https://img.shields.io/github/actions/workflow/status/mapleleaflatte03/meridian-loom/rust.yml?branch=main&style=flat-square" alt="Build passing">
  <img src="https://img.shields.io/badge/version-0.1.6-0c1117?style=flat-square" alt="Version 0.1.6">
</p>

<p align="center">
  <strong>A governed local runtime for bounded autonomous work.</strong>
</p>

<p align="center">
  <a href="docs/INSTALL.md">Install</a> ·
  <a href="docs/RUN_LOCAL.md">Run Local</a> ·
  <a href="docs/SERVICE.md">Service</a> ·
  <a href="docs/CONFIG.md">Config</a> ·
  <a href="docs/OPERATIONS.md">Operations</a> ·
  <a href="docs/ARCHITECTURE.md">Architecture</a>
</p>

# Meridian Loom

Meridian Loom is the primary hands-on product surface for Meridian v0.1.6. It carries the Meridian installer and CLI, provisions into the operator's home directory, and exposes the bounded execution primitives that operators can install, inspect, and run directly.

## Proof of Governed Execution (PoGE)

The platform's cryptographic execution-receipt architecture is defined in the [Meridian PoGE Protocol RFC](docs/MERIDIAN_PoGE_PROTOCOL.md).

- Purpose: bind governed host-calls to verifiable receipts, Merkle roots, and future settlement surfaces.
- Scope: Loom runtime host-call evidence and audit architecture, not a blanket claim that every future chain settlement primitive is already live here.

## 1-Command Install

```bash
curl -fsSL https://raw.githubusercontent.com/mapleleaflatte03/meridian-loom/main/scripts/install.sh | bash
```

The installer now prefers a prebuilt GitHub release for the current host and only falls back to a source build when a compatible release asset is unavailable. It provisions the runtime under `$HOME/.local/share/meridian-loom`, links `loom` into `$HOME/.local/bin`, and seeds the built-in Wasm capability registry.

## Runtime Primitive Story

Meridian Loom is the local runtime layer for the following primitive surfaces:

- Terminal Execution: bounded local argv execution through the native Wasm host-call lane.
- Browser Vision: bounded browser navigation and content capture through the local web/Wasm path.
- Omni-channel Presence: declared gateway surface for delivery and presence routing.
- Persistent Memory: architecture surface for durable memory records and state entries.
- Heartbeat / Background Autonomy: built-in heartbeat scheduling primitive with local receipt logging and truthful scheduler boundaries.
- Dynamic Skill Loading: imported workspace skills, plugin compatibility, and capability loading through the operator surface.

## What Ships In v0.1.6

- 1-command installer with Meridian branding and binary-first release installs.
- Polished interactive onboarding with numbered choices, setup cards, and provider-true follow-up guidance.
- Local service lifecycle with foreground and background modes.
- Tokenized local HTTP control plane.
- Queue, job ledger, audit, parity, and shadow artifacts on disk.
- Built-in Wasm capabilities for browser navigation, terminal execution, and heartbeat scheduling.
- Capability tooling for list/show/import/verify/promote flows.

## Quick Start After Install

```bash
loom doctor --root "$HOME/.local/share/meridian-loom/runtime/default" --format human
loom capability list --root "$HOME/.local/share/meridian-loom/runtime/default" --format human
```

## Three-Part Stack

- [meridian-loom](https://github.com/mapleleaflatte03/meridian-loom): local runtime surface and operator tooling.
- [meridian-kernel](https://github.com/mapleleaflatte03/meridian-kernel): governance, policy, authority, treasury, and court.
- [meridian-intelligence](https://github.com/mapleleaflatte03/meridian-intelligence): public intelligence surface and route layer.

## Truth Boundary

- Local install, queue, audit, parity, and operator surfaces are real.
- Terminal execution, browser navigation, and heartbeat scheduling are real local primitives with bounded scope.
- Omni-channel presence and persistent memory remain broader architecture surfaces beyond the current local proof line.
- This repo does not claim hosted runtime replacement or broad production cutover.
- Legacy compatibility surfaces remain only where they still serve migration or import compatibility.

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
