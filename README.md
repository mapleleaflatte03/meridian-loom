# Meridian Loom

Experimental local scaffold for the planned Meridian-native runtime.

## Truth first

- This repo is an **experimental scaffold**, not a production runtime.
- Contract compliance remains **0/7 proven hooks**.
- OpenClaw is still the live runtime today.
- Meridian remains the governance kernel above runtimes.
- This repo exists to make the Loom product lane tangible: CLI shape, setup path,
  local state layout, contract inspection, doctor/health surfaces, and setup
  rehearsal.

## What exists in this scaffold

- A Rust workspace with:
  - `loom-core` — config/state helpers and contract inspection
  - `loom-cli` — `loom` binary
  - `loom-shadow` — shadow report reader
- Working commands:
  - `loom init`
  - `loom doctor`
  - `loom health`
  - `loom status`
  - `loom config show`
  - `loom contract show`
  - `loom capsule inspect`
  - `loom shadow report`
- A local setup rehearsal script:
  - `scripts/rehearse_setup.sh`

## What does not exist yet

- No governed execution runtime
- No worker supervisor
- No MCP / Telegram / HTTP transport
- No native hook implementation
- No shadow-mode parity engine
- No public benchmark claims

## Quick start

```bash
cargo build
./target/debug/loom init --mode embedded --kernel-path /tmp/meridian-kernel --root /tmp/loom-rehearsal
./target/debug/loom doctor --root /tmp/loom-rehearsal --format human
./target/debug/loom health --root /tmp/loom-rehearsal --format json
./target/debug/loom contract show --root /tmp/loom-rehearsal
./target/debug/loom capsule inspect --root /tmp/loom-rehearsal
./target/debug/loom shadow report --root /tmp/loom-rehearsal
```

Or run the bundled rehearsal:

```bash
./scripts/rehearse_setup.sh
```

Before public publication, run the readiness check:

```bash
./scripts/check_publication_readiness.sh
```

## Layout

```text
meridian-loom/
  Cargo.toml
  loom.toml.example
  crates/
    loom-cli/
    loom-core/
    loom-shadow/
  docs/
  scripts/
```

## Current status

This repo is enough to rehearse the install/setup/operator path honestly.
It is not enough to claim Loom exists as a runtime.

## Publication readiness

This scaffold is intended to be publish-ready before the first public GitHub
push. The publication checklist lives in
[`docs/PUBLICATION_CHECKLIST.md`](docs/PUBLICATION_CHECKLIST.md), and the
bundled readiness script verifies the local prerequisites that can be checked
without a live GitHub push.
